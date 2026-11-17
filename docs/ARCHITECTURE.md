# Architecture

## Purpose

`trysac-compression-bench` is a small, dependency-free Rust library: Benchmark CLI that compares zstd, lz4, brotli and gzip on your data for ratio, throughput and peak memory, with seeded corpora pinned by SHA-256 for reproducible reports

## Shape

```text
caller code --> crate compression_bench (src/lib.rs)
                    |
                    +-- pure functions, no I/O, no global mutable state
                    +-- private modules under src/ (optional)
```

- **Public API**: every `pub` item reachable from `src/lib.rs`. Private modules and `pub(crate)` items
  may change without a major version.
- **Errors**: invalid input returns `Err` with the crate's `Error` enum (`#[non_exhaustive]`, so new
  variants are not breaking). The library never panics on input, prints or exits.
- **Side effects**: none. If a feature needs I/O, accept a trait (`std::io::Read` or a small custom
  one) instead of opening resources, and record the choice in an ADR.
- **Safety**: `unsafe` code is forbidden by the crate's lints.

## Quality contract

`scripts/check` is the single definition of the gates; `fabrica.json` tells the factory and CI
how to run it. See [TESTING.md](TESTING.md).

## Decisions

Architecture decisions are recorded in [decisions/](decisions/) as numbered ADRs.
