# UBench Specification v1.0

**The Industry Standard for Honest USB Flash Drive Benchmarking**

| | |
|---|---|
| **Spec version** | UBench-v1.0 |
| **Status** | Draft (calibration ongoing) |
| **License** | MIT |
| **Repository** | https://github.com/dongsheng123132/ubench |

This document defines the UBench v1.0 testing methodology and scoring formula.
Any implementation that follows this spec — in any language, on any OS — should
produce comparable scores for the same physical drive.

---

## 1. Why a New Standard?

Existing tools (CrystalDiskMark, ATTO, etc.) were designed for SSD/NVMe.
For USB flash drives they have three fatal flaws:

1. **OS cache pollution** — Default tests measure RAM speed, not flash speed.
   - Read benchmarks routinely show 5+ GB/s on USB 2.0 drives (impossible: USB 2.0 caps at 35 MB/s).
   - 4K random read IOPS inflated by 100-300×.
2. **No fake-capacity detection** — 80% of cheap-USB-market drives are fake-capacity scams; CDM won't catch any of them.
3. **Wrong workload assumptions** — CDM tests Q32T1 (32-deep queue, 1 thread).
   USB has no NCQ; queue depth doesn't help. The only depth that matters is Q1.

UBench fixes all three.

---

## 2. Honesty Rules (the whole point of UBench)

Any UBench-compliant benchmark **MUST** satisfy all five rules:

### Rule 1 — Direct I/O (no OS cache)
- **Windows**: Open files with `CreateFileW(... FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH ...)`
- **macOS**: `fcntl(fd, F_NOCACHE, 1)` after `open()`
- **Linux**: `open(O_DIRECT | O_DSYNC)`

This is the same approach used by Microsoft DiskSpd. Without it, all numbers are lies.

### Rule 2 — Sector-aligned buffers
Direct I/O requires that buffer addresses, buffer sizes, and file offsets all be aligned to the volume sector size.
UBench uses 4096-byte (page) alignment, which is safe for all common drives (512n, 512e, 4Kn).

### Rule 3 — Time-window measurement
A test run for N seconds, **not** N bytes. Slow drives complete fewer ops; fast drives complete more.
This makes the benchmark insensitive to drive speed and bounded in wall-clock time.

### Rule 4 — Discard warmup, take median
Every test runs at least `repeat` times. The first run is **always discarded** (controller warm-up,
SLC cache settling, file system journaling). The reported value is the **median** of remaining runs.

### Rule 5 — Deterministic random offsets
Random tests use a seeded PRNG (XorShift64, seed `0x4242424242424242`).
Two runs against the same drive should hit the same offsets, in the same order.
This is what makes results reproducible across machines and time.

---

## 3. Test Profiles

### 3.1 Quick Profile (~1 minute)

Four core tests. Recommended for casual users and pre-purchase verification.

| ID | Block | Pattern | Op | Working set | Duration | Repeats |
|---|---|---|---|---|---|---|
| `SEQ1M_WRITE` | 1 MiB | Sequential | Write | 256 MiB | 5 s | 3 |
| `SEQ1M_READ` | 1 MiB | Sequential | Read | 256 MiB | 5 s | 3 |
| `RND4K_WRITE` | 4 KiB | Random | Write | 256 MiB | 5 s | 3 |
| `RND4K_READ` | 4 KiB | Random | Read | 256 MiB | 5 s | 3 |

### 3.2 Full Profile (~10 minutes)

Six tests. Required for official UBS scoring submitted to udiskbench.org.

| ID | Block | Pattern | Op | Working set | Duration | Repeats |
|---|---|---|---|---|---|---|
| `SEQ1M_WRITE` | 1 MiB | Sequential | Write | 1 GiB | 5 s | 5 |
| `SEQ1M_READ` | 1 MiB | Sequential | Read | 1 GiB | 5 s | 5 |
| `RND4K_WRITE` | 4 KiB | Random | Write | 1 GiB | 5 s | 5 |
| `RND4K_READ` | 4 KiB | Random | Read | 1 GiB | 5 s | 5 |
| `SUSTAINED_60S_WRITE` | 1 MiB | Sequential | Write | 8 GiB | 60 s | 1 |
| `COLD_START_READ` | 4 KiB | Random | Read | 16 MiB | 1 s | 3 |

`SUSTAINED_60S_WRITE` exposes SLC-cache exhaustion: most cheap drives have a 100-500 MB SLC zone where writes are fast,
then collapse to native QLC speed (often <10 MB/s). The 60-second sustained run reveals the *real* steady-state speed.

