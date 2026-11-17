# compression-bench

Teams pick a compression codec from blog benchmarks run on someone else's data and hardware, then find it slow or weak on their own payloads. Ad hoc scripts measure once, skip round-trip checks and cannot be rerun on the same input. This crate runs every codec at chosen levels over a seeded or pinned corpus, verifies each round trip and reports ratio, median throughput and memory in a report whose non-timing fields are byte-identical across runs.

## Usage

```rust
use compression_bench::{BenchConfig, Inputs, Kind, MonotonicClock, NoMeter, Timings, run_bench};

let inputs = Inputs::Generated { seed: 42, kinds: vec![Kind::Text], size: 65_536 };
let config = BenchConfig::new(inputs, "zstd:3,lz4,brotli:5,gzip:6");
let report = run_bench(&config, &MonotonicClock::new(), &mut NoMeter).unwrap();
assert!(report.to_json(Timings::Exclude).contains("\"version\": 1"));
println!("{}", report.to_table());
```

## Public API Summary

- `Codec`: Trait for `zstd` (1-19), `lz4` (0), `brotli` (0-11), `gzip` (0-9).
- `parse_codecs`: Parses comma-separated specification like `"zstd:3,lz4"` (max 16).
- `generate_corpus`: Deterministic generation using `SplitMix64`.
- `run_bench`: Executes measurements given `BenchConfig`.
- `Report`: Serializable output, includes `to_json` (with `Timings` options) and `to_table`.
- Errors: `unknown_codec`, `invalid_level`, `corpus_mismatch`, `input_too_large`, `roundtrip_failed`, `codec_error`, `io`, `usage`.

## Why not lzbench or squash?

This is a Rust tool whose reports diff cleanly between runs because the deterministic part is separated from the timings.

See [docs/SECURITY.md](docs/SECURITY.md).
