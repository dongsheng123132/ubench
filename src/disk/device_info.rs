//! Device-info detection — bus type, media type, model name.
//!
//! Used for *labelling* the report (so users can see "this is a USB stick" vs
//! "this is a portable SSD"). The score formula stays the same regardless of
//! device class — that's the whole point of having an honest universal score.

use crate::bench::report::{DeviceClass, InterfaceInfo};

/// Detect device info for a given mount point (e.g. "E:\\" on Windows).
/// Best-effort: returns an InterfaceInfo with whatever could be detected.
/// Never fails (unknown fields fall back to defaults).
pub fn detect_for_mount(mount: &str) -> InterfaceInfo {
    #[cfg(target_os = "windows")]
    {
        return detect_windows(mount);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = mount;
        InterfaceInfo {
            bus_type: "Unknown".into(),
            usb_version: None,
            link_speed_mbps: None,
            device_class: DeviceClass::Unknown,
            model: String::new(),
            media_type: String::new(),
        }
    }
}

#[cfg(target_os = "windows")]
fn detect_windows(mount: &str) -> InterfaceInfo {
    // Extract drive letter (e.g. "E" from "E:\\")
    let letter = mount.chars().next().unwrap_or('?').to_ascii_uppercase();

    // PowerShell call: chain Get-Partition -> Get-Disk -> Get-PhysicalDisk
    // and emit a single line of CSV.
    let script = format!(
        r#"
$ErrorActionPreference = 'SilentlyContinue'
$p = Get-Partition -DriveLetter '{letter}' 2>$null
if (-not $p) {{ Write-Host '|||||'; exit 0 }}
$disk = Get-Disk -Number $p.DiskNumber 2>$null
$pd = Get-PhysicalDisk | Where-Object DeviceId -eq $p.DiskNumber 2>$null
$bus = if ($disk) {{ "$($disk.BusType)" }} else {{ '' }}
$model = if ($disk) {{ "$($disk.FriendlyName)" }} else {{ '' }}
$media = if ($pd) {{ "$($pd.MediaType)" }} else {{ '' }}
$spindle = if ($pd) {{ "$($pd.SpindleSpeed)" }} else {{ '0' }}
$linkSpeed = if ($pd) {{ "$($pd.PhysicalSectorSize)" }} else {{ '0' }}
Write-Host "$bus|$model|$media|$spindle|$linkSpeed"
"#,
        letter = letter
    );

    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output();

    let line = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    };

    let parts: Vec<&str> = line.split('|').collect();
    let bus = parts.first().copied().unwrap_or("").trim().to_string();
    let model = parts.get(1).copied().unwrap_or("").trim().to_string();
    let media = parts.get(2).copied().unwrap_or("").trim().to_string();
    let spindle: u32 = parts
        .get(3)
        .copied()
        .unwrap_or("0")
        .trim()
        .parse()
        .unwrap_or(0);

    let device_class = DeviceClass::classify(&bus, &media, spindle);

    InterfaceInfo {
        bus_type: if bus.is_empty() { "Unknown".into() } else { bus },
        usb_version: detect_usb_version(&device_class),
        link_speed_mbps: None,
        device_class,
        model,
        media_type: media,
    }
}

/// USB version detection placeholder — real detection requires querying USB
/// device descriptors (SetupAPI / WinUSB). For v1.0-alpha we only set this
/// when device class makes the answer obvious.
#[cfg(target_os = "windows")]
fn detect_usb_version(class: &DeviceClass) -> Option<String> {
    match class {
        DeviceClass::NvmeSsd => None, // not USB
        DeviceClass::SataSsd => None,
        DeviceClass::HardDisk => None,
        _ => None, // TODO: query USB descriptor
    }
}
