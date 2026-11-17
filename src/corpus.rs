use std::collections::HashMap;
use std::fmt::{self, Write as _};
use std::path::Path;

use crate::error::BenchError;

/// Maximum manifest file size in bytes (16 MiB).
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

/// Deterministic `SplitMix64` pseudo-random number generator (Steele, Lea, and Flood 2014).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Initializes a `SplitMix64` generator with the given `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Advances the state and returns the next 64-bit pseudo-random integer.
    #[allow(clippy::should_implement_trait)]
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Synthetic corpus kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// English prose built from a 256-word vocabulary with spaces and newlines.
    Text,
    /// Newline-delimited JSON log records.
    Json,
    /// Raw `SplitMix64` pseudo-random bytes.
    Random,
    /// All zero bytes.
    Zeros,
}

impl Kind {
    /// Returns the canonical lowercase name of the corpus kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
            Self::Random => "random",
            Self::Zeros => "zeros",
        }
    }

    /// Parses a corpus kind from its lowercase name (`"text"`, `"json"`, `"random"`, `"zeros"`).
    ///
    /// # Errors
    ///
    /// Returns [`BenchError`] with code `"usage"` if `s` is not a recognized kind.
    pub fn parse(s: &str) -> Result<Self, BenchError> {
        match s.trim() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "random" => Ok(Self::Random),
            "zeros" => Ok(Self::Zeros),
            other => Err(BenchError::usage(&format!("unknown corpus kind: {other}"))),
        }
    }

    /// Parses a comma-separated list of corpus kinds.
    ///
    /// # Errors
    ///
    /// Returns [`BenchError`] with code `"usage"` if the list is empty or contains an unknown kind.
    pub fn parse_list(spec: &str) -> Result<Vec<Self>, BenchError> {
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return Err(BenchError::usage("corpus kinds list cannot be empty"));
        }
        let mut out = Vec::new();
        for part in trimmed.split(',') {
            let k = Self::parse(part)?;
            out.push(k);
        }
        Ok(out)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

const WORDS: [&str; 256] = [
    "the", "be", "to", "of", "and", "a", "in", "that", "have", "i", "it", "for", "not", "on",
    "with", "he", "as", "you", "do", "at", "this", "but", "his", "by", "from", "they", "we", "say",
    "her", "she", "or", "an", "will", "my", "one", "all", "would", "there", "their", "what", "so",
    "up", "out", "if", "about", "who", "get", "which", "go", "me", "when", "make", "can", "like",
    "time", "no", "just", "him", "know", "take", "people", "into", "year", "your", "good", "some",
    "could", "them", "see", "other", "than", "then", "now", "look", "only", "come", "its", "over",
    "think", "also", "back", "after", "use", "two", "how", "our", "work", "first", "well", "way",
    "even", "new", "want", "because", "any", "these", "give", "day", "most", "us", "system",
    "data", "block", "stream", "frame", "buffer", "window", "table", "symbol", "token", "match",
    "offset", "length", "header", "footer", "digest", "check", "verify", "encode", "decode",
    "writer", "reader", "input", "output", "slice", "vector", "memory", "cache", "cycle", "clock",
    "timer", "sample", "median", "ratio", "speed", "level", "fast", "dense", "small", "large",
    "clear", "clean", "exact", "fixed", "stable", "seed", "state", "round", "shift", "value",
    "count", "index", "entry", "field", "record", "object", "array", "string", "number", "boolean",
    "metric", "report", "format", "schema", "version", "result", "status", "error", "valid",
    "limit", "bound", "range", "scale", "factor", "weight", "score", "alpha", "beta", "gamma",
    "delta", "theta", "lambda", "sigma", "omega", "north", "south", "east", "west", "upper",
    "lower", "inner", "outer", "early", "late", "short", "long", "high", "low", "deep", "wide",
    "open", "close", "start", "stop", "begin", "finish", "create", "build", "parse", "print",
    "write", "read", "store", "load", "fetch", "flush", "trace", "debug", "info", "warn", "alert",
    "fatal", "event", "action", "client", "server", "node", "cluster", "route", "packet", "socket",
    "port", "queue", "stack", "tree", "graph", "leaf", "root", "branch", "edge", "source",
    "target", "origin", "path", "file", "folder", "disk", "volume", "byte", "word", "page",
    "chunk", "segment", "sector", "region", "zone",
];

const LOG_LEVELS: [&str; 4] = ["info", "warn", "error", "debug"];

