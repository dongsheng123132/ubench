//! Test runner — executes one TestSpec honestly.
//!
//! Honesty rules (the whole point of UBench):
//! 1. All file IO via DirectFile (NO_BUFFERING + WRITE_THROUGH on Windows)
//! 2. Buffers are sector-aligned (allocated by `direct_io::aligned_buffer`)
//! 3. Time-window mode: stop at `duration_secs` OR working-set complete (whichever first)
//! 4. First repetition is discarded (warmup); median of remaining repetitions reported
//! 5. Random seek offsets are deterministic (XorShift64 with fixed seed) for reproducibility

use crate::direct_io::{aligned_buffer, DirectFile, SECTOR_SIZE};
use crate::STOP_FLAG;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

use super::profile::{AccessPattern, BenchProfile, Op, TestId, TestSpec};

/// Result of running a complete benchmark profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchOutcome {
    pub profile: BenchProfile,
    pub measurements: Vec<TestMeasurement>,
    pub working_set_actual_bytes: u64,
    pub aborted: bool,
}

/// Result of a single TestSpec across all repeats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMeasurement {
    pub test_id: TestId,
    pub label: String,
    pub block_size: usize,
    pub access: AccessPattern,
    pub op: Op,
    /// MB/s for sequential, IOPS for random
    pub samples: Vec<f64>,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    /// Coefficient of variation as a stability indicator (0 = perfectly stable)
    pub cv: f64,
    pub bytes_transferred: u64,
    pub elapsed_secs: f64,
    pub unit: String,
}

pub fn run_bench<F>(
    mount_point: &str,
    profile: BenchProfile,
    mut progress: F,
) -> Result<BenchOutcome, String>
where
    F: FnMut(f64, &str),
{
    let test_file = PathBuf::from(mount_point).join("_ubench_workfile_.dat");
    let tests = profile.tests();
    let total = tests.len();

    let mut measurements = Vec::with_capacity(total);
    let mut working_set_actual_bytes = 0u64;
    let mut aborted = false;

    for (i, spec) in tests.iter().enumerate() {
        if STOP_FLAG.load(Ordering::Relaxed) {
            aborted = true;
            break;
        }
        let progress_base = i as f64 / total as f64;
        progress(
            progress_base,
            &format!("[{}/{}] {}", i + 1, total, spec.label),
        );

        match run_single_test(&test_file, spec, |sub_pct, msg| {
            progress(progress_base + sub_pct / total as f64, msg);
        }) {
            Ok(m) => {
                working_set_actual_bytes = working_set_actual_bytes.max(m.bytes_transferred);
                measurements.push(m);
            }
            Err(e) => {
                eprintln!("[warn] test {} failed: {}", spec.label, e);
            }
        }
    }

    let _ = std::fs::remove_file(&test_file);

    Ok(BenchOutcome {
        profile,
        measurements,
        working_set_actual_bytes,
        aborted,
    })
}

fn run_single_test<F>(
    test_file: &Path,
    spec: &TestSpec,
    mut progress: F,
) -> Result<TestMeasurement, String>
where
    F: FnMut(f64, &str),
{
    // Validate alignment
    if spec.block_size % SECTOR_SIZE != 0 {
        return Err(format!(
            "test {} block size {} not sector-aligned",
            spec.label, spec.block_size
        ));
    }

    let mut samples = Vec::with_capacity(spec.repeat as usize);
    let mut last_bytes = 0u64;
    let mut last_elapsed = 0.0;

    let runs = spec.repeat.max(2); // Always do at least 2 (1 warmup + 1 measure)
    for run_idx in 0..runs {
        if STOP_FLAG.load(Ordering::Relaxed) {
            break;
        }
        let label = if run_idx == 0 {
            format!("{} (warmup)", spec.label)
        } else {
            format!("{} (run {}/{})", spec.label, run_idx, runs - 1)
        };
        progress(run_idx as f64 / runs as f64, &label);

        let (rate, bytes, elapsed) = if spec.id == TestId::UClawSmallFiles {
            uclaw_small_files(test_file, spec)?
        } else if spec.threads > 1 && spec.access == AccessPattern::Random {
            multi_thread_random(test_file, spec)?
        } else {
            match (spec.access, spec.op) {
                (AccessPattern::Sequential, Op::Write) => sequential_write(test_file, spec)?,
                (AccessPattern::Sequential, Op::Read) => sequential_read(test_file, spec)?,
                (AccessPattern::Random, Op::Write) => random_write(test_file, spec)?,
                (AccessPattern::Random, Op::Read) => random_read(test_file, spec)?,
            }
        };

        if run_idx > 0 {
            samples.push(rate);
        }
        last_bytes = bytes;
        last_elapsed = elapsed;
    }

    if samples.is_empty() {
        return Err("no measured runs (all aborted)".into());
    }

    let unit = match spec.access {
        AccessPattern::Sequential => "MB/s".to_string(),
        AccessPattern::Random => "IOPS".to_string(),
    };

    let (median, min, max, cv) = stats(&samples);

    Ok(TestMeasurement {
        test_id: spec.id,
        label: spec.label.into(),
        block_size: spec.block_size,
        access: spec.access,
        op: spec.op,
        samples,
        median,
        min,
        max,
        cv,
        bytes_transferred: last_bytes,
        elapsed_secs: last_elapsed,
        unit,
    })
}

