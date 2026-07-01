# Changes

## [2026-07-02]

### Fixed

- **macOS inactive.env path under App Translocation**: Replaced the previous workaround (which returned the OS config path) with a proper call to `SecTranslocateCreateOriginalPathForURL` (Security framework). `find_inactive_env_path` now resolves the real `.app` bundle location even when macOS Gatekeeper has remounted the app at a temporary path, so the GUI always shows the correct `inactive.env` path next to the original `.app` in the download folder.

### Changed

- **`load_env` priority order** (reverted and simplified): inactive.env is no longer read at runtime — it is promoted to the OS config folder on install. Runtime priority is now:
  1. `.env` in the current working directory (CLI / dev usage)
  2. `.env` in the OS-specific installed config folder