`COLD_START_READ` measures first-byte latency from a freshly-opened file, important for U-Claw-like workloads
(many small files, each opened independently).

---

## 4. Scoring (UBench Score, UBS)

### 4.1 Sub-score formula (per test)

Each test produces a sub-score in `[0, 100]` using **logarithmic normalization**:

```
if measured ≤ floor:    sub_score = 0
if measured ≥ target:   sub_score = 100
otherwise:              sub_score = 100 × (log₁₀(measured) − log₁₀(floor))
                                          / (log₁₀(target)  − log₁₀(floor))
```

Log scale is used because the dynamic range is huge (real drives span 0.1 IOPS to 50,000 IOPS).
Linear scaling would compress everything below the median into the bottom 10%.

### 4.2 Reference targets and weights (UBench v1.0)

Targets are calibrated against premium USB 3.2 Gen 2 drives (10 Gbps).
Mid-range USB 3.0 (5 Gbps) drives should land at UBS 70-85.

| Test | Floor (0 pt) | Target (100 pt) | Weight |
|---|---:|---:|---:|
| SEQ1M Write | 5 MB/s | **1000 MB/s** | 15% |
| SEQ1M Read | 10 MB/s | **1000 MB/s** | 10% |
| RND4K Write | 5 IOPS | **15,000 IOPS** | 25% |
| RND4K Read | 30 IOPS | **15,000 IOPS** | 35% |
| Sustained 60s Write | 2 MB/s | **500 MB/s** | 10% |
| Cold-start Read | 30 IOPS | **5,000 IOPS** | 5% |

**Weight rationale:** RND4K Read has highest weight (35%) because that's the bottleneck for
the most common painful workload — opening many small files (e.g. starting U-Claw, browsing photos,
unzipping archives). Sequential speed matters less than people think for daily use.

### 4.3 Final UBS

```
UBS = round( Σ(sub_score × weight) / Σ(weight) )
```

### 4.4 Grades

| UBS | Grade | Meaning |
|---|---|---|
| 85-100 | **A** Excellent | Premium drive, suitable for OS / heavy workload |
| 70-84  | **B** Good      | Daily driver, fine for most use |
| 50-69  | **C** Acceptable | Light use, document storage only |
| 25-49  | **D** Poor       | Slow, painful for many small files |
| 0-24   | **F** Garbage    | Likely fake or damaged, do not use |

---

## 5. Reproducibility & Verification

Every UBench report includes:
- `ubench_version` — implementation version
- `formula_version` — `"UBench-v1.0"` (incremented only when scoring formula changes)
- `signature` — SHA-256 of canonicalized JSON of all other fields

Anyone can recompute the SHA-256 to verify the report wasn't tampered with.

`formula_version` MUST NOT change for cosmetic reasons. Bumping it invalidates all historical scores.

---

## 6. Compliance Statement

An implementation is **UBench-v1.0 compliant** if and only if:
1. It implements all five honesty rules from §2.
2. It uses the exact test parameters from §3 for the corresponding profile.
3. Its scoring formula matches §4 exactly (same targets, floors, weights, log formula).
4. Its JSON report includes the signature defined in §5.

The reference implementation lives at https://github.com/dongsheng123132/ubench
and its test results are the ground truth.

---

## 7. Future Versions

UBench v1.x will not change scoring (formula_version stays `UBench-v1.0`).
Breaking changes (new tests, retuned weights) will ship as `UBench-v2.0`.
Old scores remain comparable within their formula version.

---

## Appendix A — Why USB 4K Q1 is the right metric

USB drives are typically:
- USB 2.0: half-duplex, 1 outstanding command, no NCQ
- USB 3.0+: better, but still no command queueing on most controllers
- UASP (USB Attached SCSI Protocol): supports queueing, but few mass-market drives implement it

So Q1 (single outstanding command) is the only depth that matches real-world behavior.
Higher queue depths in CDM measure controller buffer effects, not flash speed.

## Appendix B — How fake capacity scammers fool benchmarks

Fake-capacity drives report a wrong size and silently corrupt data beyond the real limit.
Standard speed benchmarks (CDM/ATTO/HDTune) only test a small region (1 GB by default), which fits even an 8GB drive marked as 64GB.
UBench's optional capacity verification (separate from the bench command) writes numbered blocks across the full reported capacity and reads them back to detect remapping.