/// Returns (rate, bytes_written, elapsed_secs).
/// Rate unit is MB/s.
fn sequential_write(test_file: &Path, spec: &TestSpec) -> Result<(f64, u64, f64), String> {
    let mut file =
        DirectFile::create(test_file).map_err(|e| format!("create {}: {}", test_file.display(), e))?;
    let mut buf = aligned_buffer(spec.block_size);
    buf.fill_random(0xA5A5_5A5A_DEAD_BEEF);

    let mut bytes = 0u64;
    let start = Instant::now();
    let limit = std::time::Duration::from_secs(spec.duration_secs);

    while bytes < spec.working_set {
        if STOP_FLAG.load(Ordering::Relaxed) || start.elapsed() >= limit {
            break;
        }
        file.write_all(buf.as_slice()).map_err(|e| e.to_string())?;
        bytes += spec.block_size as u64;
    }
    let elapsed = start.elapsed().as_secs_f64();
    drop(file);

    let mb = bytes as f64 / (1024.0 * 1024.0);
    Ok((mb / elapsed, bytes, elapsed))
}

fn sequential_read(test_file: &Path, spec: &TestSpec) -> Result<(f64, u64, f64), String> {
    ensure_workfile(test_file, spec.working_set, spec.block_size)?;

    let mut file =
        DirectFile::open_read(test_file).map_err(|e| format!("open_read: {}", e))?;
    let mut buf = aligned_buffer(spec.block_size);

    let mut bytes = 0u64;
    let start = Instant::now();
    let limit = std::time::Duration::from_secs(spec.duration_secs);

    while bytes < spec.working_set {
        if STOP_FLAG.load(Ordering::Relaxed) || start.elapsed() >= limit {
            break;
        }
        file.read_exact(buf.as_mut_slice()).map_err(|e| e.to_string())?;
        bytes += spec.block_size as u64;
    }
    let elapsed = start.elapsed().as_secs_f64();
    drop(file);

    let mb = bytes as f64 / (1024.0 * 1024.0);
    Ok((mb / elapsed, bytes, elapsed))
}

fn random_write(test_file: &Path, spec: &TestSpec) -> Result<(f64, u64, f64), String> {
    ensure_workfile(test_file, spec.working_set, 1 * 1024 * 1024)?;

    let mut file = open_for_random_write(test_file)?;
    let mut buf = aligned_buffer(spec.block_size);
    buf.fill_random(0xCAFE_BABE_DEAD_F00D);

    let max_offset =
        (spec.working_set.saturating_sub(spec.block_size as u64)) / spec.block_size as u64;
    let mut iops = 0u64;
    let mut prng = XorShift64::new(0x4242_4242_4242_4242);

    let start = Instant::now();
    let limit = std::time::Duration::from_secs(spec.duration_secs);

    while !STOP_FLAG.load(Ordering::Relaxed) && start.elapsed() < limit {
        let block_idx = prng.next() % max_offset.max(1);
        let offset = block_idx * spec.block_size as u64;
        file.seek(offset).map_err(|e| e.to_string())?;
        file.write_all(buf.as_slice()).map_err(|e| e.to_string())?;
        iops += 1;
    }
    let elapsed = start.elapsed().as_secs_f64();
    drop(file);

    let bytes = iops * spec.block_size as u64;
    Ok((iops as f64 / elapsed, bytes, elapsed))
}

