# Simple Clipboard

[![GitHub stars](https://img.shields.io/github/stars/NUCL3ARN30N/simple-clipboard?style=for-the-badge)](https://github.com/NUCL3ARN30N/simple-clipboard/stargazers)
[![GitHub forks](https://img.shields.io/github/forks/NUCL3ARN30N/simple-clipboard?style=for-the-badge)](https://github.com/NUCL3ARN30N/simple-clipboard/network/members)
[![GitHub issues](https://img.shields.io/github/issues/NUCL3ARN30N/simple-clipboard?style=for-the-badge)](https://github.com/NUCL3ARN30N/simple-clipboard/issues)
[![GitHub license](https://img.shields.io/github/license/NUCL3ARN30N/simple-clipboard?style=for-the-badge)](LICENSE)

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/Tauri%202-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)
![JavaScript](https://img.shields.io/badge/JavaScript-F7DF1E?style=for-the-badge&logo=javascript&logoColor=black)
![Material Design](https://img.shields.io/badge/Material%203-757575?style=for-the-badge&logo=materialdesign&logoColor=white)
![Windows](https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)

> Fast, Material Design clipboard history manager that replaces <kbd>Win</kbd> + <kbd>V</kbd> and lives in the system tray

## Quick Start

Grab a build from the [releases page](https://github.com/NUCL3ARN30N/simple-clipboard/releases),
install or unzip it, and press <kbd>Win</kbd> + <kbd>V</kbd>.

| File                                         | Build                                                                     |
| -------------------------------------------- | ------------------------------------------------------------------------- |
| **`SimpleClipboard_x.y.z_x64-setup.exe`**    | Windows installer (per user, no admin needed)                             |
| **`SimpleClipboard_x.y.z_x64.msi`**          | Windows MSI                                                               |
| **`SimpleClipboard_x.y.z_x64_portable.zip`** | Windows portable build. Data is stored in a `data` folder next to the exe |
| **`simple-clipboard_x.y.z_amd64.deb`**       | Debian/Ubuntu                                                             |

## Features

| #  | Feature                      | Description                                                                                                                                                                                                                                                                                                        |
| -- | ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1  | **Win + V replacement**      | A low-level keyboard hook handles Win+V. If Windows still opens its own clipboard history as well, click *Settings → Shortcut → Disable Windows Win + V*. This adds `V` to `HKCU\…\Explorer\Advanced\DisabledHotkeys` and restarts Explorer. *Restore Windows default* undoes it, and so does the NSIS uninstaller |
| 2  | **Text and picture history** | With a configurable size. Pictures can be turned off                                                                                                                                                                                                                                                               |
| 3  | **Pinned palette**           | An optional extra column (left or right) for items you paste often. Pinned items are never trimmed. The palette folds into a slim rail and remembers whether it was open or collapsed                                                                                                                              |
| 4  | **Colored tags**             | For pinned items, with tag filter chips in the palette                                                                                                                                                                                                                                                             |
| 5  | **Search**                   | Across all text and tag names                                                                                                                                                                                                                                                                                      |
| 6  | **Auto paste**               | Picking an item pastes it into the app you were using                                                                                                                                                                                                                                                              |
| 7  | **Material 3 UI**            | Light, dark and system themes and a choice of accent color                                                                                                                                                                                                                                                         |
| 8  | **Export and import**        | Settings, tags and pinned text items as JSON                                                                                                                                                                                                                                                                       |
| 9  | **Updates**                  | Checks GitHub releases on startup or on demand. *Install update* downloads the right file for your install type (setup.exe, MSI, portable zip or `.deb`) and verifies its SHA-256 checksum when GitHub provides one. Installed versions start the installer; the portable version replaces its exe and restarts    |
| 10 | **Password manager aware**   | Respects the `ExcludeClipboardContentFromMonitorProcessing` flag that password managers set                                                                                                                                                                                                                        |

## Keyboard

| Key                                         | Action                                                              |
| ------------------------------------------- | ------------------------------------------------------------------- |
| <kbd>Win</kbd> + <kbd>V</kbd>               | Open/close the popup                                                |
| <kbd>↑</kbd> <kbd>↓</kbd>                   | Select                                                              |
| <kbd>Enter</kbd>                            | Paste selected item                                                 |
| <kbd>Ctrl</kbd> + <kbd>1</kbd>…<kbd>9</kbd> | Paste item by number                                                |
| <kbd>Tab</kbd>                              | Switch between history and pinned palette (expands it if collapsed) |
| <kbd>Ctrl</kbd> + <kbd>P</kbd>              | Pin / unpin                                                         |
| <kbd>Del</kbd>                              | Delete item                                                         |
| <kbd>Esc</kbd>                              | Clear search / close                                                |

## Linux Notes

| Topic               | Detail                                                                                                                                                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Auto paste**      | Uses `xdotool` on X11 (installed as a dependency) or `wtype` on Wayland                                                                                                                                      |
| **Global shortcut** | Wayland does not allow apps to register global shortcuts. Bind the command `simple-clipboard` to a key in your desktop's keyboard settings. Running it a second time opens the popup of the running instance |

## Requirements

**Windows:** Windows 10/11 with WebView2 (ships with the OS)  
**Linux:** Debian/Ubuntu `.deb`; `xdotool` (X11) or `wtype` (Wayland) for auto paste  
**Build:** Rust and Node.js — `npm run dev` to run locally, `npx tauri build` for a release build

---

Badges from [envoy1084/awesome-badges](https://github.com/envoy1084/awesome-badges)
