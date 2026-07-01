# Changes

## [2026-07-02]

### Fixed

- **macOS inactive.env path under App Translocation**: When the client `.app` is run from a Gatekeeper-translocated location (`AppTranslocation` in path), `find_inactive_env_path` now returns the install-time config path (`~/Library/Application Support/open-remote-url/client/.env`) instead of a meaningless temporary path. This aligns client behavior with the already-installed host, which already returns the config dir path.

### Added

- **GUI panel: click-to-copy values**: All value fields in the GUI control panel (Executable Path, Configuration, Host URL, Client URL, Relay URL) now copy their text to the clipboard when clicked. A pointer cursor and "Click to copy" tooltip indicate the interaction.
