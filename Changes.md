# Changes

## [2026-07-03]

### Changed

- Renamed "client" to "sender" and "host" to "receiver" throughout the entire codebase (source, configs, READMEs, binary names, bundle IDs, crate names, directory names). No backward compatibility.
- Renamed env var `RELAY_URL` to `SELF_URL` everywhere (Rust struct field `relay_url` → `self_url`, `.env` files, README docs).
