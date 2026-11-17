# Testing

## Existing Tests

- `tests/api.rs`: Integration and property tests verifying the public API. Exercises the `FakeClock` with custom durations to verify median and p90 calculation, and tests basic `run_bench` flows.
- `src/codec.rs`: Unit tests for `parse_codecs` and logic for individual codec implementations (zstd, lz4, brotli, gzip).
- `src/corpus.rs`: Unit tests verifying `SplitMix64` output, `generate_corpus` determinism, and `parse_manifest` behavior.
- `src/error.rs`: Logic for truncating error messages and formatting.
- `src/harness.rs`: Logic for report generation and input handling.
- `src/stats.rs`: Mathematical validation for `median_ns`, `p90_ns`, `throughput_mbs`, and `compression_ratio`.
