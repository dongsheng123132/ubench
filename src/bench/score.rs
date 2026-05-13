//! UBench Score (UBS) — 100-point scale.
//!
//! Each test contributes a sub-score (0-100). The final UBS is a weighted average.
//! Reference values are calibrated against real-world drives:
//!   - Garbage USB ≈ 5-15 UBS
//!   - Mid-range USB 3.0 ≈ 50-70 UBS
//!   - High-end USB 3.2 / NVMe-in-USB-enclosure ≈ 80-95 UBS
//!   - NVMe SSD (direct) ≈ 95-100 UBS
//!
//! Scoring curve uses log10 normalization to compress the wide dynamic range
//! between SLC-cached writes (1000+ MB/s) and raw QLC writes (5 MB/s).
//!
//! Spec: see SPEC.md §4 "Scoring".

use serde::{Deserialize, Serialize};

use super::profile::TestId;
use super::runner::{BenchOutcome, TestMeasurement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UBenchScore {
    /// Final aggregated UBench Score (0-100)
    pub ubs: u32,
    pub grade: Grade,
    pub sub_scores: Vec<SubScore>,
    pub formula_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubScore {
    pub test_id: TestId,
    pub label: String,
    pub measured: f64,
    pub unit: String,
    /// 100-point reference target (full score)
    pub target: f64,
    /// Weight in the final UBS calculation (0-100)
    pub weight_pct: f64,
    pub score: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Grade {
    /// Excellent — premium drive, suitable for OS / heavy workload
    A,
    /// Good — daily driver, fine for most use
    B,
    /// Acceptable — light use, document storage only
    C,
    /// Poor — slow, painful for U-Claw / multiple small files
    D,
    /// Garbage — likely fake or damaged, do not use
    F,
}

impl Grade {
    pub fn from_ubs(ubs: u32) -> Self {
        match ubs {
            85..=100 => Grade::A,
            70..=84 => Grade::B,
            50..=69 => Grade::C,
            25..=49 => Grade::D,
            _ => Grade::F,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Grade::A => "A (Excellent)",
            Grade::B => "B (Good)",
            Grade::C => "C (Acceptable)",
            Grade::D => "D (Poor)",
            Grade::F => "F (Garbage)",
        }
    }
}

/// Reference targets in MB/s (sequential) or IOPS (random).
/// `target` is the value that scores 100; `floor` is the value that scores 0.
struct Target {
    /// 100-point value
    target: f64,
    /// 0-point value (anything ≤ this → 0)
    floor: f64,
    /// Weight in final UBS (must sum to 100 across all tests in a profile)
    weight_pct: f64,
}

fn target_for(test_id: TestId) -> Target {
    // Calibration baseline (UBench v1.0):
    //   - Targets correspond to top-tier USB 3.2 drives (SanDisk Extreme Pro, etc)
    //     measured under direct I/O.
    //   - Floor = entry-level drives. Below this floor → drive is unusable.
    //   - Validated against real measurements:
    //     * E (mid-range USB-A): UBS ~75
    //     * F (cheap USB):       UBS ~10
    //     * NVMe SSD direct:     UBS ~98
    //   See SPEC.md §4 for the full calibration methodology.
    match test_id {
        // Targets calibrated to top-tier USB 3.2 Gen 2 drives (10 Gbps).
        // Mid-range USB 3.0 (5 Gbps) drives should land at UBS 70-85.
        TestId::Seq1mWrite => Target { target: 1000.0, floor: 5.0, weight_pct: 15.0 },
        TestId::Seq1mRead  => Target { target: 1000.0, floor: 10.0, weight_pct: 10.0 },
        TestId::Rnd4kWrite => Target { target: 15000.0, floor: 5.0, weight_pct: 25.0 },
        TestId::Rnd4kRead  => Target { target: 15000.0, floor: 30.0, weight_pct: 35.0 },
        TestId::Sustained60sWrite => Target { target: 500.0, floor: 2.0, weight_pct: 10.0 },
        TestId::ColdStartRead     => Target { target: 5000.0, floor: 30.0, weight_pct: 5.0 },
    }
}

/// Compute one sub-score using log-scale normalization.
/// Score = 100 * (log10(measured) - log10(floor)) / (log10(target) - log10(floor))
/// Clamped to [0, 100].
fn compute_score(measured: f64, target: &Target) -> u32 {
    if measured <= target.floor {
        return 0;
    }
    if measured >= target.target {
        return 100;
    }
    let log_m = measured.log10();
    let log_t = target.target.log10();
    let log_f = target.floor.log10();
    let pct = (log_m - log_f) / (log_t - log_f) * 100.0;
    pct.round().clamp(0.0, 100.0) as u32
}

pub fn score_report(outcome: &BenchOutcome) -> UBenchScore {
    let mut sub_scores = Vec::new();
    let mut weight_used = 0.0f64;
    let mut weighted_total = 0.0f64;

    // Index measurements by test_id for quick lookup
    let mut by_id = std::collections::HashMap::new();
    for m in &outcome.measurements {
        by_id.insert(m.test_id, m);
    }

    // Score every test that the profile *should* have included
    for spec in outcome.profile.tests() {
        let m: Option<&&TestMeasurement> = by_id.get(&spec.id);
        let target = target_for(spec.id);
        let (measured, unit) = match m {
            Some(meas) => (meas.median, meas.unit.clone()),
            None => (0.0, String::new()), // Test failed → 0 score
        };
        let score = compute_score(measured, &target);
        weighted_total += score as f64 * target.weight_pct;
        weight_used += target.weight_pct;

        sub_scores.push(SubScore {
            test_id: spec.id,
            label: spec.label.into(),
            measured,
            unit,
            target: target.target,
            weight_pct: target.weight_pct,
            score,
        });
    }

    let ubs = if weight_used > 0.0 {
        (weighted_total / weight_used).round().clamp(0.0, 100.0) as u32
    } else {
        0
    };

    UBenchScore {
        ubs,
        grade: Grade::from_ubs(ubs),
        sub_scores,
        formula_version: "UBench-v1.0".to_string(),
    }
}
