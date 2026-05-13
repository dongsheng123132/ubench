//! Signed JSON report — what gets uploaded to udiskbench.org or shared.
//!
//! Format is stable across UBench v1.x; consumers can rely on it.
//! The `signature` field is SHA256 over the canonicalized payload (excluding signature itself).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::runner::BenchOutcome;
use super::score::UBenchScore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub ubench_version: String,
    pub formula_version: String,
    pub generated_at: DateTime<Utc>,
    pub host: HostInfo,
    pub drive: DriveInfo,
    pub interface: InterfaceInfo,
    pub outcome: BenchOutcome,
    pub score: UBenchScore,
    /// SHA256 of canonical JSON of all fields above (hex-lower).
    /// Anyone can recompute this hash to verify the report wasn't tampered with.
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub mount_point: String,
    pub label: String,
    pub file_system: String,
    pub claimed_capacity_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceInfo {
    /// Bus type as reported by the OS (e.g. "USB", "NVMe", "SATA")
    pub bus_type: String,
    /// USB version if known (e.g. "USB 3.0", "USB 3.2 Gen 2", "Type-C") — best-effort
    pub usb_version: Option<String>,
    /// Negotiated link speed in MB/s (best-effort)
    pub link_speed_mbps: Option<u32>,
}

impl BenchReport {
    pub fn build(
        outcome: BenchOutcome,
        score: UBenchScore,
        host: HostInfo,
        drive: DriveInfo,
        interface: InterfaceInfo,
    ) -> Self {
        let mut report = BenchReport {
            ubench_version: env!("CARGO_PKG_VERSION").to_string(),
            formula_version: score.formula_version.clone(),
            generated_at: Utc::now(),
            host,
            drive,
            interface,
            outcome,
            score,
            signature: String::new(),
        };
        report.signature = report.compute_signature();
        report
    }

    fn compute_signature(&self) -> String {
        // Serialize without the signature field for hashing
        let payload = serde_json::json!({
            "ubench_version": self.ubench_version,
            "formula_version": self.formula_version,
            "generated_at": self.generated_at,
            "host": self.host,
            "drive": self.drive,
            "interface": self.interface,
            "outcome": self.outcome,
            "score": self.score,
        });
        let canonical = serde_json::to_string(&payload).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify the signature matches. Used by the upcoming `udiskbench.org` ingester.
    pub fn verify(&self) -> bool {
        self.signature == self.compute_signature()
    }
}
