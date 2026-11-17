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

## Not verified

- Windows and Linux browser detection/launch (written, not run). macOS detection and launch were run against Orion.
