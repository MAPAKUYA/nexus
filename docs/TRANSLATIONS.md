# Translations / Переводы

NEXUS ships with 8 interface languages: English (default), Русский, Español, Deutsch, Français,
Português (Brasil), 简体中文, 日本語.

**Quality note.** The translations were written by an AI assistant and have not been reviewed by native
speakers. Corrections are very welcome — open an issue or a pull request.

## Where the texts live
- Interface: `src/i18n/<code>.ts` (one file per language, all implement the `Dict` type from `src/i18n/types.ts`,
  so the compiler refuses a missing string).
- Tray menu and tooltip: `crates/nexus-core/src/i18n.rs`.
- Dates, months and weekdays come from the system (`Intl`), not from the dictionaries.

## Adding a language (example: Italian, code `it`)
1. Copy `src/i18n/en.ts` to `src/i18n/it.ts`, rename the export to `it` and translate the values.
2. Register it in `src/i18n/index.ts`: add `"it"` to the `Lang` type, an entry in `LANGUAGES`
   (native name and `Intl` locale, e.g. `it`) and in `DICTS`.
3. Add `"it"` to `SUPPORTED_LANGUAGES` in `crates/nexus-core/src/settings.rs` (and bump the array length)
   and a `"it" => TrayText { … }` line in `crates/nexus-core/src/i18n.rs`.
4. Run `npm test` and `cargo test -p nexus-core` — they check that every language is complete, that the
   language lists in TypeScript and Rust match, and that the tray texts exist.
5. For the installer: add the language to `bundle.windows.nsis.languages` in `src-tauri/tauri.conf.json` (NSIS
   language name, e.g. `Italian`) and map its Windows language id (LCID, e.g. 1040) to the code in
   `src-tauri/installer-hooks.nsh`.

## First-run language
- Portable build: English.
- Installer build: the language chosen in the installer (stored by the installer in
  `HKCU\Software\NEXUS\Language`); can be changed any time in Settings.
