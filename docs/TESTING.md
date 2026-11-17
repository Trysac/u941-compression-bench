# Testing

## Running

```sh
cargo test                                                # unit, integration, property and doc tests
cargo llvm-cov                                            # the same with a coverage summary
cargo run --locked --manifest-path scripts/check/Cargo.toml   # every gate, as CI and the factory run it
```

## Policy

- Line coverage of the library (measured by `cargo llvm-cov`) must stay at or above 80 %; the gate
  fails below it.
- Integration tests in `tests/` exercise the public API only. They assert every observable effect:
  returned values, returned errors (`assert_eq!(f(x), Err(Error::...))`) and their messages.
- Property tests (`proptest!`) state invariants such as idempotence or round trips; the README
  example runs as a doc test.
- Each bug fix starts with a failing test that reproduces it.
- Tests are deterministic: no network, no wall clock, no dependence on test order.

## Tiers

- Local tier (every gate run, coverage measured here): unit, integration, property and doc tests, plus
  conformance on small fixtures kept in the repository.
- CI tier: conformance that needs an external tool or a download. Start the file under `tests/` with
  `#![cfg(feature = "conformance")]`; `scripts/check` enables the `conformance` feature only when
  `FABRICA_CI=1`, while clippy always compiles it. The CI `conformance` job sets `FABRICA_CI=1` and runs
  the full suite. Tools are installed by the optional `ci/conformance-setup.sh` (bash, CI only; verify
  every download against a pinned SHA-256), and tests skip cleanly when their tool is missing.

## Layout

Integration tests live in `tests/`, one file per area; private helpers may have `#[cfg(test)]` unit
tests next to them. Fixtures and golden files go in `tests/fixtures/`.
