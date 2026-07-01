# Changes

## [2026-07-02]

### Fixed

- **macOS installer ignored HTTP=false for Info.plist**: `setup_macos_app_bundle` was unconditionally writing `http` and `https` into `CFBundleURLTypes` regardless of the `HTTP` env setting. It now reads `ClientConfig` (which is already loaded from `.env` at install time) and writes only the schemes that are actually configured.

- **load_env CLI fallback order**: Clarified and extended the `.env` search order for non-installed (CLI / dev) usage:
  1. Installed config dir (OS-specific, e.g. `~/Library/Application Support/…`)
  2. `inactive.env` bundled alongside the executable / `.app`
  3. `.env` in the current working directory
  4. `inactive.env` in the current working directory
  5. Walk parent directories for `.env` (standard dotenvy behaviour)

  Previously step 3/4 were missing, so running `--daemon` from a directory containing `.env` or `inactive.env` could fall through to dotenvy's parent-walk instead of picking up the file in the exact working directory first.
