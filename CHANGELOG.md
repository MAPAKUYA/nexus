# Changelog

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
