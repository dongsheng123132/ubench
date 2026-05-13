//! UBench v1.0 — Industry-standard USB flash drive benchmark.
//!
//! Two profiles:
//! - `quick`: 4 tests (~1 minute)  — SEQ1M R/W + RND4K R/W
//! - `full`:  6 tests (~10 minutes) — adds 60s sustained write + cold-start latency
//!
//! All tests use `direct_io::DirectFile` to bypass OS cache.
//!
//! Spec: see SPEC.md in the repo root.

pub mod profile;
pub mod report;
pub mod runner;
pub mod score;

pub use profile::{AccessPattern, BenchProfile, Op, TestId, TestSpec};
pub use report::BenchReport;
pub use runner::{run_bench, BenchOutcome, TestMeasurement};
pub use score::{score_report, UBenchScore};
