//! UBench v1.0 standard test profiles.
//!
//! Numbers below are the spec — DO NOT change without bumping the version
//! and updating SPEC.md, otherwise scores stop being comparable across runs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BenchProfile {
    /// 1 minute — minimum viable benchmark
    Quick,
    /// 10 minutes — full standard benchmark (recommended for scoring)
    Full,
    /// AS SSD compatibility — 4 tests matching AS SSD Benchmark layout (Seq + 4K + 4K-64Thrd + AccTime)
    AsSsdCompat,
}

impl BenchProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            BenchProfile::Quick => "quick",
            BenchProfile::Full => "full",
            BenchProfile::AsSsdCompat => "asssd",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "quick" | "q" => Some(BenchProfile::Quick),
            "full" | "f" => Some(BenchProfile::Full),
            "asssd" | "as-ssd" | "as_ssd" => Some(BenchProfile::AsSsdCompat),
            _ => None,
        }
    }

    pub fn tests(self) -> &'static [TestSpec] {
        match self {
            BenchProfile::Quick => QUICK_TESTS,
            BenchProfile::Full => FULL_TESTS,
            BenchProfile::AsSsdCompat => AS_SSD_TESTS,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TestSpec {
    pub id: TestId,
    /// Human label e.g. "SEQ1M Q1T1 Read"
    pub label: &'static str,
    /// Block size in bytes (must be sector-aligned multiple)
    pub block_size: usize,
    /// Sequential or random
    pub access: AccessPattern,
    /// Read or write
    pub op: Op,
    /// Test working-set size in bytes (file size for the test)
    pub working_set: u64,
    /// Time budget in seconds (test stops at first of: budget OR working-set scanned)
    pub duration_secs: u64,
    /// How many times to run; the median of the last (n-1) runs is reported
    /// (the first run is always discarded as warmup).
    pub repeat: u32,
    /// Number of parallel worker threads (1 = single-threaded).
    /// AS SSD's "4K-64 Thrd" uses 64. Set to 1 for AS SSD's plain "4K".
    pub threads: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TestId {
    Seq1mRead,
    Seq1mWrite,
    Rnd4kRead,
    Rnd4kWrite,
    /// AS SSD compatibility: 4K random read with 64 worker threads
    Rnd4k64thRead,
    /// AS SSD compatibility: 4K random write with 64 worker threads
    Rnd4k64thWrite,
    Sustained60sWrite,
    ColdStartRead,
    /// U-Claw realistic workload: open + read N small files, report seconds
    UClawSmallFiles,
}

impl TestId {
    pub fn as_str(self) -> &'static str {
        match self {
            TestId::Seq1mRead => "SEQ1M_READ",
            TestId::Seq1mWrite => "SEQ1M_WRITE",
            TestId::Rnd4kRead => "RND4K_READ",
            TestId::Rnd4kWrite => "RND4K_WRITE",
            TestId::Rnd4k64thRead => "RND4K_64TH_READ",
            TestId::Rnd4k64thWrite => "RND4K_64TH_WRITE",
            TestId::Sustained60sWrite => "SUSTAINED_60S_WRITE",
            TestId::ColdStartRead => "COLD_START_READ",
            TestId::UClawSmallFiles => "UCLAW_SMALL_FILES",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccessPattern {
    Sequential,
    Random,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Op {
    Read,
    Write,
}

const MB: u64 = 1024 * 1024;

/// Quick profile (~60 seconds).
const QUICK_TESTS: &[TestSpec] = &[
    TestSpec {
        id: TestId::Seq1mWrite, label: "SEQ1M Q1T1 Write",
        block_size: 1024 * 1024, access: AccessPattern::Sequential, op: Op::Write,
        working_set: 256 * MB, duration_secs: 5, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::Seq1mRead, label: "SEQ1M Q1T1 Read",
        block_size: 1024 * 1024, access: AccessPattern::Sequential, op: Op::Read,
        working_set: 256 * MB, duration_secs: 5, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4kWrite, label: "RND4K Q1T1 Write",
        block_size: 4096, access: AccessPattern::Random, op: Op::Write,
        working_set: 256 * MB, duration_secs: 5, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4kRead, label: "RND4K Q1T1 Read",
        block_size: 4096, access: AccessPattern::Random, op: Op::Read,
        working_set: 256 * MB, duration_secs: 5, repeat: 3, threads: 1,
    },
];

/// Full profile (~10 minutes). Adds 64-thread tests (AS SSD compat),
/// 60s sustained write, cold-start latency, and U-Claw small-file probe.
const FULL_TESTS: &[TestSpec] = &[
    TestSpec {
        id: TestId::Seq1mWrite, label: "SEQ1M Q1T1 Write",
        block_size: 1024 * 1024, access: AccessPattern::Sequential, op: Op::Write,
        working_set: 1024 * MB, duration_secs: 5, repeat: 5, threads: 1,
    },
    TestSpec {
        id: TestId::Seq1mRead, label: "SEQ1M Q1T1 Read",
        block_size: 1024 * 1024, access: AccessPattern::Sequential, op: Op::Read,
        working_set: 1024 * MB, duration_secs: 5, repeat: 5, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4kWrite, label: "RND4K Q1T1 Write",
        block_size: 4096, access: AccessPattern::Random, op: Op::Write,
        working_set: 1024 * MB, duration_secs: 5, repeat: 5, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4kRead, label: "RND4K Q1T1 Read",
        block_size: 4096, access: AccessPattern::Random, op: Op::Read,
        working_set: 1024 * MB, duration_secs: 5, repeat: 5, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4k64thWrite, label: "RND4K-64Thrd Write (AS SSD compat)",
        block_size: 4096, access: AccessPattern::Random, op: Op::Write,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 64,
    },
    TestSpec {
        id: TestId::Rnd4k64thRead, label: "RND4K-64Thrd Read (AS SSD compat)",
        block_size: 4096, access: AccessPattern::Random, op: Op::Read,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 64,
    },
    TestSpec {
        id: TestId::Sustained60sWrite, label: "Sustained 60s Write (SLC cache exhaustion)",
        block_size: 1024 * 1024, access: AccessPattern::Sequential, op: Op::Write,
        working_set: 8 * 1024 * MB, duration_secs: 60, repeat: 1, threads: 1,
    },
    TestSpec {
        id: TestId::ColdStartRead, label: "Cold-start First-byte Read Latency",
        block_size: 4096, access: AccessPattern::Random, op: Op::Read,
        working_set: 16 * MB, duration_secs: 1, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::UClawSmallFiles, label: "U-Claw Small-Files Real Workload",
        block_size: 4096, access: AccessPattern::Random, op: Op::Read,
        working_set: 64 * MB, duration_secs: 30, repeat: 1, threads: 1,
    },
];

/// AS SSD Benchmark compatibility profile.
/// Matches AS SSD's 4 tests so users can directly compare with merchant screenshots.
/// Note: AS SSD reports as MB/s; UBench also reports MB/s for these (rate * block_size).
const AS_SSD_TESTS: &[TestSpec] = &[
    TestSpec {
        id: TestId::Seq1mWrite, label: "Seq Write (AS SSD)",
        block_size: 1024 * 1024, access: AccessPattern::Sequential, op: Op::Write,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::Seq1mRead, label: "Seq Read (AS SSD)",
        block_size: 1024 * 1024, access: AccessPattern::Sequential, op: Op::Read,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4kWrite, label: "4K Write (AS SSD)",
        block_size: 4096, access: AccessPattern::Random, op: Op::Write,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4kRead, label: "4K Read (AS SSD)",
        block_size: 4096, access: AccessPattern::Random, op: Op::Read,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 1,
    },
    TestSpec {
        id: TestId::Rnd4k64thWrite, label: "4K-64Thrd Write (AS SSD)",
        block_size: 4096, access: AccessPattern::Random, op: Op::Write,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 64,
    },
    TestSpec {
        id: TestId::Rnd4k64thRead, label: "4K-64Thrd Read (AS SSD)",
        block_size: 4096, access: AccessPattern::Random, op: Op::Read,
        working_set: 1024 * MB, duration_secs: 10, repeat: 3, threads: 64,
    },
];
