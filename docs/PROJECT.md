# Project record

## Objective

Benchmark CLI that compares zstd, lz4, brotli and gzip on your data for ratio, throughput and peak memory, with seeded corpora pinned by SHA-256 for reproducible reports.

## Scope

- In scope: Benchmarking via CLI or library, seeded deterministic corpus generation (text, json, random, zeros), manifest-based file pinning via SHA-256, round-trip verification, and reporting (JSON/Table).

## Out of Scope

- Multithreaded compression.
- Dictionaries.
- Streaming over files larger than memory.
- Support for xz or snappy.
- Plots.

## Timeline

| Date | Event |
|---|---|
| 2026 | Repository created from the factory utility template. |

## Effort metrics

| Metric | Value |
|---|---|
| Tokens (input / output) | 2001230 / 38803 |
| Hours (human) | pending measurement |
| Cost (USD) | 0.9697 |

## Factory run u941 (measured from API usage)

| Date (UTC) | Phase | Model | Batch | Input | Output | Cache write | Cache read | Cost (USD) |
|---|---|---|---|---|---|---|---|---|
| 2026-11-17 | generation | google/gemini-4-argon | yes | 6349 | 26364 | 0 | 0 | 0.5527 |
| 2026-11-17 | fix-1 | google/gemini-3.8-flash | no | 154038 | 9464 | 0 | 923663 | 0.2203 |
| 2026-11-17 | fix-2 | google/gemini-3.8-flash | no | 35979 | 319 | 0 | 82621 | 0.0344 |
| 2026-11-17 | fix-3 | google/gemini-3.8-flash | no | 43518 | 441 | 0 | 97387 | 0.0416 |
| 2026-11-17 | audit | google/gemini-3.8-flash | no | 91149 | 909 | 0 | 540759 | 0.1123 |
| 2026-11-17 | docs | google/gemini-3.1-flash-lite | no | 25767 | 1306 | 0 | 0 | 0.0084 |

Total: $0.9697. Prices: vertex line, as measured by its adapter. Providers serve the current version of each model id, so the date tells which version answered; versions, launch stages and price sources per line are in the factory's model register (docs/MODELS.md).
