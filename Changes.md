# Changes

## [2026-07-04]

### Fixed

- **macOS: double-clicking the installed .app showed "Application Not Responding" while the daemon was already running**, for both sender and receiver. Cause: once the `--daemon` process is checked in with Launch Services, a later double-click is routed to *it* (kAEOpenApplication/kAEReopenApplication) instead of spawning a fresh process, and the daemon can't visibly respond (sender is `Accessory`/windowless, receiver never pumps a run loop). First attempt intercepted that Apple Event on the daemon and spawned a helper process, but this caused a repeated-spawn/focus-stealing loop every few seconds and was reverted. Fixed properly by setting `LSMultipleInstancesProhibited = false` in the generated `Info.plist` (`shared/src/installer.rs`, `shared/post_build.rs`), so Launch Services always spawns a fresh process on double-click instead of routing to the daemon — the fresh process already has a fallback path that shows the GUI/forwards a pending URL, so no daemon-side change is needed. **Requires reinstalling** (Reinstall/Update button, or `--install`) since the plist is written at install time.

## [2026-07-03]

### Changed

- **`load_env` now loads `inactive.env` as the lowest-priority source** when neither the OS config `.env` (installed) nor a cwd `.env` exist. This lets the GUI panel and CLI status display meaningful defaults before installation, using the `inactive.env` bundled next to the app.
- **Sender GUI panel** now shows two additional rows: `Custom Schemes` (comma-separated list, or "(none)") and `HTTP/HTTPS` (Registered / Not registered).
- **CLI status output** (`--` / no-arg) now matches the GUI panel exactly for both sender and receiver: always shows Executable, Configuration, and all URL/auth/scheme fields regardless of install state. Executable shows "(Will be set upon installation)" when not installed.
- Fixed `load_env("client")` typo in `sender/src/main.rs` daemon startup path (was passing wrong app_type "client", now correctly "sender").
