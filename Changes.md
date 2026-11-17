# Changes

2026-11-17

## Added

- Sender: route http/https URLs by host. `DOMAINS` (exact host, `*.example.com`, IP, CIDR) with `DOMAIN_MODE`
  (`exclude`: listed hosts open locally, the rest is sent; `include`: only listed hosts are sent). Other schemes are always sent.
- Sender: `BROWSER` and automatic remembering of the real default browser (taken just before the Sender replaces the
  OS default for http/https). Local URLs are launched directly in that app (macOS `open -b`, Linux `gtk-launch`,
  Windows ProgID command), so they never loop back to the Sender. If no browser is known or the launch fails, the URL is sent.
- `shared/src/domain_filter.rs`, `shared/src/local_browser.rs` (unit tests included), status output, `sender/.env`,
  `docs/SPEC.md`, `README.md`, `README.en.md`.

## Fixed

- Launching the local browser no longer waits for the launcher to exit. On Windows a browser that was not running
  keeps the launcher alive until the browser is closed (measured: 318 s with Brave), which blocked the caller. A launcher
  that fails within 1.5 s is still reported as an error (the URL is then sent to the Receiver).

## Verified

- macOS: detection and launch against Orion. Windows 11: `cargo test -p shared --lib` and a real launch of Brave by
  ProgId (`BROWSER=BraveHTML.<id>`; on Windows the Sender is already the default, so `BROWSER` must be set).
- Not verified: Linux.
