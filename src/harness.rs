use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use crate::codec::{Codec, parse_codecs};
use crate::corpus::{Kind, generate_corpus, parse_manifest_file};
use crate::error::BenchError;
use crate::sha256::sha256_hex;
use crate::stats::{compression_ratio, median_ns, p90_ns, throughput_mbs};

/// Default maximum total uncompressed corpus size in bytes (1 GiB).
pub const DEFAULT_MAX_TOTAL_BYTES: usize = 1_073_741_824;

/// Clock abstraction for deterministic testing and monotonic timing.
pub trait Clock {
    /// Returns the current monotonic timestamp in nanoseconds.
    fn now_ns(&self) -> u64;
}

/// Monotonic clock backed by [`std::time::Instant`].
#[derive(Debug, Clone)]
pub struct MonotonicClock {
    epoch: Instant,
}

impl MonotonicClock {
    /// Creates a new [`MonotonicClock`] anchored at the current instant.
    #[must_use]
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MonotonicClock {
    fn now_ns(&self) -> u64 {
        let nanos = self.epoch.elapsed().as_nanos();
        u64::try_from(nanos).unwrap_or(u64::MAX)
    }
}

/// Peak resident memory meter abstraction.
pub trait MemoryMeter {
    /// Resets peak high-water mark before a benchmarked run.
    fn reset(&mut self);

    /// Reads the peak resident memory in KiB, or `None` if unavailable.
    fn peak_kib(&mut self) -> Option<u64>;
}

/// Linux peak RSS meter using `/proc/self/clear_refs` and `VmHWM` in `/proc/self/status`.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinuxPeakRss {
    reset_succeeded: bool,
}

impl LinuxPeakRss {
    /// Creates a new [`LinuxPeakRss`] meter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reset_succeeded: false,
        }
    }

    /// Parses `VmHWM` in KiB from the contents of `/proc/self/status`.
    #[must_use]
    pub fn parse_vm_hwm(status_text: &str) -> Option<u64> {
        for line in status_text.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                let num_str = rest.split_whitespace().next()?;
                return num_str.parse::<u64>().ok();
            }
        }
        None
    }
}

impl MemoryMeter for LinuxPeakRss {
    fn reset(&mut self) {
        self.reset_succeeded = std::fs::write("/proc/self/clear_refs", b"5\n").is_ok();
    }

    fn peak_kib(&mut self) -> Option<u64> {
        if !self.reset_succeeded {
            return None;
        }
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        Self::parse_vm_hwm(&status)
    }
}

/// No-op memory meter that always reports `None` (`"n/a"`).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoMeter;

impl MemoryMeter for NoMeter {
    fn reset(&mut self) {}

    fn peak_kib(&mut self) -> Option<u64> {
        None
    }
}

/// Input source for a benchmark run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inputs {
    /// Deterministically generated corpus buffers.
    Generated {
        /// `SplitMix64` seed.
        seed: u64,
        /// Corpus kinds to generate.
        kinds: Vec<Kind>,
        /// Length in bytes of each generated buffer.
        size: usize,
    },
    /// User-supplied files with an optional `sha256sum` manifest for pinning.
    Files {
        /// Paths to input files.
        paths: Vec<PathBuf>,
        /// Optional manifest file path.
        manifest: Option<PathBuf>,
    },
}

/// Configuration for [`run_bench`].
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Input specification (seeded generated buffers or files).
    pub inputs: Inputs,
    /// Comma-separated codec specification (e.g. `"zstd:3,lz4,brotli:5,gzip:6"`).
    pub codecs: String,
    /// Number of untimed warmup iterations per codec and input (default `1`).
    pub warmup: usize,
    /// Number of timed measurement iterations per codec and input (default `5`).
    pub runs: usize,
    /// Maximum allowed sum of input sizes in bytes (default `1_073_741_824`).
    pub max_total_bytes: usize,
}

impl BenchConfig {
    /// Creates a [`BenchConfig`] with standard defaults (`warmup: 1`, `runs: 5`, `max_total_bytes: 1 GiB`).
    #[must_use]
    pub fn new(inputs: Inputs, codecs: impl Into<String>) -> Self {
        Self {
            inputs,
            codecs: codecs.into(),
            warmup: 1,
            runs: 5,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        }
    }
}

