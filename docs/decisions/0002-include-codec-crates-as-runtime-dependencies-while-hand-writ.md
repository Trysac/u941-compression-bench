# Include Codec Crates as Runtime Dependencies While Hand-Writing Harness, Corpus, and SHA-256

Status: accepted (generated, pending human review)

## Context

The benchmark utility must measure real-world compression performance and ratios across zstd, lz4, brotli, and gzip while keeping reports deterministic and free of unnecessary framework dependencies.

## Decision

Depend on zstd 0.14, lz4_flex 0.14, brotli 9, and flate2 1.1 as the subjects under test, and hand-implement SplitMix64 corpus generation, FIPS 180-4 SHA-256 hashing, median/p90 statistics, JSON/table serialization, and CLI parsing in the standard library.

## Consequences

Callers get accurate measurements of production compression codecs with zero serialization or CLI framework overhead, and report.to_json(Timings::Exclude) produces byte-identical output across machines and runs.