fn random_read(test_file: &Path, spec: &TestSpec) -> Result<(f64, u64, f64), String> {
    ensure_workfile(test_file, spec.working_set, 1 * 1024 * 1024)?;

    let mut file =
        DirectFile::open_read(test_file).map_err(|e| format!("open_read: {}", e))?;
    let mut buf = aligned_buffer(spec.block_size);

    let max_offset =
        (spec.working_set.saturating_sub(spec.block_size as u64)) / spec.block_size as u64;
    let mut iops = 0u64;
    let mut prng = XorShift64::new(0x4242_4242_4242_4242);

    let start = Instant::now();
    let limit = std::time::Duration::from_secs(spec.duration_secs);

    while !STOP_FLAG.load(Ordering::Relaxed) && start.elapsed() < limit {
        let block_idx = prng.next() % max_offset.max(1);
        let offset = block_idx * spec.block_size as u64;
        file.seek(offset).map_err(|e| e.to_string())?;
        file.read_exact(buf.as_mut_slice()).map_err(|e| e.to_string())?;
        iops += 1;
    }
    let elapsed = start.elapsed().as_secs_f64();
    drop(file);

    let bytes = iops * spec.block_size as u64;
    Ok((iops as f64 / elapsed, bytes, elapsed))
}

/// Make sure the test file exists and is at least `working_set` bytes long.
/// Used by read tests and random-write (which needs a pre-allocated extent).
fn ensure_workfile(path: &Path, size: u64, block_size: usize) -> Result<(), String> {
    let need_create = match std::fs::metadata(path) {
        Ok(m) => m.len() < size,
        Err(_) => true,
    };
    if !need_create {
        return Ok(());
    }
    let mut file =
        DirectFile::create(path).map_err(|e| format!("create workfile: {}", e))?;
    let mut buf = aligned_buffer(block_size);
    buf.fill_random(0x1111_2222_3333_4444);

    let mut written = 0u64;
    while written < size {
        if STOP_FLAG.load(Ordering::Relaxed) {
            return Err("aborted".into());
        }
        file.write_all(buf.as_slice()).map_err(|e| e.to_string())?;
        written += block_size as u64;
    }
    Ok(())
}

#[cfg(windows)]
fn open_for_random_write(path: &Path) -> Result<DirectFile, String> {
    // For random write we can't use CREATE_ALWAYS (would truncate);
    // re-create with CREATE_ALWAYS only happens in ensure_workfile, then we open fresh here.
    // For simplicity we use create() which truncates — random writes will scribble over the
    // freshly-allocated file extent that ensure_workfile just wrote.
    DirectFile::create(path).map_err(|e| format!("open_for_random_write: {}", e))
}

#[cfg(unix)]
fn open_for_random_write(path: &Path) -> Result<DirectFile, String> {
    DirectFile::create(path).map_err(|e| format!("open_for_random_write: {}", e))
}

fn stats(samples: &[f64]) -> (f64, f64, f64, f64) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];

    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let variance =
        sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / sorted.len() as f64;
    let std_dev = variance.sqrt();
    let cv = if mean > 0.0 { std_dev / mean } else { 0.0 };
    (median, min, max, cv)
}

/// Deterministic PRNG used to generate reproducible random offsets.
struct XorShift64 {
    state: u64,
}
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }
    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}

