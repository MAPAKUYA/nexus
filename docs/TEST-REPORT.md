# NEXUS 1.0.0 — test and security report

What was actually run, and what was not. Environment: the build/test machine was Linux; the Windows
binaries were cross-compiled. The public release is built by GitHub Actions on a real Windows runner.

## Automated tests
| Suite | Result |
|---|---|
| `cargo test -p nexus-core` — tracking logic (focus/running separation, idle without roll-back, lock, sleep gaps, midnight), SQLite (upsert without duplicates, migration v1→v2, corrupt-file recovery), periods/dates (leap years, weeks from Monday), reports, settings, tray texts | **63 / 63 passed** |
| `npm test` — formatting, chart series, focus breakdown, completeness of all 8 translations, Rust/TS language lists in sync | **23 / 23 passed** |
| `tsc --noEmit` | no errors |
| Windows build (`--locked`) | succeeded, 0 compiler warnings |
| `cargo clippy` | **not run** (the toolchain available for the build did not provide a matching clippy); CI runs on a current stable toolchain |

## Dependency and source audit
| Check | Result |
|---|---|
| `cargo audit` (RustSec, 420 crates) | **0 vulnerabilities**; 2 warnings (`proc-macro-error` unmaintained, `glib` unsoundness) — both Linux/GTK-only, not part of the Windows build |
| `npm audit` | **0 vulnerabilities** |
| Own source: sockets, HTTP clients, process spawning, keyboard/clipboard/title/screenshot APIs | none found |
| Built `NEXUS.exe`: network DLLs and socket functions in the import table | none |
| Tauri permissions | zero granted; no plugins; no remote IPC |
| UI bundle: `fetch`/`XMLHttpRequest`/`WebSocket`/external resources | none |

## Installer (NSIS) — run under Wine, silent mode
Files and shortcuts installed · uninstall entry written · `HKCU\Software\NEXUS\Language` written ·
**update mode** keeps autostart + language · **real uninstall** removes autostart, language key and entry, keeps statistics ·
language mapping for all 8 languages and an unknown id (11/11 cases) · the app's registry reader returns the right
code / nothing / nothing for missing / garbage / non-string values.
Not covered by Wine: the interactive wizard (language dialog, finish page), SmartScreen, WebView2 bootstrapper.

## Run on a real Windows PC by the project author (60 s, 6 cores)
Window open: 7 processes, 275.5 MB private bytes, 0.15 % CPU, only loopback sockets.
Tray: 1 process, 59.7 MB, 0 % CPU, no sockets. Minimize-to-tray, autostart, idle/lock behavior and all 8 languages
were checked manually and worked.

## Visual checks
UI rendered in a WebKit test browser with mocked data at 720–1280 px widths in English, German, Japanese and
Russian (not the Windows WebView2 engine/fonts; layout check only). Two layout defects were found and fixed
(time columns overflow; toolbar wrapping).

## Not verified / known limitations
- The interactive installer wizard and the installer's WebView2 bootstrapper on a clean Windows machine.
- Behavior on Windows 10 vs 11 differences, multi-monitor/high-DPI beyond what was tested manually.
- Translations were written by an AI and are not reviewed by native speakers.
- No code signing yet (SmartScreen warning is expected); the database is not encrypted.