/// Generates a deterministic byte corpus of exact length `len` using `SplitMix64(seed)`.
#[must_use]
pub fn generate_corpus(seed: u64, kind: Kind, len: usize) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    match kind {
        Kind::Zeros => vec![0u8; len],
        Kind::Random => {
            let mut rng = SplitMix64::new(seed);
            let mut buf = Vec::with_capacity(len);
            while buf.len() < len {
                let val = rng.next_u64().to_le_bytes();
                let remaining = len - buf.len();
                if remaining >= 8 {
                    buf.extend_from_slice(&val);
                } else {
                    buf.extend_from_slice(&val[..remaining]);
                }
            }
            buf
        }
        Kind::Text => {
            let mut rng = SplitMix64::new(seed);
            let mut buf = Vec::with_capacity(len + 32);
            let mut word_in_line = 0usize;
            while buf.len() < len {
                let val = rng.next_u64();
                for shift in (0..64).step_by(8) {
                    if buf.len() >= len {
                        break;
                    }
                    let idx = ((val >> shift) & 0xFF) as usize;
                    buf.extend_from_slice(WORDS[idx].as_bytes());
                    word_in_line += 1;
                    if word_in_line >= 12 {
                        buf.push(b'\n');
                        word_in_line = 0;
                    } else {
                        buf.push(b' ');
                    }
                }
            }
            buf.truncate(len);
            buf
        }
        Kind::Json => {
            let mut rng = SplitMix64::new(seed);
            let mut buf = Vec::with_capacity(len + 128);
            let mut line = String::with_capacity(128);
            let mut counter = 0u64;
            while buf.len() < len {
                let r1 = rng.next_u64();
                let r2 = rng.next_u64();
                let ts = 1_700_000_000_000u64 + (r1 % 86_400_000);
                let lvl = LOG_LEVELS[((r1 >> 32) & 0x03) as usize];
                let w1 = WORDS[(r2 & 0xFF) as usize];
                let w2 = WORDS[((r2 >> 8) & 0xFF) as usize];
                let w3 = WORDS[((r2 >> 16) & 0xFF) as usize];
                let w4 = WORDS[((r2 >> 24) & 0xFF) as usize];
                let id = (r2 >> 32) ^ counter;
                counter = counter.wrapping_add(1);
                line.clear();
                let _ = writeln!(
                    line,
                    "{{\"ts\":{ts},\"level\":\"{lvl}\",\"msg\":\"{w1} {w2} {w3} {w4}\",\"id\":{id}}}"
                );
                buf.extend_from_slice(line.as_bytes());
            }
            buf.truncate(len);
            buf
        }
    }
}

/// Parses a `sha256sum`-compatible manifest file (`"<64-hex>  <path>"` or `"<64-hex> *<path>"`).
///
/// # Errors
///
/// Returns [`BenchError`] with code `"io"` if reading fails or `"corpus_mismatch"` / `"usage"` if malformed.
pub fn parse_manifest_file(path: &Path) -> Result<HashMap<String, String>, BenchError> {
    let meta = std::fs::metadata(path).map_err(|e| BenchError::io(&e.to_string()))?;
    if meta.len() > MAX_MANIFEST_BYTES {
        return Err(BenchError::usage("manifest file exceeds 16 MiB limit"));
    }
    let content = std::fs::read_to_string(path).map_err(|e| BenchError::io(&e.to_string()))?;
    parse_manifest_str(&content)
}

/// Parses `sha256sum` lines from a string into a map from file path to expected lowercase hex digest.
///
/// # Errors
///
/// Returns [`BenchError`] with code `"usage"` if a non-empty line is not in valid `sha256sum` format.
pub fn parse_manifest_str(content: &str) -> Result<HashMap<String, String>, BenchError> {
    let mut map = HashMap::new();
    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let (hex_part, path_part) = line
            .split_once("  ")
            .or_else(|| line.split_once(" *"))
            .ok_or_else(|| BenchError::usage(&format!("malformed manifest line {}", idx + 1)))?;
        if hex_part.len() != 64 || !hex_part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(BenchError::usage(&format!(
                "invalid sha256 hex on manifest line {}",
                idx + 1
            )));
        }
        if path_part.is_empty() {
            return Err(BenchError::usage(&format!(
                "empty file path on manifest line {}",
                idx + 1
            )));
        }
        map.insert(path_part.to_string(), hex_part.to_ascii_lowercase());
    }
    Ok(map)
}