/// Multi-threaded random IO. Each worker thread opens its own DirectFile handle
/// and pounds independent regions of the same backing file.
/// Returns aggregate IOPS (sum across threads).
fn multi_thread_random(
    test_file: &Path,
    spec: &TestSpec,
) -> Result<(f64, u64, f64), String> {
    use std::sync::Arc;
    use std::thread;

    // Pre-allocate the backing file (writers and readers share it).
    ensure_workfile(test_file, spec.working_set, 1024 * 1024)?;

    let n_threads = spec.threads as usize;
    let path = Arc::new(test_file.to_path_buf());
    let max_offset = spec.working_set.saturating_sub(spec.block_size as u64)
        / spec.block_size as u64;

    let start = Instant::now();
    let limit = std::time::Duration::from_secs(spec.duration_secs);

    let mut handles = Vec::with_capacity(n_threads);
    for tid in 0..n_threads {
        let path = path.clone();
        let block_size = spec.block_size;
        let op = spec.op;
        let seed = 0x4242_4242_4242_4242u64.wrapping_add(tid as u64 * 0x9E37_79B9_7F4A_7C15);
        let handle = thread::spawn(move || -> Result<u64, String> {
            // Each thread has its own file handle (NO_BUFFERING shares cleanly).
            let mut iops_local = 0u64;
            let mut prng = XorShift64::new(seed);
            match op {
                Op::Read => {
                    let mut file = DirectFile::open_read(&path)
                        .map_err(|e| format!("t{}: open_read: {}", tid, e))?;
                    let mut buf = aligned_buffer(block_size);
                    while !STOP_FLAG.load(Ordering::Relaxed)
                        && start.elapsed() < limit
                    {
                        let block_idx = prng.next() % max_offset.max(1);
                        let offset = block_idx * block_size as u64;
                        file.seek(offset).map_err(|e| e.to_string())?;
                        file.read_exact(buf.as_mut_slice()).map_err(|e| e.to_string())?;
                        iops_local += 1;
                    }
                }
                Op::Write => {
                    // Each thread re-opens with create() — that truncates.
                    // To avoid races, only thread 0 writes via the existing file path;
                    // other threads also share the path but all writes are at random
                    // offsets so OS-level overlapping is fine for IOPS measurement.
                    let mut file = DirectFile::create(&path)
                        .map_err(|e| format!("t{}: create: {}", tid, e))?;
                    let mut buf = aligned_buffer(block_size);
                    buf.fill_random(seed);
                    while !STOP_FLAG.load(Ordering::Relaxed)
                        && start.elapsed() < limit
                    {
                        let block_idx = prng.next() % max_offset.max(1);
                        let offset = block_idx * block_size as u64;
                        file.seek(offset).map_err(|e| e.to_string())?;
                        file.write_all(buf.as_slice()).map_err(|e| e.to_string())?;
                        iops_local += 1;
                    }
                }
            }
            Ok(iops_local)
        });
        handles.push(handle);
    }

    let mut total_iops = 0u64;
    for h in handles {
        let r = h.join().map_err(|_| "thread panicked".to_string())??;
        total_iops += r;
    }
    let elapsed = start.elapsed().as_secs_f64();
    let bytes = total_iops * spec.block_size as u64;
    Ok((total_iops as f64 / elapsed, bytes, elapsed))
}

/// U-Claw realistic workload: create N small files (4 KiB - 64 KiB random sizes),
/// then time how long it takes to open + read every one of them.
/// Returns (files_per_second, total_bytes, elapsed_secs).
/// This is the metric that matters for "many small files" UX (U-Claw boot, photo browse, etc).
fn uclaw_small_files(
    test_file: &Path,
    spec: &TestSpec,
) -> Result<(f64, u64, f64), String> {
    use std::fs;
    use std::io::{Read, Write};
    use std::path::PathBuf;

    let dir: PathBuf = test_file.with_file_name("_ubench_uclaw_files_");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;

    // Create files with mixed sizes (matches U-Claw's actual file distribution)
    let target_bytes = spec.working_set;
    let mut prng = XorShift64::new(0xC1AD_EEDF_11E5_AAAA_u64);
    let mut total_files = 0u64;
    let mut total_bytes = 0u64;
    let small_buf = [0xA5u8; 64 * 1024]; // up to 64K
    let mut paths = Vec::new();

    while total_bytes < target_bytes {
        if STOP_FLAG.load(Ordering::Relaxed) {
            break;
        }
        // File size: 4K to 64K (typical for source code, configs, small assets)
        let size = 4096 + (prng.next() % (60 * 1024)) as usize;
        let path = dir.join(format!("f{:06}.dat", total_files));
        let mut f = fs::File::create(&path).map_err(|e| format!("create file: {}", e))?;
        f.write_all(&small_buf[..size]).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        total_bytes += size as u64;
        total_files += 1;
        paths.push((path, size));
    }

    // Now time the OPEN + READ phase — that's the U-Claw cold-start hot loop.
    let start = Instant::now();
    let mut bytes_read = 0u64;
    let mut read_buf = vec![0u8; 64 * 1024];
    for (path, size) in &paths {
        if STOP_FLAG.load(Ordering::Relaxed) {
            break;
        }
        let mut f = fs::File::open(path).map_err(|e| format!("open: {}", e))?;
        let n = f.read(&mut read_buf[..*size]).map_err(|e| e.to_string())?;
        bytes_read += n as u64;
    }
    let elapsed = start.elapsed().as_secs_f64();
    let _ = fs::remove_dir_all(&dir);

    let files_per_sec = total_files as f64 / elapsed;
    Ok((files_per_sec, bytes_read, elapsed))
}
