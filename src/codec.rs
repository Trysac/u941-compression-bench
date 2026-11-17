use std::io::{Read, Write};

use crate::error::BenchError;

/// Maximum number of codecs allowed in a single specification.
pub const MAX_CODECS: usize = 16;

/// Hard cap on decompression output buffer size (1 GiB) to guard against decompression bombs.
const MAX_DECOMPRESS_CAPACITY: usize = 1_073_741_824;

/// Trait implemented by every benchmarked compression codec.
pub trait Codec: Send + Sync {
    /// Canonical lowercase name of the codec (`"zstd"`, `"lz4"`, `"brotli"`, `"gzip"`).
    fn name(&self) -> &str;

    /// Configured compression level (`0` for `lz4`).
    fn level(&self) -> i32;

    /// Compresses `input` into a newly allocated byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`BenchError`] with code `"codec_error"` if compression fails.
    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, BenchError>;

    /// Decompresses `input` expecting `original_len` uncompressed bytes.
    ///
    /// # Errors
    ///
    /// Returns [`BenchError`] with code `"codec_error"` if decompression fails or exceeds bounds.
    fn decompress(&self, input: &[u8], original_len: usize) -> Result<Vec<u8>, BenchError>;
}

struct ZstdCodec {
    level: i32,
}

impl Codec for ZstdCodec {
    fn name(&self) -> &'static str {
        "zstd"
    }

    fn level(&self) -> i32 {
        self.level
    }

    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, BenchError> {
        zstd::bulk::compress(input, self.level)
            .map_err(|e| BenchError::codec_error(&format!("zstd compress: {e}")))
    }

    fn decompress(&self, input: &[u8], original_len: usize) -> Result<Vec<u8>, BenchError> {
        let cap = original_len.min(MAX_DECOMPRESS_CAPACITY);
        zstd::bulk::decompress(input, cap)
            .map_err(|e| BenchError::codec_error(&format!("zstd decompress: {e}")))
    }
}

struct Lz4Codec;

impl Codec for Lz4Codec {
    fn name(&self) -> &'static str {
        "lz4"
    }

    fn level(&self) -> i32 {
        0
    }

    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, BenchError> {
        let mut enc = lz4_flex::frame::FrameEncoder::new(Vec::new());
        enc.write_all(input)
            .map_err(|e| BenchError::codec_error(&format!("lz4 write: {e}")))?;
        enc.finish()
            .map_err(|e| BenchError::codec_error(&format!("lz4 finish: {e}")))
    }

    fn decompress(&self, input: &[u8], original_len: usize) -> Result<Vec<u8>, BenchError> {
        let cap = original_len.min(MAX_DECOMPRESS_CAPACITY);
        let dec = lz4_flex::frame::FrameDecoder::new(input);
        let mut out = Vec::with_capacity(cap);
        dec.take(MAX_DECOMPRESS_CAPACITY as u64)
            .read_to_end(&mut out)
            .map_err(|e| BenchError::codec_error(&format!("lz4 decompress: {e}")))?;
        Ok(out)
    }
}

struct BrotliCodec {
    level: i32,
}

impl Codec for BrotliCodec {
    fn name(&self) -> &'static str {
        "brotli"
    }

    fn level(&self) -> i32 {
        self.level
    }

    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, BenchError> {
        let mut out = Vec::new();
        {
            let quality = u32::try_from(self.level).unwrap_or(5);
            let mut writer = brotli::CompressorWriter::new(&mut out, 4096, quality, 22);
            writer
                .write_all(input)
                .map_err(|e| BenchError::codec_error(&format!("brotli compress: {e}")))?;
            writer
                .flush()
                .map_err(|e| BenchError::codec_error(&format!("brotli flush: {e}")))?;
        }
        Ok(out)
    }

    fn decompress(&self, input: &[u8], original_len: usize) -> Result<Vec<u8>, BenchError> {
        let cap = original_len.min(MAX_DECOMPRESS_CAPACITY);
        let dec = brotli::Decompressor::new(input, 4096);
        let mut out = Vec::with_capacity(cap);
        dec.take(MAX_DECOMPRESS_CAPACITY as u64)
            .read_to_end(&mut out)
            .map_err(|e| BenchError::codec_error(&format!("brotli decompress: {e}")))?;
        Ok(out)
    }
}

struct GzipCodec {
    level: i32,
}

