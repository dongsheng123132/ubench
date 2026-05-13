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
    /// Device classification (USB flash drive / portable SSD / NVMe SSD / SATA SSD / HDD / Unknown).
    /// Used only as a label on reports — scoring formula stays the same across device types
    /// (so a 86 USB-stick and a 86 NVMe-SSD are equally good *for their cost class*).
    pub device_class: DeviceClass,
    /// Hardware model string (e.g. "Samsung MZAL4512HBLU-00BL2", "VendorC ProductCode")
    pub model: String,
    /// Media type from OS (SSD / HDD / Unspecified)
    pub media_type: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceClass {
    /// Plain USB stick (USB bus + no SSD media)
    UsbFlashDrive,
    /// Portable SSD in USB enclosure (USB bus + SSD media, often with UASP)
    PortableSsd,
    /// USB-attached spinning hard disk
    UsbHardDisk,
    /// Internal NVMe SSD (NVMe bus)
    NvmeSsd,
    /// Internal SATA SSD (SATA bus + SSD media)
    SataSsd,
    /// Internal SATA hard disk (SATA bus + HDD media)
    HardDisk,
    /// Cannot determine from OS metadata
    Unknown,
}

impl DeviceClass {
    pub fn label(self) -> &'static str {
        match self {
            DeviceClass::UsbFlashDrive => "USB Flash Drive (U盘)",
            DeviceClass::PortableSsd   => "Portable SSD (移动固态)",
            DeviceClass::UsbHardDisk   => "USB Hard Disk (USB 移动硬盘)",
            DeviceClass::NvmeSsd       => "NVMe SSD",
            DeviceClass::SataSsd       => "SATA SSD",
            DeviceClass::HardDisk      => "Hard Disk Drive (机械硬盘)",
            DeviceClass::Unknown       => "Unknown",
        }
    }
    /// Classify based on bus type + media type + heuristics.
    /// `bus`: "USB" / "NVMe" / "SATA" / "SCSI" / etc.
    /// `media`: "SSD" / "HDD" / "Unspecified" / ""
    /// `spindle_rpm`: 0 for SSD, >0 for HDD (sometimes only this distinguishes)
    pub fn classify(bus: &str, media: &str, spindle_rpm: u32) -> Self {
        let bus_u = bus.to_ascii_uppercase();
        let media_u = media.to_ascii_uppercase();
        match (bus_u.as_str(), media_u.as_str()) {
            ("USB", "SSD") => DeviceClass::PortableSsd,
            ("USB", "HDD") => DeviceClass::UsbHardDisk,
            ("USB", _) => {
                // Most USB sticks report "Unspecified". If there's a spindle, it's a USB HDD.
                if spindle_rpm > 0 { DeviceClass::UsbHardDisk } else { DeviceClass::UsbFlashDrive }
            }
            ("NVME", _) => DeviceClass::NvmeSsd,
            ("SATA", "SSD") => DeviceClass::SataSsd,
            ("SATA", "HDD") => DeviceClass::HardDisk,
            ("SATA", _) => {
                if spindle_rpm > 0 { DeviceClass::HardDisk } else { DeviceClass::SataSsd }
            }
            _ => DeviceClass::Unknown,
        }
    }
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
