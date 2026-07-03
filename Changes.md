# Changes

## [2026-07-04]

### Fixed

- **macOS: double-clicking the installed .app showed "Application Not Responding" while the daemon was already running**, for both sender and receiver. Cause: once the `--daemon` process is checked in with Launch Services, a later double-click is routed to *it* as a `kAEOpenApplication`/`kAEReopenApplication` Apple Event instead of spawning a fresh process; neither daemon had a handler for that event (sender's is `Accessory`/windowless, receiver's daemon never pumped a run loop at all), so the event went unanswered and Finder eventually reported the app as hung. Added `shared::mac_apple_events::install_open_application_handler()`, installed by both daemons, which spawns a fresh instance of the executable on that event (that fresh process takes the normal GUI branch and shows the status window). Receiver's `--daemon` startup was also restructured to match the sender's: tokio server on a background thread, `NSApplication` run loop on the main thread, so Apple Events are actually pumped.

## [2026-07-03]

### Changed

- **`load_env` now loads `inactive.env` as the lowest-priority source** when neither the OS config `.env` (installed) nor a cwd `.env` exist. This lets the GUI panel and CLI status display meaningful defaults before installation, using the `inactive.env` bundled next to the app.
- **Sender GUI panel** now shows two additional rows: `Custom Schemes` (comma-separated list, or "(none)") and `HTTP/HTTPS` (Registered / Not registered).
- **CLI status output** (`--` / no-arg) now matches the GUI panel exactly for both sender and receiver: always shows Executable, Configuration, and all URL/auth/scheme fields regardless of install state. Executable shows "(Will be set upon installation)" when not installed.
- Fixed `load_env("client")` typo in `sender/src/main.rs` daemon startup path (was passing wrong app_type "client", now correctly "sender").
