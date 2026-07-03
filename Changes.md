# Changes

## [2026-07-03]

### Changed

- **`load_env` now loads `inactive.env` as the lowest-priority source** when neither the OS config `.env` (installed) nor a cwd `.env` exist. This lets the GUI panel and CLI status display meaningful defaults before installation, using the `inactive.env` bundled next to the app.
- **Sender GUI panel** now shows two additional rows: `Custom Schemes` (comma-separated list, or "(none)") and `HTTP/HTTPS` (Registered / Not registered).
- **CLI status output** (`--` / no-arg) now matches the GUI panel exactly for both sender and receiver: always shows Executable, Configuration, and all URL/auth/scheme fields regardless of install state. Executable shows "(Will be set upon installation)" when not installed.
- Fixed `load_env("client")` typo in `sender/src/main.rs` daemon startup path (was passing wrong app_type "client", now correctly "sender").
