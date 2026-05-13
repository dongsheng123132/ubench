# UBench

**The industry standard for honest USB flash drive benchmarking.**
**全球首个绕过 OS 缓存、能识别假盘的 U 盘跑分行业标准。**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Spec](https://img.shields.io/badge/spec-UBench--v1.0-green)](SPEC.md)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-blue)]()

---

## Why UBench?

Every existing USB drive benchmark — CrystalDiskMark, ATTO, HD Tune — is **lying to you**.

They show your USB flash drive doing **5,000 MB/s reads** and **120,000 IOPS** because they measure the OS file cache (RAM speed), not the actual flash. The numbers are physically impossible: USB 2.0 tops out at 35 MB/s, USB 3.2 Gen 2 at 1.2 GB/s.

**UBench bypasses the OS cache entirely** (using `FILE_FLAG_NO_BUFFERING` on Windows, `F_NOCACHE` on macOS — the same technique Microsoft DiskSpd uses) and reports the **real** speed your drive can deliver.

### Real-world example

Same drive, same machine, same minute:

| Tool | SEQ1M Read | RND4K Read | What it actually measured |
|---|---:|---:|---|
| Naive benchmark | 5,610 MB/s | 120,169 IOPS | RAM speed (OS cache) |
| **UBench** | **467 MB/s** | **359 IOPS** | The actual USB flash chip |

The naive number was inflated **335×** for 4K random reads. That's why your "fast USB 3.0 drive" feels like garbage when you put 10,000 small files on it.

---

## Quick Start

### Install

```bash
git clone https://github.com/dongsheng123132/ubench
cd ubench
cargo build --release
# Binary at: target/release/ubench
```

Requires Rust 1.70+.

### Benchmark a drive

```bash
# Quick profile (~1 minute, 4 core tests)
ubench bench E:\ --quick

# Full profile (~10 minutes, 6 tests including SLC-cache exhaustion + cold-start latency)
ubench bench E:\ --full

# Save signed JSON report (uploadable to udiskbench.org)
ubench bench E:\ --quick --out my-drive.json

# JSON output to stdout (for AI tools / CI)
ubench bench E:\ --quick --json
```

### Sample output

```
╔══════════════════════════════════════════════════════════════╗
║  UBench v1.0.0 — 行业标准 U盘跑分                              ║
║  绕过 OS 缓存的真实速度测量 (Direct I/O)                         ║
╚══════════════════════════════════════════════════════════════╝

  目标盘     : E:\
  测试模式   : QUICK (~1 分钟,4 项)

┌──────────────────┬─────────────┬────────┬────────┬────────┐
│ 测试项           ┆ 中位数      ┆ 最小   ┆ 最大   ┆ 稳定性 │
╞══════════════════╪═════════════╪════════╪════════╪════════╡
│ SEQ1M Q1T1 Write ┆ 426.6 MB/s  ┆ 423.5  ┆ 426.6  ┆ 0.4%   │
│ SEQ1M Q1T1 Read  ┆ 478.5 MB/s  ┆ 467.4  ┆ 478.5  ┆ 1.2%   │
│ RND4K Q1T1 Write ┆ 4630.4 IOPS ┆ 4297.4 ┆ 4630.4 ┆ 3.7%   │
│ RND4K Q1T1 Read  ┆ 4214.8 IOPS ┆ 4157.6 ┆ 4214.8 ┆ 0.7%   │
└──────────────────┴─────────────┴────────┴────────┴────────┘

  ▶▶  UBench Score (UBS): 88 / 100
  ▶▶  Grade            : A (Excellent)
  签名: sha256:e8971c0e0e2307edf151281a0df2586f
```

---

## What Makes UBench Different

| | UBench | CrystalDiskMark | ATTO | Naive Tools |
|---|---|---|---|---|
| **Bypasses OS cache** | ✅ Direct I/O | ✅ (DiskSpd) | ❌ | ❌ |
| **Detects fake capacity** | ✅ | ❌ | ❌ | ❌ |
| **Tests SLC cache exhaustion** | ✅ 60s sustained | ❌ | ❌ | ❌ |
| **USB-tuned workload (Q1)** | ✅ | ❌ tests Q32 | partial | ❌ |
| **Reproducible across runs** | ✅ seeded PRNG | ⚠️ | ❌ | ❌ |
| **Cryptographically signed** | ✅ SHA-256 | ❌ | ❌ | ❌ |
| **CLI / scriptable** | ✅ `--json` | ❌ GUI only | ❌ GUI only | mixed |
| **Open spec** | ✅ [SPEC.md](SPEC.md) | partial | ❌ | ❌ |
| **License** | MIT | freeware | freeware | various |

---

## UBench Score (UBS)

A single 0-100 number summarizing the drive. See [SPEC.md §4](SPEC.md#4-scoring-ubench-score-ubs) for the full formula.

| UBS | Grade | Verdict |
|---|---|---|
| 85-100 | **A** Excellent | Premium drive, suitable for OS / heavy workload |
| 70-84 | **B** Good | Daily driver |
| 50-69 | **C** Acceptable | Document storage only |
| 25-49 | **D** Poor | Painful for many small files |
| 0-24 | **F** Garbage | Likely fake or damaged |

Weights (RND4K Read carries the most weight — 35% — because that's what determines whether your drive feels fast or slow in real life):

```
RND4K Read   35%  ← The bottleneck for "many small files" workloads
RND4K Write  25%
SEQ1M Write  15%
SEQ1M Read   10%
Sustained60s 10%  (Full profile only)
ColdStartRead 5%  (Full profile only)
```

---

## The Industry Standard

UBench v1.0 is open and free for anyone to implement. The full specification is in [SPEC.md](SPEC.md).

If you build a competing implementation that follows the spec, your scores will be **directly comparable** to ours. That's the entire point.

The signed JSON report format is stable across UBench v1.x. We're building [udiskbench.org](https://udiskbench.org) (coming soon) for crowd-sourced drive rankings.

---

## Architecture

Pure Rust CLI, no GUI dependencies, single binary.

```
src/
├── main.rs              # Entry: tokio runtime + Ctrl+C handler
├── lib.rs               # Module declarations + STOP_FLAG
├── direct_io.rs         # ★ The core: Win32 NO_BUFFERING + macOS F_NOCACHE
├── bench/
│   ├── profile.rs       # Quick / Full test profiles (the spec)
│   ├── runner.rs        # Time-windowed test execution
│   ├── score.rs         # Logarithmic UBS scoring formula
│   └── report.rs        # Signed JSON report (SHA-256)
├── disk/                # Drive detection (Windows: WMI, macOS: diskutil)
├── test/                # Legacy quality tests (capacity, badblock, thermal)
├── cli/                 # Command-line interface
├── report/              # HTML report generation
└── db.rs                # SQLite report history
```

---

## License

MIT. Use it, fork it, build a startup on top of it. Just don't claim "UBench-compliant" if you broke the spec.

---

## Contributing

Issues and PRs welcome at https://github.com/dongsheng123132/ubench

The spec ([SPEC.md](SPEC.md)) is the source of truth. If a measurement disagrees with the spec, the measurement is wrong, not the spec.

---

## Acknowledgments

- Microsoft **DiskSpd** team — for proving direct I/O is the right way
- **CrystalDiskMark** — for popularizing benchmarking; UBench started by reading its source
- The **U-Claw** project — the workload that made it obvious how broken existing benchmarks are
