# Security notes

## Threat model

`trysac-compression-bench` is a pure library without I/O. The main risks are:

- **Supply chain**: a compromised dependency or CI action. Mitigation: standard library only,
  `Cargo.lock` with crates.io checksums and `--locked` builds, `cargo-audit` (RustSec) and
  OSV-Scanner in the gates, Dependabot weekly updates, actions pinned by commit SHA, minimal
  workflow permissions.
- **Leaked secrets**: tokens committed by mistake. Mitigation: `gitleaks` runs in the local gates
  and in CI; `.env` files are git-ignored.
- **Insecure code**: memory unsafety, disabled TLS verification, command injection. Mitigation:
  `unsafe_code = "forbid"` in `Cargo.toml`, clippy with `-D warnings`, and the factory's Semgrep
  rules (`.semgrep/factory-rules.yml`, `nosemgrep` ignored); fix the code instead of silencing it.
- **Untrusted input**: callers may pass hostile strings. Mitigation: validate input and return
  explicit errors; never panic on input; never spawn a shell with untrusted data.

## Publishing

Releases are git tags; publishing to crates.io needs a registry token, which is never stored in the
repository.

To report a vulnerability, see [SECURITY.md](../SECURITY.md).
