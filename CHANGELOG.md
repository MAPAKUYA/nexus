# Changelog

## 1.1.0 — 2026-11-09
- **Excluded apps:** time in an excluded app is not tracked at all (neither in focus nor running).
  Excluding an app permanently deletes all statistics already collected for it; the app asks for confirmation first.
  Add apps from Settings → Excluded apps (known and currently running apps, with search), or with the optional
  quick ⊘ button in the app list (Settings → Quick exclude button). Removing an app from the list does not bring
  the deleted data back.
- **Light theme:** Settings → Theme: System (default, follows Windows), Light or Dark. The window frame and its
  background follow the chosen theme, so there is no dark flash when the window opens.
- **Custom date range:** new "Custom" tab with From / To dates and quick buttons for the last 7 / 30 / 90 days.
  Up to about 50 years; ranges longer than 92 days are shown by month.
- **App icons (optional):** Settings → Show app icons shows each app's icon left of its name. The icon is read from
  the program's .exe with the Windows API, converted to a small PNG by a built-in encoder (no new dependencies) and
  stored only in the local database. Store (UWP) apps may show a shared icon; apps without an icon in their file
  show a neutral placeholder. Icons appear a few seconds after an app is first seen. When the setting is off,
  nothing is read or drawn.
- **Animations** (Settings → Animations, on by default; the Windows "show animations" setting is not used, only
  this switch): the highlight of the active tab and of the 7 / 30 / 90 buttons glides between buttons and fades
  out when the dates match no preset; the top-apps bar changes its segments smoothly; Statistics ↔ Settings
  cross-fade; the exclude dialog fades in and out; new icons fade in; the status dot and theme changes are smooth.
  Nothing animates at startup or on the periodic data refresh, nothing runs continuously, and nothing runs while
  the window is closed to the tray.
- The toolbar is more compact and the content area slightly wider (1000 px), so the date picker stays on one line.
- Database schema v3 (new tables `ignored_apps` and `app_icons`; existing data is migrated automatically).
  Going back to an older version after this is not supported — keep a copy of
  `%APPDATA%\app.nexus.tracker\nexus.sqlite3` if you want to be able to.
- No new Tauri permissions, no network code, no new dependencies.

## 1.0.1
- **License changed from MIT to GPL-3.0-only** (Copyright (c) 2026 MAPAKUYA). Version 1.0.0 stays available under MIT
  for everyone who received it.
- Funding links added (Boosty, DonationAlerts) on the project page; the app itself is unchanged.
- CI: configuration files are checked for an accidental BOM (a stray byte-order mark broke a build once).

## 1.0.0
First public release.
- Focus time and running time per app, tracked separately; idle and locked-screen detection.
- Day / week / month / year / all-time statistics, history, charts, per-app breakdown.
- Tray mode with window release (low memory), autostart, close-to-tray option, single instance.
- 8 interface languages; installer with language selection (per-user, no admin rights).
- Fully local: no network code. Data in a local SQLite file; recovery from a damaged file.
