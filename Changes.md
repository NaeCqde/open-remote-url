# Changes

## [2026-07-03]

### Fixed

- **macOS URL scheme handler opened GUI instead of forwarding URL**: When macOS launches `OpenRemoteURLClient.app` as the registered URL scheme handler (e.g. `open steam://install/4084250`), the URL is delivered as a `kAEGetURL` Apple Event — not as a command-line argument. The app previously ignored the Apple Event and opened the GUI panel instead. Added `url_event::capture()` in `client/src/main.rs` that registers a `kAEGetURL` handler and spins the CFRunLoop briefly (200 ms) to receive the event before any GUI setup. If a URL is captured, it is forwarded to the running client daemon via `POST /open` and the process exits.

### Added

- **`url_event` module** (`client/src/main.rs`, macOS only): Apple Event handler for `kInternetEventClass`/`kAEGetURL`. Exports `install_handler()`, `capture()`, and `send_to_self()` (used in tests).
- **Unit test** (`client/src/main.rs`): `apple_event_url_roundtrip` — installs the handler, sends a synthetic `kAEGetURL` event with `kAEWaitReply` (synchronous same-process dispatch), and asserts the URL is returned by `capture()`. Runs on a single machine with no external dependencies.
- **Integration test** (`client/tests/url_scheme_test.rs`, macOS only): `open_command_url_forwarded_end_to_end` — starts a mock host HTTP server, spawns the client daemon (via temp `.env`), calls `open -b quest.nae.open-remote-url.client steam://install/4084250`, and asserts the mock host received the URL. Skipped automatically if the app bundle is not installed or port 30000 is occupied.
