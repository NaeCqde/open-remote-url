# Changes

## [2026-07-02]

### Changed

- **Port forwarding skipped for non-http/https URLs**: When the client receives a custom-scheme URL (e.g. `vcc://`, `unityhub://`), it now sends only the open request to the host and returns immediately — no port history is collected, no `/ports` update is sent, and the 3-second new-port polling loop does not run. Port forwarding continues to work as before for `http://` and `https://` URLs.
