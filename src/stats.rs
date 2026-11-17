/// Computes the median of `samples` in nanoseconds.
///
/// Uses the lower median (`(n - 1) / 2` in 0-indexed sorted order) when `samples.len()` is even.
/// Returns `0` if `samples` is empty.
#[must_use]
pub fn median_ns(samples: &[u64]) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let idx = (sorted.len() - 1) / 2;
    sorted[idx]
}

/// Computes the 90th percentile of `samples` in nanoseconds using the nearest-rank method.
///
/// Rank `ceil(0.90 * N)` (1-indexed), or `(9 * N + 9) / 10 - 1` (0-indexed).
/// Returns `0` if `samples` is empty.
#[must_use]
pub fn p90_ns(samples: &[u64]) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let rank_1 = (9 * n).div_ceil(10);
    let idx = rank_1.saturating_sub(1).min(n - 1);
    sorted[idx]
}

/// Computes throughput in decimal megabytes per second (`MB/s = bytes / ns * 1000`).
///
/// Returns `0.0` if `duration_ns` is `0` or `bytes` is `0`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn throughput_mbs(bytes: usize, duration_ns: u64) -> f64 {
    if bytes == 0 || duration_ns == 0 {
        return 0.0;
    }
    (bytes as f64) * 1000.0 / (duration_ns as f64)
}

/// Computes the compression ratio `original_bytes / compressed_bytes`.
///
/// Returns `1.0` if both are `0`, or `0.0` if `compressed_bytes` is `0`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn compression_ratio(original_bytes: usize, compressed_bytes: usize) -> f64 {
    if compressed_bytes == 0 {
        if original_bytes == 0 {
            return 1.0;
        }
        return 0.0;
    }
    (original_bytes as f64) / (compressed_bytes as f64)
}