impl Codec for GzipCodec {
    fn name(&self) -> &'static str {
        "gzip"
    }

    fn level(&self) -> i32 {
        self.level
    }

    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, BenchError> {
        let lvl = u32::try_from(self.level).unwrap_or(6);
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(lvl));
        enc.write_all(input)
            .map_err(|e| BenchError::codec_error(&format!("gzip compress: {e}")))?;
        enc.finish()
            .map_err(|e| BenchError::codec_error(&format!("gzip finish: {e}")))
    }

    fn decompress(&self, input: &[u8], original_len: usize) -> Result<Vec<u8>, BenchError> {
        let cap = original_len.min(MAX_DECOMPRESS_CAPACITY);
        let dec = flate2::read::GzDecoder::new(input);
        let mut out = Vec::with_capacity(cap);
        dec.take(MAX_DECOMPRESS_CAPACITY as u64)
            .read_to_end(&mut out)
            .map_err(|e| BenchError::codec_error(&format!("gzip decompress: {e}")))?;
        Ok(out)
    }
}

/// Parses a comma-separated list of `name[:level]` codec specifications.
///
/// Supported codecs and level ranges:
/// - `zstd`: levels `1..=19`, default `3`
/// - `lz4`: no level (`0`, or `:0`)
/// - `brotli`: levels `0..=11`, default `5`
/// - `gzip`: levels `0..=9`, default `6`
///
/// At most 16 entries may be specified.
///
/// # Errors
///
/// Returns [`BenchError`] with code `"unknown_codec"` if a codec name is unrecognized,
/// `"invalid_level"` if a level is out of range or non-numeric, or `"usage"` if the list
/// is empty or exceeds 16 entries.
pub fn parse_codecs(spec: &str) -> Result<Vec<Box<dyn Codec>>, BenchError> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err(BenchError::usage("codec specification cannot be empty"));
    }

    let parts: Vec<&str> = trimmed.split(',').collect();
    if parts.len() > MAX_CODECS {
        return Err(BenchError::usage(&format!(
            "at most {MAX_CODECS} codecs allowed, got {}",
            parts.len()
        )));
    }

    let mut codecs: Vec<Box<dyn Codec>> = Vec::with_capacity(parts.len());
    for raw_item in parts {
        let item = raw_item.trim();
        if item.is_empty() {
            return Err(BenchError::usage("empty entry in codec specification"));
        }
        let (name, level_opt) = match item.split_once(':') {
            Some((n, l)) => (n.trim(), Some(l.trim())),
            None => (item, None),
        };

        match name {
            "zstd" => {
                let level = parse_range_level("zstd", level_opt, 3, 1, 19)?;
                codecs.push(Box::new(ZstdCodec { level }));
            }
            "lz4" => {
                if let Some(lvl_str) = level_opt {
                    let parsed = lvl_str
                        .parse::<i32>()
                        .map_err(|_| BenchError::invalid_level(&format!("lz4:{lvl_str}")))?;
                    if parsed != 0 {
                        return Err(BenchError::invalid_level(&format!(
                            "lz4 does not accept non-zero level {parsed}"
                        )));
                    }
                }
                codecs.push(Box::new(Lz4Codec));
            }
            "brotli" => {
                let level = parse_range_level("brotli", level_opt, 5, 0, 11)?;
                codecs.push(Box::new(BrotliCodec { level }));
            }
            "gzip" => {
                let level = parse_range_level("gzip", level_opt, 6, 0, 9)?;
                codecs.push(Box::new(GzipCodec { level }));
            }
            other => return Err(BenchError::unknown_codec(other)),
        }
    }

    Ok(codecs)
}

fn parse_range_level(
    codec_name: &str,
    level_opt: Option<&str>,
    default_level: i32,
    min: i32,
    max: i32,
) -> Result<i32, BenchError> {
    let Some(lvl_str) = level_opt else {
        return Ok(default_level);
    };
    if lvl_str.is_empty() {
        return Err(BenchError::invalid_level(&format!("{codec_name}:")));
    }
    let level = lvl_str
        .parse::<i32>()
        .map_err(|_| BenchError::invalid_level(&format!("{codec_name}:{lvl_str}")))?;
    if !(min..=max).contains(&level) {
        return Err(BenchError::invalid_level(&format!(
            "{codec_name}:{level} (expected {min}..={max})"
        )));
    }
    Ok(level)
}
