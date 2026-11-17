// The README is the crate documentation, so its usage example runs as a doc test.
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

/// CLI runner and argument parser.
pub mod cli;
/// Compression codecs and specification parser.
pub mod codec;
/// Seeded deterministic corpus generation and manifest validation.
pub mod corpus;
/// Error types and machine-readable error codes.
pub mod error;
/// Benchmark execution harness, clock, memory meter, and reporting.
pub mod harness;
/// FIPS 180-4 SHA-256 implementation.
pub mod sha256;
/// Summary statistics (median, p90, throughput).
pub mod stats;

pub use codec::{Codec, parse_codecs};
pub use corpus::{Kind, SplitMix64, generate_corpus};
pub use error::{BenchError, ErrorCode};
pub use harness::{
    BenchConfig, Clock, CorpusEntry, Inputs, LinuxPeakRss, MemoryMeter, MonotonicClock, NoMeter,
    Report, ResultEntry, Timings, run_bench,
};
pub use sha256::sha256_hex;
pub use stats::{median_ns, p90_ns, throughput_mbs};
