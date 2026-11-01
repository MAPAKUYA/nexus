> Русская пошаговая инструкция без командной строки: [RELEASING.ru.md](RELEASING.ru.md)

# Publishing NEXUS on GitHub

## One-time setup
1. Create an empty repository on GitHub (e.g. `nexus`), public.
2. Replace the `OWNER` placeholder with your GitHub user/organization name:
   ```powershell
   Get-ChildItem -Recurse -File -Include *.md,*.toml,*.json | Where-Object { $_.FullName -notmatch 'node_modules|target' } |
     ForEach-Object { (Get-Content $_ -Raw -Encoding UTF8) -replace 'MAPAKUYA','your-name' | Set-Content $_ -Encoding UTF8 -NoNewline }
   ```
   (or search-and-replace in your editor; files: `README*.md`, `package.json`, `src-tauri/Cargo.toml`).
   Also adjust the copyright holder in `LICENSE`, `src-tauri/tauri.conf.json` and the `authors` fields if you wish.
3. Push:
   ```powershell
   git init
   git add .
   git commit -m "NEXUS 1.0.0"
   git branch -M main
   git remote add origin https://github.com/your-name/nexus.git
   git push -u origin main
   ```
4. In *Settings → Code security* turn on **Private vulnerability reporting** and Dependabot alerts.
5. Open the *Actions* tab: the **CI** workflow should run and be green (Windows build, tests, audits).

## Making a release
1. Bump the version in `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `crates/nexus-core/Cargo.toml`
   (run `cargo update --workspace` to refresh `Cargo.lock`) and add a section to `CHANGELOG.md`.
2. Commit, then tag and push the tag:
   ```powershell
   git tag v1.0.0
   git push origin v1.0.0
   ```
3. The **Release** workflow builds on Windows, runs the tests, creates the installer, a portable zip and
   `SHA256SUMS.txt`, and uploads them to a **draft** release. Open *Releases*, check the files, then **Publish**.

## Code signing (optional, removes the SmartScreen warning over time)
Free signing for open-source projects is available through SignPath Foundation; paid certificates also work.
Tauri supports a custom signing command: `bundle.windows.signCommand` in `src-tauri/tauri.conf.json`.