/// Whether to include machine-dependent timing and memory fields in JSON serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timings {
    /// Include median/p90 durations, throughput, and peak memory fields.
    Include,
    /// Omit all timing and memory fields so JSON output is 100% deterministic across machines.
    Exclude,
}

/// Metadata for a single evaluated corpus item in a [`Report`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusEntry {
    /// Name of the corpus (`"text"`, `"json"`, or the file path).
    pub name: String,
    /// Uncompressed size in bytes.
    pub size: usize,
    /// Lowercase 64-character hex SHA-256 digest of the uncompressed payload.
    pub sha256: String,
}

/// Benchmark measurement row for one codec (and level) aggregated across the corpus.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultEntry {
    /// Codec name.
    pub codec: String,
    /// Codec compression level.
    pub level: i32,
    /// Total uncompressed bytes across all inputs.
    pub original_size: usize,
    /// Total compressed bytes across all inputs.
    pub compressed_size: usize,
    /// Compression ratio (`original_size / compressed_size`).
    pub ratio: f64,
    /// Median compression duration in nanoseconds.
    pub comp_median_ns: u64,
    /// 90th percentile compression duration in nanoseconds.
    pub comp_p90_ns: u64,
    /// Median decompression duration in nanoseconds.
    pub decomp_median_ns: u64,
    /// 90th percentile decompression duration in nanoseconds.
    pub decomp_p90_ns: u64,
    /// Compression throughput in decimal MB/s.
    pub comp_mbs: f64,
    /// Decompression throughput in decimal MB/s.
    pub decomp_mbs: f64,
    /// Peak resident set size in KiB, if supported by the meter.
    pub peak_kib: Option<u64>,
}

/// Complete benchmark report containing corpus metadata and per-codec results.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Schema version (always `1`).
    pub version: u32,
    /// Pinned corpus descriptors.
    pub corpus: Vec<CorpusEntry>,
    /// Per-codec benchmark results.
    pub results: Vec<ResultEntry>,
}

