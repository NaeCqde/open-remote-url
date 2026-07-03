# Changes

## [2026-07-04]

### Fixed

- **macOS: window focus was stolen every few seconds while the sender daemon was running (worse after registering a custom URL scheme), and double-clicking the installed .app sometimes showed "Application Not Responding".** Root cause: the LaunchAgent plist launches the daemon via `/usr/bin/open -a <bundle> --args --daemon` (needed so Launch Services checks it in as the running bundle instance for Apple Event routing) combined with `KeepAlive = true`. `open` exits immediately after asking Launch Services to start/activate the app — it does not stay running — so launchd saw its child exit right away and relaunched `open -a ...` in a tight loop, each iteration re-activating/focusing the app. Fixed by changing `KeepAlive` to `{SuccessfulExit: false}` (`shared/src/installer.rs`) so launchd only relaunches `open` on a genuine failure, not its normal successful exit.
- Also set `LSMultipleInstancesProhibited = false` in the generated `Info.plist` (all three places it's written: `shared/src/installer.rs`, `shared/post_build.rs`, `shared/src/scheme_handler.rs::rewrite_info_plist` which runs on every daemon start and was clobbering the installer's copy) so a double-click always spawns a fresh process instead of Launch Services routing the event to the already-checked-in daemon, which can't visibly respond. An earlier attempt intercepted the Apple Event on the daemon itself and spawned a helper process, but that caused the same focus-stealing symptom (actually caused by the `open -a`/`KeepAlive` loop above) and was reverted. **Requires reinstalling** (Reinstall/Update button, or `--install`) since all of the above is written at install/daemon-start time.

## [2026-07-03]

### Changed

- **`load_env` now loads `inactive.env` as the lowest-priority source** when neither the OS config `.env` (installed) nor a cwd `.env` exist. This lets the GUI panel and CLI status display meaningful defaults before installation, using the `inactive.env` bundled next to the app.
- **Sender GUI panel** now shows two additional rows: `Custom Schemes` (comma-separated list, or "(none)") and `HTTP/HTTPS` (Registered / Not registered).
- **CLI status output** (`--` / no-arg) now matches the GUI panel exactly for both sender and receiver: always shows Executable, Configuration, and all URL/auth/scheme fields regardless of install state. Executable shows "(Will be set upon installation)" when not installed.
- Fixed `load_env("client")` typo in `sender/src/main.rs` daemon startup path (was passing wrong app_type "client", now correctly "sender").
