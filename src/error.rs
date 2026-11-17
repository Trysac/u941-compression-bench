use std::fmt;

/// Maximum number of untrusted input characters echoed in an error message.
const MAX_MSG_INPUT_LEN: usize = 120;

/// Machine-readable error category for [`BenchError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Codec name is not one of `zstd`, `lz4`, `brotli`, or `gzip`.
    UnknownCodec,
    /// Codec compression level is outside its valid range or malformed.
    InvalidLevel,
    /// A corpus file's SHA-256 digest did not match the manifest.
    CorpusMismatch,
    /// Total corpus size exceeds the configured maximum byte limit.
    InputTooLarge,
    /// Decompressed output did not match the original input bytes.
    RoundtripFailed,
    /// Underlying codec failed to compress or decompress.
    CodecError,
    /// I/O error reading corpus files or manifest.
    Io,
    /// Invalid CLI arguments or configuration.
    Usage,
}

impl ErrorCode {
    /// Returns the stable machine-readable string identifier for this error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownCodec => "unknown_codec",
            Self::InvalidLevel => "invalid_level",
            Self::CorpusMismatch => "corpus_mismatch",
            Self::InputTooLarge => "input_too_large",
            Self::RoundtripFailed => "roundtrip_failed",
            Self::CodecError => "codec_error",
            Self::Io => "io",
            Self::Usage => "usage",
        }
    }
}

/// Error returned by corpus generation, codec parsing, benchmark execution, or CLI invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchError {
    code: ErrorCode,
    message: String,
}

impl BenchError {
    /// Creates a new [`BenchError`] with the given [`ErrorCode`] and detail message.
    ///
    /// Any message longer than 200 characters is truncated safely.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let raw = message.into();
        let sanitized = truncate_str(&raw, 180);
        Self {
            code,
            message: sanitized,
        }
    }

    /// Returns the stable machine-readable code string (e.g. `"unknown_codec"`).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code.as_str()
    }

    /// Returns the structured [`ErrorCode`] enum variant.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        self.code
    }

    /// Returns the human-readable error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Constructs an `unknown_codec` error.
    #[must_use]
    pub fn unknown_codec(name: &str) -> Self {
        let safe = truncate_str(name, MAX_MSG_INPUT_LEN);
        Self::new(ErrorCode::UnknownCodec, format!("unknown codec: {safe}"))
    }

    /// Constructs an `invalid_level` error.
    #[must_use]
    pub fn invalid_level(detail: &str) -> Self {
        let safe = truncate_str(detail, MAX_MSG_INPUT_LEN);
        Self::new(ErrorCode::InvalidLevel, format!("invalid level: {safe}"))
    }

    /// Constructs a `corpus_mismatch` error.
    #[must_use]
    pub fn corpus_mismatch(detail: &str) -> Self {
        let safe = truncate_str(detail, MAX_MSG_INPUT_LEN);
        Self::new(
            ErrorCode::CorpusMismatch,
            format!("corpus mismatch: {safe}"),
        )
    }

    /// Constructs an `input_too_large` error.
    #[must_use]
    pub fn input_too_large(total: usize, max: usize) -> Self {
        Self::new(
            ErrorCode::InputTooLarge,
            format!("input total size {total} exceeds limit {max}"),
        )
    }

    /// Constructs a `roundtrip_failed` error.
    #[must_use]
    pub fn roundtrip_failed(codec: &str, corpus: &str) -> Self {
        let safe_codec = truncate_str(codec, 40);
        let safe_corpus = truncate_str(corpus, 80);
        Self::new(
            ErrorCode::RoundtripFailed,
            format!("roundtrip failed for codec {safe_codec} on {safe_corpus}"),
        )
    }

    /// Constructs a `codec_error` error.
    #[must_use]
    pub fn codec_error(detail: &str) -> Self {
        let safe = truncate_str(detail, MAX_MSG_INPUT_LEN);
        Self::new(ErrorCode::CodecError, format!("codec error: {safe}"))
    }

    /// Constructs an `io` error.
    #[must_use]
    pub fn io(detail: &str) -> Self {
        let safe = truncate_str(detail, MAX_MSG_INPUT_LEN);
        Self::new(ErrorCode::Io, format!("io error: {safe}"))
    }

    /// Constructs a `usage` error.
    #[must_use]
    pub fn usage(detail: &str) -> Self {
        let safe = truncate_str(detail, MAX_MSG_INPUT_LEN);
        Self::new(ErrorCode::Usage, format!("usage error: {safe}"))
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push_str("...");
        out
    }
}

impl fmt::Display for BenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code(), self.message)
    }
}

impl std::error::Error for BenchError {}
