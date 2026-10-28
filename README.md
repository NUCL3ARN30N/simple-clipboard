# Simple Clipboard

A fast, Material Design clipboard history manager written in Rust ([Tauri 2](https://tauri.app)).
On Windows it replaces the built-in <kbd>Win</kbd> + <kbd>V</kbd> clipboard history. It runs in the system tray.

## Features

- **Win + V replacement**: a low-level keyboard hook handles Win+V. If Windows still opens its own clipboard history as well, click *Settings → Shortcut → Disable Windows Win + V*. This adds `V` to `HKCU\…\Explorer\Advanced\DisabledHotkeys` and restarts Explorer. *Restore Windows default* undoes it, and so does the NSIS uninstaller.
- **Text and picture history**, with a configurable size. Pictures can be turned off.
- **Pinned palette**: an optional extra column (left or right) for items you paste often. Pinned items are never trimmed.
- **Colored tags** for pinned items, with tag filter chips in the palette.
- **Search** across all text and tag names.
- **Auto paste**: picking an item pastes it into the app you were using.
- **Material 3 UI** with light, dark and system themes and a choice of accent color.
- **Export and import** of settings, tags and pinned text items as JSON.
- **Update check** against GitHub releases, on startup or on demand.
- Respects the `ExcludeClipboardContentFromMonitorProcessing` flag that password managers set.

## Keyboard

| Key | Action |
| --- | --- |
| <kbd>Win</kbd> + <kbd>V</kbd> | Open/close the popup |
| <kbd>↑</kbd> <kbd>↓</kbd> | Select |
| <kbd>Enter</kbd> | Paste selected item |
| <kbd>Ctrl</kbd> + <kbd>1</kbd>…<kbd>9</kbd> | Paste item by number |
| <kbd>Tab</kbd> | Switch between history and pinned palette |
| <kbd>Ctrl</kbd> + <kbd>P</kbd> | Pin / unpin |
| <kbd>Del</kbd> | Delete item |
| <kbd>Esc</kbd> | Clear search / close |

## Downloads

Builds are on the [releases page](https://github.com/NUCL3ARN30N/simple-clipboard/releases):

- `SimpleClipboard_x.y.z_x64-setup.exe`: Windows installer (per user, no admin needed)
- `SimpleClipboard_x.y.z_x64.msi`: Windows MSI
- `SimpleClipboard_x.y.z_x64_portable.zip`: Windows portable build. Data is stored in a `data` folder next to the exe. Requires WebView2, which ships with Windows 10/11.
- `simple-clipboard_x.y.z_amd64.deb`: Debian/Ubuntu

### Linux notes

- Auto paste uses `xdotool` on X11 (installed as a dependency) or `wtype` on Wayland.
- Wayland does not allow apps to register global shortcuts. Bind the command `simple-clipboard` to a key in your desktop's keyboard settings. Running it a second time opens the popup of the running instance.
