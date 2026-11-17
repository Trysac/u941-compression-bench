# compression-bench

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/Trysac/u941-compression-bench/actions/workflows/ci.yml/badge.svg)](https://github.com/Trysac/u941-compression-bench/actions/workflows/ci.yml)

Benchmark CLI that compares zstd, lz4, brotli and gzip on your data for ratio, throughput and peak memory, with seeded corpora pinned by SHA-256 for reproducible reports

## Problem

Describe in two or three sentences the concrete problem this utility solves, who hits it, and what
they do today without it. Replace this paragraph when the first real feature lands.

## Install

```sh
cargo add trysac-compression-bench --git https://github.com/Trysac/u941-compression-bench
```

Requires Rust 1.88 or newer. No dependencies outside the standard library.

## Usage

```rust
use compression_bench::{Error, normalize};

// normalize trims surrounding whitespace and lower-cases the text.
assert_eq!(normalize("  Hello World ").as_deref(), Ok("hello world"));

// Blank input is rejected explicitly instead of returning an empty string.
assert_eq!(compression_bench::normalize("   "), Err(Error::Blank));
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) and [decisions](docs/decisions/)
- [Testing](docs/TESTING.md) and [security](docs/SECURITY.md)
- [Project record](docs/PROJECT.md) and [stack](docs/STACK.md)
- [Changelog](CHANGELOG.md) and [contributing guide](CONTRIBUTING.md)

## License

[MIT](LICENSE) (c) 2026 Miguel Gomez
