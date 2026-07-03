# Changes

## [2026-07-04]

### Fixed

- **macOS: window focus was stolen every few seconds while the sender daemon was running (worse after registering a custom URL scheme).** Root cause: the LaunchAgent plist launches the daemon via `/usr/bin/open -a <bundle> --args --daemon` (needed so Launch Services checks it in as the running bundle instance for Apple Event routing) combined with `KeepAlive = true`. `open` exits immediately after asking Launch Services to start/activate the app — it does not stay running — so launchd saw its child exit right away and relaunched `open -a ...` in a tight loop, each iteration re-activating/focusing the app. Fixed by changing `KeepAlive` to `{SuccessfulExit: false}` (`shared/src/installer.rs`) so launchd only relaunches `open` on a genuine failure, not its normal successful exit.
- **macOS: double-clicking the installed .app while the daemon was already running showed "Application Not Responding" (sender/receiver) or did nothing visible.** `LSMultipleInstancesProhibited = false` in the generated `Info.plist` turned out to be a no-op here (multiple instances are already the default; Finder still tries to *activate* an existing instance of the same bundle ID rather than spawning fresh on double-click). The real fix: `shared::mac_apple_events::install_open_application_handler()`, installed by both daemons, spawns a fresh instance of the executable when the daemon receives `kAEOpenApplication`/`kAEReopenApplication` (the event Finder sends on double-click); that fresh process takes the normal GUI branch and shows the status window. This was tried once before and reverted because it appeared to cause a focus-stealing loop — that loop was actually the separate `open -a`/`KeepAlive` bug above, now fixed, so this is safe to reapply. Receiver's `--daemon` startup was restructured to match the sender's (tokio on a background thread, `NSApplication` run loop on the main thread) so Apple Events are actually pumped and delivered.

## [2026-07-03]

### Changed

- **`load_env` now loads `inactive.env` as the lowest-priority source** when neither the OS config `.env` (installed) nor a cwd `.env` exist. This lets the GUI panel and CLI status display meaningful defaults before installation, using the `inactive.env` bundled next to the app.
- **Sender GUI panel** now shows two additional rows: `Custom Schemes` (comma-separated list, or "(none)") and `HTTP/HTTPS` (Registered / Not registered).
- **CLI status output** (`--` / no-arg) now matches the GUI panel exactly for both sender and receiver: always shows Executable, Configuration, and all URL/auth/scheme fields regardless of install state. Executable shows "(Will be set upon installation)" when not installed.
- Fixed `load_env("client")` typo in `sender/src/main.rs` daemon startup path (was passing wrong app_type "client", now correctly "sender").
