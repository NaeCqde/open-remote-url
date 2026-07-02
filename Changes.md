# Changes

## [2026-07-03]

### Changed

- **Apple Events are now received inside the client daemon** (macOS): Previously, when macOS launched the app as a URL scheme handler, a separate short-lived process captured the `kAEGetURL` Apple Event and forwarded the URL to the daemon over HTTP. This spawned an extra process on every URL open. Now the daemon itself is the Apple Event target: in `--daemon` mode the main thread runs `CFRunLoopRun()` to receive `kAEGetURL` events, while the tokio HTTP server runs on a background thread. Received URLs are bridged into the async world through an mpsc channel, so no additional process is created.

### Added

- **`client/src/url_event.rs`** (macOS only): dedicated Apple Event module. Exposes `listen()` (create the URL channel), `install_handler()` (register the `kAEGetURL` handler), `take_receiver()` (hand the receiver to `daemon::run()`), `run_forever()` (block the main thread in `CFRunLoopRun`), and `send_to_self()` (test-only synthetic event dispatch).

### Removed

- **Inline `url_event` module and launch-time `capture()` forwarding** from `client/src/main.rs`: the old approach that briefly spun the run loop in a separate process and POSTed the URL to the daemon is gone. `main()` is no longer `#[tokio::main]`; on macOS `--daemon` it runs the Apple Event loop on the main thread and the tokio runtime on a background thread, and otherwise builds a standard tokio runtime for the CLI/GUI paths.

### Tests

- **Unit test** (`client/src/main.rs`, `apple_event_url_roundtrip`): updated to the new channel-based API — `listen()` + `install_handler()`, send a synthetic `kAEGetURL` event with `kAEWaitReply` (synchronous same-process dispatch), and assert the URL arrives on the receiver from `take_receiver()`. Runs on a single machine with no external dependencies.
