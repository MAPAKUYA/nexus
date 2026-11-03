# NEXUS

**A private, fully local app-usage tracker for Windows 10/11.**
See how much time you spend in each application — what was in focus and what was merely running —
without any account, cloud, telemetry or internet connection.

[Русская версия](README.ru.md)

## Features
- **Two separate clocks per app:** time in focus (you are actually using it) and time the app was running.
- **Idle and lock detection:** time with no mouse/keyboard input and a locked screen are not counted
  (idle counting can be switched on in Settings, e.g. for watching videos).
- **Day / Week / Month / Year / All time**, date picker, history by day and by month, activity chart and
  a focus-time breakdown by app.
- **Lives in the tray:** minimizing closes the window and frees its memory; tracking continues.
  Optional autostart (minimized to the tray).
- **Survives shutdown, sleep and restarts:** data is flushed every 30 s, on exit and when Windows ends the session.
- **8 interface languages:** English, Русский, Español, Deutsch, Français, Português (Brasil), 简体中文, 日本語.
- Light on resources: ~0 % CPU; in the tray only the small NEXUS process stays alive.

## Privacy
- Works **100 % offline**. NEXUS contains no network code, no analytics, no crash reporting, no updater.
- Collects only the **program name** (e.g. `chrome.exe`) and **how long** it was in focus / running. It never records
  keystrokes, window titles or contents, clipboard, screenshots or files.
- Everything is stored in one local SQLite file: `%APPDATA%\app.nexus.tracker\nexus.sqlite3`.
- No administrator rights needed. No processes are launched.
- Details, audit results and known limitations: [docs/SECURITY.md](docs/SECURITY.md).

## Install
Download from the [Releases](../../releases) page:

| File | For whom |
|---|---|
| `NEXUS_x.y.z_x64-setup.exe` | **Recommended.** Per-user installer (no admin rights), asks for the language, adds a Start menu shortcut and an uninstall entry. |
| `NEXUS-x.y.z-portable-x64.zip` | Unzip anywhere and run `NEXUS.exe`. Turn off *Start with Windows* in Settings before deleting the folder. |
| `SHA256SUMS.txt` | Checksums of the files above. |

Requirements: Windows 10 or 11 (x64), and the *Microsoft Edge WebView2 Runtime* (preinstalled on Windows 11
and current Windows 10; the installer fetches it if it is missing).

> **SmartScreen:** the files are not code-signed yet, so Windows may show *"Windows protected your PC"*.
> Choose *More info → Run anyway*, and compare the file with `SHA256SUMS.txt`
> (`Get-FileHash .\NEXUS_*_x64-setup.exe`). Releases are built by GitHub Actions from this repository's source.

## Using it
- Closing or minimizing: *minimize* sends NEXUS to the tray; the tray icon opens it again (left click) or quits it (right click → *Quit*).
  In *Settings* you can make the close button (×) behave the same way.
- The idle threshold (1–60 min), idle counting, autostart and language are in *Settings*.
- *Settings → Delete all statistics* removes all data.

## Build from source (Windows)
Prerequisites: [Node.js](https://nodejs.org) 20+, [Rust](https://rustup.rs) stable (MSVC toolchain) with the
"Desktop development with C++" build tools, WebView2 Runtime.

```powershell
git clone https://github.com/MAPAKUYA/nexus.git
cd nexus
npm ci
npm run tauri build        # installer: target\release\bundle\nsis\NEXUS_<version>_x64-setup.exe
                           # plain exe:  target\release\nexus.exe
```

Development: `npm run tauri dev`.

Tests and checks:
```powershell
npm test                   # UI logic, formatting, translations completeness
npm run typecheck
cargo test -p nexus-core   # tracking logic, SQLite, reports, settings
cargo audit                # (cargo install cargo-audit) known vulnerabilities in dependencies
npm audit
powershell -ExecutionPolicy Bypass -File .\scripts\verify-windows.ps1 -Seconds 60   # CPU/RAM/network of a running NEXUS
```

## Project layout
```
crates/nexus-core/   platform-independent core: tracking, SQLite, reports, settings (unit-tested)
src-tauri/           Windows shell: Win32 sampling, tray, Tauri commands, installer hooks
src/                 React + TypeScript UI, translations in src/i18n/
scripts/             verify-windows.ps1 (load/network check), cross-build helpers for Linux
docs/                SECURITY.md, PERFORMANCE.md, TRANSLATIONS.md, TEST-REPORT.md
```

## Contributing
Bug reports, translation fixes and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/TRANSLATIONS.md](docs/TRANSLATIONS.md). Security issues: see [SECURITY.md](SECURITY.md).

## Support the author
NEXUS is free. If it is useful to you, you can support its development — entirely optional, nothing is locked behind it:

- [Boosty](https://boosty.to/matemzaz)
- [DonationAlerts](https://www.donationalerts.com/r/matem)

(The app itself never opens links or connects to the internet; donations are only possible from this page.)

## License
NEXUS is free software, released under the [GNU General Public License v3.0](LICENSE) (`GPL-3.0-only`).
Copyright (c) 2026 MAPAKUYA.

You may use, study, change and share it. If you distribute NEXUS (modified or not), you must give the recipients the
source code under the same license and keep the copyright notices. Version 1.0.0 was published earlier under the MIT
license; whoever received that version keeps the MIT rights for it.
