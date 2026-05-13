# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**UBench** — Industry standard for honest USB flash drive benchmarking. Cross-platform (Windows + macOS) CLI tool in pure Rust.

The defining feature: **bypasses OS file cache** via `FILE_FLAG_NO_BUFFERING` (Windows) and `F_NOCACHE` (macOS), so reported speeds reflect the actual flash chip — not RAM. Without this, all USB benchmarks are off by 10-300×.

The project descended from `udisk-inspector` (which still exists in this repo's history under `dongsheng123132/udisk-inspector`). Capacity-verification, bad-block, and thermal tests carried over; the new `bench` command implements the UBench v1.0 standard.

## Build & Run Commands

```bash
cargo check                              # Fast type check
cargo build --release                    # Release binary

# UBench v1.0 (the new industry-standard command)
ubench bench E:\ --quick                 # ~1 minute
ubench bench E:\ --full                  # ~10 minutes
ubench bench E:\ --quick --out r.json    # Signed JSON report
ubench bench E:\ --quick --json          # JSON to stdout

# Legacy quality tests (still supported)
ubench list                              # List USB devices
ubench list --json
ubench info E:
ubench test E:\ --capacity               # Fake-capacity verification
ubench test E:\ --badblock               # Bad block scan
ubench test E:\ --thermal 60             # Sustained write stress
ubench report list                       # SQLite history
```

## Architecture

Pure Rust CLI (clap + tokio), no GUI.

### Source Layout (`src/`)

- **`main.rs`** — CLI entry: tokio runtime, Ctrl+C handler
- **`lib.rs`** — Module declarations + `STOP_FLAG: AtomicBool`
- **`direct_io.rs`** — ★ The whole point of UBench. `DirectFile` opens with NO_BUFFERING + WRITE_THROUGH on Windows. `aligned_buffer()` allocates page-aligned memory required by direct I/O. macOS uses `F_NOCACHE`. **All bench I/O must use this module — never `std::fs::File`.**
- **`bench/`** — UBench v1.0 standard
  - `profile.rs` — Quick (4 tests, ~1 min) and Full (6 tests, ~10 min) profiles. The numbers here ARE the spec — see SPEC.md.
  - `runner.rs` — Time-windowed test execution. First run discarded, median of remaining reported. Deterministic XorShift64 PRNG for reproducible random offsets.
  - `score.rs` — Logarithmic 0-100 scoring per test, weighted UBS aggregate. Targets calibrated against premium USB 3.2 Gen 2 drives.
  - `report.rs` — Signed JSON report with SHA-256 over canonical payload.
- **`test/`** — Legacy quality tests (NOT direct I/O — these are quality checks, not perf)
  - `capacity.rs` — Fake-capacity detection (write numbered blocks, read back)
  - `badblock.rs` — Full write+verify scan
  - `thermal.rs` — Sustained write degradation
- **`disk/detect.rs`** — Windows: WMI + wmic fallback. macOS: diskutil plist.
- **`cli/mod.rs`** — All subcommand handlers. The `bench` handler is `cmd_bench()` at the end of the file.
- **`report/score.rs`** — Legacy 100-point quality score (capacity 35 + speed 25 + stability 15 + badblock 25). Different from UBS.
- **`report/html.rs`** — Standalone HTML report with embedded ECharts.
- **`db.rs`** — SQLite via rusqlite (bundled). DB file: `ubench_reports.db`.

## Key Design Decisions

- **Direct I/O is non-negotiable** — Without it, the project is identical to existing flawed benchmarks. Any new bench code MUST use `crate::direct_io::DirectFile`.
- **Sector alignment** — `direct_io::SECTOR_SIZE = 4096`. All buffers from `aligned_buffer()` are page-aligned. Block sizes in profiles MUST be multiples of 4096.
- **Time-window vs byte-budget** — Bench tests run for `duration_secs`, not until N bytes. Slow drives complete fewer ops; fast drives more. Bounds wall-clock time.
- **Discard warmup** — Every test run at least 3 times; first is dropped. Median of remaining reported.
- **`formula_version` is sacred** — Bumping `"UBench-v1.0"` invalidates all historical scores. Don't tune targets cosmetically.
- **`--json` mode**: stdout is one JSON line, progress goes to stderr. AI-tool friendly.
- **OutputMode comparison** — `OutputMode` derives `PartialEq` for `mode == OutputMode::Human` checks.

## Critical Gotchas

- **WMI is unreliable** — COM init can panic; wrap in `catch_unwind` and fall back to wmic
- **Windows paths with Chinese characters** — always quote
- **All disk I/O runs in `spawn_blocking`** to keep tokio responsive
- **Direct I/O misalignment crashes** — Wrong block size or offset returns ERROR_INVALID_PARAMETER. Always validate against `SECTOR_SIZE` first.
- **`AlignedBuffer` is `unsafe impl Send`** — VirtualAlloc returns a raw pointer; we manually mark it Send because the allocation is owned exclusively. This is what `DirectFile`'s caller needs to pass buffers between threads.

## Testing the Tool

There are physical drives connected on the dev machine for calibration:
- `E:\` — mid-range USB 3.0 (iU5, 119 GiB) → expected UBS ~85
- `F:\` — cheap USB 3.0 (VendorC ProductCode, 14.6 GiB) → expected UBS ~38
- `D:\` / `Z:\` — NVMe virtual drives (baseline only) → expected UBS ~98

If your changes break these expected scores by >10 points, either you broke direct I/O or you're tuning the formula. Both require updating SPEC.md and bumping `formula_version`.
