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

- `BROWSER` is written at every Sender start: when the OS default is another browser, it is stored in the installed `.env`
  (overwriting) and the Sender takes over; when the Sender already is the default, `BROWSER` is read (Windows: if nothing is
  set, the first browser registered under StartMenuInternet). `config::set_env_value` edits one key and keeps the rest.
- Uninstall (Sender) restores the default browser from `BROWSER` (else the remembered one) if the Sender is still the default:
  macOS bundle id, Linux `.desktop` id (and removes the Sender's `.desktop`). Windows cannot set it from a program, so the
  Sender's registration is removed and the user is told to pick a browser. `BROWSER` stays in the `.env`.

## Fixed

- Launching the local browser no longer waits for the launcher to exit. On Windows a browser that was not running
  keeps the launcher alive until the browser is closed (measured: 318 s with Brave), which blocked the caller. A launcher
  that fails within 1.5 s is still reported as an error (the URL is then sent to the Receiver).

## Verified

- macOS: detection and launch against Orion. Windows 11: `cargo test -p shared --lib` and a real launch of Brave by
  ProgId (`BROWSER=BraveHTML.<id>`; on Windows the Sender is already the default, so `BROWSER` must be set).
- Not verified: Linux; the startup write of `BROWSER`, the Windows registered-browser fallback and the uninstall restore
  were unit-tested only (set_env_value) and not run on a real machine.
