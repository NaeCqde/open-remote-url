# Change Log

All notable changes to the `open_remote_url` project are documented below.

## [2026-05-26]

### Added
- **Centralized Config Parsing**: Added `shared/src/config.rs` to unify configuration reads. Environment options are restricted to:
  - **Host**: `HOST`, `PORT`, `PASSPHRASE`
  - **Client**: `HOST_URL`, `RELAY_HOST`, `RELAY_PORT`, `PASSPHRASE`
- **WorkingDirectory Config**: Configured plists, systemd services, and startup codes (`main` entry point) to set the working directory to the parent executable directory. This allows background daemons to correctly locate and parse `.env` files next to the executables.
- **Auto-Start on Installation**: Added `launchctl start` configuration within macOS `installer.rs` routines so that `--install` starts the background daemon immediately.
- **Stdout Status Output**: Replaced GUI popup modules (Win32 MessageBox, AppleScript, zenity/kdialog dialogs) with standard stdout console prints (`println!`). Running either executable without arguments outputs detailed installation and daemon running status to stdout.
- **Clean Registry Cleanup**: Updated `uninstall` script handlers to force stop background binaries and completely sweep leftover browser protocols, launch plists, and systemd units.
- **Term-Symmetrical installer/uninstaller flags**: Implemented dynamic installer logic inside both `open-remote-url` and `open-remote-url-host` using `--install` and `--uninstall` parameters.

### Changed
- **Host / Client Naming**: Renamed the server module to `host` and updated workspace cargo references.
- **Passphrase Renaming**: Renamed `AUTH_TOKEN`/`auth_token` authentication references to `PASSPHRASE`/`passphrase`.
- **Removed RELAY_URL env**: Removed `RELAY_URL` environment overrides in favor of dynamic local network IP detection.
