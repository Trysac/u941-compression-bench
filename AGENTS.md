# AGENTS.md

Instructions for coding agents (and humans) working on the crate `trysac-compression-bench` (library name
`compression_bench`, so callers write `use compression_bench::...`).

## Commands

| Task | Command |
|---|---|
| Fetch dependencies | `cargo fetch --locked` |
| Run every gate (must pass before a PR) | `cargo run --locked --manifest-path scripts/check/Cargo.toml` |
| Tests only | `cargo test` |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` |
| Coverage | `cargo llvm-cov` |

The gate sequence is: `cargo fmt --check`, `cargo clippy` (`-D warnings`, every target and feature),
Semgrep (the factory's rules in `.semgrep/`), tests under `cargo llvm-cov` with line coverage >= 80 % of
the library plus the doc tests, documentation manifest (`docs-manifest.json`), gitleaks, `cargo audit`
and OSV-Scanner over `Cargo.lock`. It stops at the first failure.

## Folder map

| Path | Content |
|---|---|
| `src/lib.rs` | The library root; its public items are the public API. Split code into modules under `src/`. |
| `tests/` | Integration tests through the public API (`tests/api.rs`), property tests with proptest. |
| `tests/fixtures/` | Optional fixtures and golden files used by tests. |
| `scripts/check/` | The single definition of the quality gates (its own crate, never published). |
| `docs/` | Architecture, decisions (ADRs), testing, security, project record, stack. |
| `fabrica.json` | Install and check commands used by the factory and CI. |
| `docs-manifest.json` | Documentation files the docs gate requires. |
| `.github/` | CI wrapper and Dependabot configuration. |

## Conventions

- Rust stable, edition 2024, `rust-version` in `Cargo.toml`; formatted with rustfmt, clean under
  clippy (`all` and `pedantic`); `unsafe` code is forbidden.
- Standard library only unless an ADR in `docs/decisions/` justifies a dependency.
- Every public item has a doc comment; errors are returned as `Result` with a crate error type,
  never panics on input, and fallible functions document `# Errors`.
- Write the failing test first; integration tests in `tests/` use every literal of the acceptance items
  verbatim, and property tests (`proptest!`) cover invariants.
- CI-only conformance tests (external tools or downloads) start with `#![cfg(feature = "conformance")]`
  in a file under `tests/`; only CI enables the feature.
- Every behavior change updates `README.md` usage, `CHANGELOG.md` (Unreleased) and, if it is a
  design choice, a new ADR.
- Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`).

## Limits

- Never push to `main`; work on a branch and open a pull request.
- Do NOT edit `.github/`, `fabrica.json`, `docs-manifest.json`, `scripts/`, `Cargo.toml`, `Cargo.lock`,
  `build.rs` or any tool configuration (`rustfmt.toml`, `clippy.toml`, `.cargo/`).
  They are the quality contract; changes to them are made by a human.
- Never lower the coverage threshold, add `#[allow(...)]` to silence clippy, or skip a test to make a
  gate pass.
- Never commit secrets, tokens or `.env` files; gitleaks blocks them.
