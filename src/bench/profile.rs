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
}

impl BenchProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            BenchProfile::Quick => "quick",
            BenchProfile::Full => "full",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "quick" | "q" => Some(BenchProfile::Quick),
            "full" | "f" => Some(BenchProfile::Full),
            _ => None,
        }
    }

    pub fn tests(self) -> &'static [TestSpec] {
        match self {
            BenchProfile::Quick => QUICK_TESTS,
            BenchProfile::Full => FULL_TESTS,
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TestId {
    Seq1mRead,
    Seq1mWrite,
    Rnd4kRead,
    Rnd4kWrite,
    Sustained60sWrite,
    ColdStartRead,
}

impl TestId {
    pub fn as_str(self) -> &'static str {
        match self {
            TestId::Seq1mRead => "SEQ1M_READ",
            TestId::Seq1mWrite => "SEQ1M_WRITE",
            TestId::Rnd4kRead => "RND4K_READ",
            TestId::Rnd4kWrite => "RND4K_WRITE",
            TestId::Sustained60sWrite => "SUSTAINED_60S_WRITE",
            TestId::ColdStartRead => "COLD_START_READ",
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
/// Smaller working set, fewer repeats, no sustained / cold-start.
const QUICK_TESTS: &[TestSpec] = &[
    TestSpec {
        id: TestId::Seq1mWrite,
        label: "SEQ1M Q1T1 Write",
        block_size: 1 * 1024 * 1024,
        access: AccessPattern::Sequential,
        op: Op::Write,
        working_set: 256 * MB,
        duration_secs: 5,
        repeat: 3,
    },
    TestSpec {
        id: TestId::Seq1mRead,
        label: "SEQ1M Q1T1 Read",
        block_size: 1 * 1024 * 1024,
        access: AccessPattern::Sequential,
        op: Op::Read,
        working_set: 256 * MB,
        duration_secs: 5,
        repeat: 3,
    },
    TestSpec {
        id: TestId::Rnd4kWrite,
        label: "RND4K Q1T1 Write",
        block_size: 4 * 1024,
        access: AccessPattern::Random,
        op: Op::Write,
        working_set: 256 * MB,
        duration_secs: 5,
        repeat: 3,
    },
    TestSpec {
        id: TestId::Rnd4kRead,
        label: "RND4K Q1T1 Read",
        block_size: 4 * 1024,
        access: AccessPattern::Random,
        op: Op::Read,
        working_set: 256 * MB,
        duration_secs: 5,
        repeat: 3,
    },
];

/// Full profile (~10 minutes). 5x repeats, larger working set, adds sustained + cold-start.
const FULL_TESTS: &[TestSpec] = &[
    TestSpec {
        id: TestId::Seq1mWrite,
        label: "SEQ1M Q1T1 Write",
        block_size: 1 * 1024 * 1024,
        access: AccessPattern::Sequential,
        op: Op::Write,
        working_set: 1024 * MB,
        duration_secs: 5,
        repeat: 5,
    },
    TestSpec {
        id: TestId::Seq1mRead,
        label: "SEQ1M Q1T1 Read",
        block_size: 1 * 1024 * 1024,
        access: AccessPattern::Sequential,
        op: Op::Read,
        working_set: 1024 * MB,
        duration_secs: 5,
        repeat: 5,
    },
    TestSpec {
        id: TestId::Rnd4kWrite,
        label: "RND4K Q1T1 Write",
        block_size: 4 * 1024,
        access: AccessPattern::Random,
        op: Op::Write,
        working_set: 1024 * MB,
        duration_secs: 5,
        repeat: 5,
    },
    TestSpec {
        id: TestId::Rnd4kRead,
        label: "RND4K Q1T1 Read",
        block_size: 4 * 1024,
        access: AccessPattern::Random,
        op: Op::Read,
        working_set: 1024 * MB,
        duration_secs: 5,
        repeat: 5,
    },
    TestSpec {
        id: TestId::Sustained60sWrite,
        label: "Sustained 60s Write (SLC cache exhaustion)",
        block_size: 1 * 1024 * 1024,
        access: AccessPattern::Sequential,
        op: Op::Write,
        working_set: 8 * 1024 * MB, // up to 8 GiB or whatever fits
        duration_secs: 60,
        repeat: 1,
    },
    TestSpec {
        id: TestId::ColdStartRead,
        label: "Cold-start First-byte Read Latency",
        block_size: 4 * 1024,
        access: AccessPattern::Random,
        op: Op::Read,
        working_set: 16 * MB, // small probe
        duration_secs: 1,
        repeat: 3,
    },
];
