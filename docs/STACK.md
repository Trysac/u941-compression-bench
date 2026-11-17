# Stack

## Used

| Tool | Why |
|---|---|
| Rust stable, edition 2024 | `rust-version` 1.88; one toolchain covers build, test, docs and format. |
| Cargo + Cargo.lock | Built-in dependency management with crates.io checksums; gates build with `--locked`. |
| rustfmt + clippy | The canonical formatter and linter (`all` and `pedantic`, `-D warnings`). |
| cargo test + proptest | Integration, doc and property-based tests. |
| cargo-llvm-cov | Line coverage from rustc's own instrumentation; runs on Linux, Windows and in containers. |
| cargo-audit | RustSec advisories for every crate in `Cargo.lock`. |
| Semgrep CE + OSV-Scanner | The factory's own rules; OSV advisories over `Cargo.lock`. |
| gitleaks | Secret scanning locally and in CI. |
| GitHub Actions + Dependabot | Thin CI wrapper over the local gates; weekly dependency updates. |

## Discarded

| Tool | Why not |
|---|---|
| cargo-tarpaulin | Its default engine needs ptrace (a seccomp change in Docker); llvm-cov does not. |
| cargo-deny | cargo-audit covers advisories; licenses and bans can be added via ADR. |
| cargo-fuzz | Needs nightly Rust; add a `fuzz/` target via ADR when a parser warrants it. |
| Makefile / shell scripts for gates | Not portable to Windows; the gates are a Rust program in `scripts/check`. |
