# CLAUDE.md

## Git commits and GitHub

- Always commit as **NUCL3ARN30N** with the email **63464047+NUCL3ARN30N@users.noreply.github.com**:
  ```sh
  git config user.name "NUCL3ARN30N"
  git config user.email "63464047+NUCL3ARN30N@users.noreply.github.com"
  ```
  Check `git config user.name` / `user.email` before committing. If they differ, set them in the repo or pass `--author="NUCL3ARN30N <63464047+NUCL3ARN30N@users.noreply.github.com>"`.
- Never contribute as Claude:
  - no `Co-Authored-By: Claude …` trailers
  - no "Generated with Claude Code" lines in commit messages, PR descriptions, issues or comments
  - no Claude/Anthropic author or committer identity

## Project

Simple Clipboard is a clipboard history manager built with Tauri 2. The Rust backend is in `src-tauri/src/`. The frontend in `src/` is plain HTML/CSS/JS with a Material 3 look and no bundler. It replaces Win+V on Windows and lives in the tray.

- `npm run dev`: run locally
- `npx tauri build --bundles nsis,msi`: Windows release build (the `.deb` is built by CI on Ubuntu)
- `cargo test --manifest-path src-tauri/Cargo.toml`: unit tests
- Local test builds go to `E:\Nextcloud\8_Coding\Github\_builds\simple-clipboard_<version>\`. That folder holds the setup.exe, the MSI, the portable zip, and an unzipped portable folder that contains a `portable` marker file.
- Pushing a `v*` tag makes `.github/workflows/build.yml` publish a GitHub release.

## Gotchas

- Never build a `WebviewWindow` on the main thread (sync commands, menu/tray handlers). It deadlocks on Windows. Build it from a worker thread instead (see `open_settings_window`).
- Starting an OS window drag sends a spurious blur event on Windows. `popup::on_blur` checks the foreground window before hiding.
- Hiding Windows' own Win+V uses the `DisabledHotkeys` value under `HKCU\…\Explorer\Advanced` and needs an Explorer restart. The NSIS uninstall hook restores it.
- CSS: `[hidden]` is forced to `display: none !important` in `theme.css`. Keep it, because components set their own `display`.
- The in-app updater picks release assets by suffix (`-setup.exe`, `.msi`, `_portable.zip`, `_amd64.deb`; see `InstallKind::asset_suffix`). Keep these in sync with the file names in `build.yml`.
