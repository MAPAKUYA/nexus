# Contributing

Thanks for helping! Small, focused pull requests are easiest to review.

## Before you open a pull request
```powershell
npm ci
npm test && npm run typecheck
cargo test -p nexus-core
```
CI runs the same checks plus `cargo audit` / `npm audit` and a Windows build.

## Principles (please keep them)
- **Offline and private:** no network code, telemetry or new data collection. Collect names and durations only.
- **Few dependencies:** justify every new crate/package; prefer the standard library.
- **Everything user-visible is translated:** add texts to every file in `src/i18n/` (the compiler and tests will remind you).
- **Core logic belongs in `crates/nexus-core`** (no Windows/Tauri code) so it can be unit-tested.
- Document every `unsafe` block with a `SAFETY:` comment.

Translations: see [docs/TRANSLATIONS.md](docs/TRANSLATIONS.md).
