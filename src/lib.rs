// The README is the crate documentation, so its usage example runs as a doc test.
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use std::fmt;

/// Why an input was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The input is empty or holds only whitespace.
    Blank,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank => f.write_str("input is blank"),
        }
    }
}

impl std::error::Error for Error {}

/// Trims surrounding whitespace and lower-cases the text.
///
/// # Errors
///
/// Returns [`Error::Blank`] when nothing is left after trimming, instead of an empty string.
pub fn normalize(input: &str) -> Result<String, Error> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::Blank);
    }
    Ok(trimmed.to_lowercase())
}
