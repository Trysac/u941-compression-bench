# Security notes

## Threat Model

### Defended

- **Untrusted Input**: Input strings for codec specifications and CLI arguments are validated; invalid input returns explicit error codes (e.g., `unknown_codec`, `usage`) rather than panicking or crashing.
- **Resource Exhaustion**: Decompression capacity is capped at 1 GiB per operation to prevent decompression bombs. Input corpus sizes are capped at 1 GiB (`DEFAULT_MAX_TOTAL_BYTES`). Manifest file size is capped at 16 MiB.
- **Supply Chain**: Uses `Cargo.lock` with checksums; `unsafe` code is forbidden.

### Not Defended

- **Hostile Codec Implementations**: The crate calls third-party libraries (`zstd`, `lz4_flex`, `brotli`, `flate2`). Vulnerabilities within these specific underlying C-bindings or logic are inherited.
- **Arbitrary File Access**: The tool will read any file path provided via CLI or manifest if permissions allow.