impl Report {
    /// Serializes the report as hand-formatted JSON with a fixed key order:
    /// top-level keys `"version"`, `"corpus"`, `"results"`.
    #[must_use]
    pub fn to_json(&self, timings: Timings) -> String {
        let mut out = String::with_capacity(512);
        out.push_str("{\n  \"version\": ");
        let _ = writeln!(out, "{},", self.version);

        out.push_str("  \"corpus\": [");
        if !self.corpus.is_empty() {
            out.push('\n');
            for (i, c) in self.corpus.iter().enumerate() {
                out.push_str("    {\"name\": \"");
                escape_json_into(&c.name, &mut out);
                let _ = write!(
                    out,
                    "\", \"size\": {}, \"sha256\": \"{}\"}}",
                    c.size, c.sha256
                );
                if i + 1 < self.corpus.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("  ");
        }
        out.push_str("],\n");

        out.push_str("  \"results\": [");
        if !self.results.is_empty() {
            out.push('\n');
            for (i, r) in self.results.iter().enumerate() {
                out.push_str("    {\"codec\": \"");
                escape_json_into(&r.codec, &mut out);
                let _ = write!(
                    out,
                    "\", \"level\": {}, \"original_size\": {}, \"compressed_size\": {}, \"ratio\": \"{:.3}\"",
                    r.level, r.original_size, r.compressed_size, r.ratio
                );
                if timings == Timings::Include {
                    let _ = write!(
                        out,
                        ", \"comp_median_ns\": {}, \"comp_p90_ns\": {}, \"decomp_median_ns\": {}, \"decomp_p90_ns\": {}, \"comp_mbs\": \"{:.3}\", \"decomp_mbs\": \"{:.3}\", \"peak_kib\": ",
                        r.comp_median_ns,
                        r.comp_p90_ns,
                        r.decomp_median_ns,
                        r.decomp_p90_ns,
                        r.comp_mbs,
                        r.decomp_mbs
                    );
                    match r.peak_kib {
                        Some(kib) => {
                            let _ = write!(out, "{kib}");
                        }
                        None => out.push_str("null"),
                    }
                }
                out.push('}');
                if i + 1 < self.results.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("  ");
        }
        out.push_str("]\n}\n");
        out
    }

    /// Formats the report as a human-readable aligned text table.
    ///
    /// Columns: `codec`, `level`, `ratio`, `comp MB/s`, `decomp MB/s`, `peak KiB`.
    #[must_use]
    pub fn to_table(&self) -> String {
        let mut out = String::with_capacity(256);
        let _ = writeln!(
            out,
            "{:<8} {:>5} {:>8} {:>11} {:>11} {:>8}",
            "codec", "level", "ratio", "comp MB/s", "decomp MB/s", "peak KiB"
        );
        for r in &self.results {
            let peak_str = r
                .peak_kib
                .map_or_else(|| "n/a".to_string(), |v| v.to_string());
            let _ = writeln!(
                out,
                "{:<8} {:>5} {:>8.3} {:>11.2} {:>11.2} {:>8}",
                r.codec, r.level, r.ratio, r.comp_mbs, r.decomp_mbs, peak_str
            );
        }
        out
    }
}

fn escape_json_into(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

struct LoadedCorpus {
    entry: CorpusEntry,
    bytes: Vec<u8>,
}

fn load_inputs(inputs: &Inputs, max_total_bytes: usize) -> Result<Vec<LoadedCorpus>, BenchError> {
    let mut loaded = Vec::new();
    let mut total_bytes: usize = 0;

    match inputs {
        Inputs::Generated { seed, kinds, size } => {
            if kinds.is_empty() {
                return Err(BenchError::usage("at least one corpus kind is required"));
            }
            for &kind in kinds {
                total_bytes = total_bytes
                    .checked_add(*size)
                    .ok_or_else(|| BenchError::input_too_large(usize::MAX, max_total_bytes))?;
                if total_bytes > max_total_bytes {
                    return Err(BenchError::input_too_large(total_bytes, max_total_bytes));
                }
                let bytes = generate_corpus(*seed, kind, *size);
                let sha256 = sha256_hex(&bytes);
                loaded.push(LoadedCorpus {
                    entry: CorpusEntry {
                        name: kind.as_str().to_string(),
                        size: bytes.len(),
                        sha256,
                    },
                    bytes,
                });
            }
        }
        Inputs::Files { paths, manifest } => {
            if paths.is_empty() {
                return Err(BenchError::usage("at least one input file is required"));
            }
            let manifest_map = match manifest {
                Some(mpath) => Some(parse_manifest_file(mpath)?),
                None => None,
            };

            for path in paths {
                let meta = std::fs::metadata(path)
                    .map_err(|e| BenchError::io(&format!("{}: {e}", path.display())))?;
                let file_len = usize::try_from(meta.len())
                    .map_err(|_| BenchError::input_too_large(usize::MAX, max_total_bytes))?;
                total_bytes = total_bytes
                    .checked_add(file_len)
                    .ok_or_else(|| BenchError::input_too_large(usize::MAX, max_total_bytes))?;
                if total_bytes > max_total_bytes {
                    return Err(BenchError::input_too_large(total_bytes, max_total_bytes));
                }
                let bytes = std::fs::read(path)
                    .map_err(|e| BenchError::io(&format!("{}: {e}", path.display())))?;
                if bytes.len() > max_total_bytes {
                    return Err(BenchError::input_too_large(bytes.len(), max_total_bytes));
                }
                let sha256 = sha256_hex(&bytes);
                let display_name = path.to_string_lossy().into_owned();

                if let Some(ref map) = manifest_map {
                    let expected = map.get(&display_name).or_else(|| {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .and_then(|n| map.get(n))
                    });
                    match expected {
                        Some(exp_hex) if exp_hex == &sha256 => {}
                        Some(exp_hex) => {
                            return Err(BenchError::corpus_mismatch(&format!(
                                "{display_name}: expected {exp_hex}, got {sha256}"
                            )));
                        }
                        None => {
                            return Err(BenchError::corpus_mismatch(&format!(
                                "{display_name}: missing from manifest"
                            )));
                        }
                    }
                }

                loaded.push(LoadedCorpus {
                    entry: CorpusEntry {
                        name: display_name,
                        size: bytes.len(),
                        sha256,
                    },
                    bytes,
                });
            }
        }
    }

    Ok(loaded)
}

/// Runs the compression benchmark over the configured inputs and codecs.
///
/// # Errors
///
/// Returns [`BenchError`] if:
/// - Codec specification is invalid (`"unknown_codec"`, `"invalid_level"`, `"usage"`),
/// - Total corpus size exceeds `config.max_total_bytes` (`"input_too_large"`),
/// - A file does not match its manifest entry (`"corpus_mismatch"`),
/// - Any codec fails or produces a mismatched roundtrip (`"codec_error"`, `"roundtrip_failed"`).
pub fn run_bench(
    config: &BenchConfig,
    clock: &dyn Clock,
    meter: &mut dyn MemoryMeter,
) -> Result<Report, BenchError> {
    if config.runs == 0 {
        return Err(BenchError::usage("runs must be at least 1"));
    }
    let codecs = parse_codecs(&config.codecs)?;
    let corpora = load_inputs(&config.inputs, config.max_total_bytes)?;

    let mut results = Vec::with_capacity(codecs.len());
    for codec in &codecs {
        let entry = measure_codec(
            codec.as_ref(),
            &corpora,
            config.warmup,
            config.runs,
            clock,
            meter,
        )?;
        results.push(entry);
    }

    let corpus_entries = corpora.into_iter().map(|c| c.entry).collect();
    Ok(Report {
        version: 1,
        corpus: corpus_entries,
        results,
    })
}

fn measure_codec(
    codec: &dyn Codec,
    corpora: &[LoadedCorpus],
    warmup: usize,
    runs: usize,
    clock: &dyn Clock,
    meter: &mut dyn MemoryMeter,
) -> Result<ResultEntry, BenchError> {
    meter.reset();

    let mut original_size: usize = 0;
    let mut compressed_size: usize = 0;
    let mut compressed_payloads: Vec<Vec<u8>> = Vec::with_capacity(corpora.len());

    // Warmup & roundtrip verification for every corpus buffer
    for item in corpora {
        original_size += item.bytes.len();
        let mut last_comp = None;
        let iterations = warmup.max(1);
        for _ in 0..iterations {
            let comp = codec.compress(&item.bytes)?;
            let decomp = codec.decompress(&comp, item.bytes.len())?;
            if decomp != item.bytes {
                return Err(BenchError::roundtrip_failed(codec.name(), &item.entry.name));
            }
            last_comp = Some(comp);
        }
        let comp = last_comp.unwrap_or_default();
        compressed_size += comp.len();
        compressed_payloads.push(comp);
    }

    // Timed compression runs
    let mut comp_samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = clock.now_ns();
        let mut run_compressed = Vec::with_capacity(corpora.len());
        for item in corpora {
            let comp = codec.compress(&item.bytes)?;
            run_compressed.push(comp);
        }
        let end = clock.now_ns();
        comp_samples.push(end.saturating_sub(start));
        compressed_payloads = run_compressed;
    }

    // Timed decompression runs
    let mut decomp_samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = clock.now_ns();
        for (item, comp) in corpora.iter().zip(compressed_payloads.iter()) {
            let decomp = codec.decompress(comp, item.bytes.len())?;
            if decomp != item.bytes {
                return Err(BenchError::roundtrip_failed(codec.name(), &item.entry.name));
            }
        }
        let end = clock.now_ns();
        decomp_samples.push(end.saturating_sub(start));
    }

    let peak_kib = meter.peak_kib();
    let comp_med = median_ns(&comp_samples);
    let comp_p90 = p90_ns(&comp_samples);
    let decomp_med = median_ns(&decomp_samples);
    let decomp_p90 = p90_ns(&decomp_samples);

    let ratio = compression_ratio(original_size, compressed_size);
    let comp_mbs = throughput_mbs(original_size, comp_med);
    let decomp_mbs = throughput_mbs(original_size, decomp_med);

    Ok(ResultEntry {
        codec: codec.name().to_string(),
        level: codec.level(),
        original_size,
        compressed_size,
        ratio,
        comp_median_ns: comp_med,
        comp_p90_ns: comp_p90,
        decomp_median_ns: decomp_med,
        decomp_p90_ns: decomp_p90,
        comp_mbs,
        decomp_mbs,
        peak_kib,
    })
}
