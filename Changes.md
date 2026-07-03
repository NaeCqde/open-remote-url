# Changes

## [2026-07-03]

### Changed

- **`load_env` now merges all three config sources** instead of stopping at the first match. Priority (highest wins): 3. current directory `.env` > 2. OS config folder `.env` > 1. existing process environment variables. All three are always read; a variable present in a higher-priority source overrides the same variable from a lower-priority source.
