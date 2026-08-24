<p align="center">
  <img src="assets/icon.png" width="128" alt="naeasy icon">
</p>

<h1 align="center">naeasy</h1>

<p align="center">
  <b>All your Git projects, one keystroke away.</b><br>
  A featherweight menu-bar launcher: find any local project and open it in your IDE — instantly.
</p>

<p align="center">
  <img src="https://img.shields.io/github/v/release/vahagncrypto99-netizen/naeasy?color=0a84ff" alt="Release">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT">
  <img src="https://img.shields.io/badge/macOS-menu%20bar-black?logo=apple" alt="macOS">
  <img src="https://img.shields.io/badge/Ubuntu-deb-E95420?logo=ubuntu&logoColor=white" alt="Ubuntu">
  <img src="https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white" alt="Tauri 2">
  <img src="https://img.shields.io/badge/backend-Rust-orange?logo=rust" alt="Rust">
</p>

---

You have dozens of projects scattered across workspace folders. Opening one means digging through Finder, a terminal, or your IDE's welcome screen. **naeasy** turns that into:

<p align="center"><code>⌘⇧M</code> → type 3 letters → <code>Enter</code> → your project is open. ✨</p>

Point it at your workspace folders once — it discovers every Git repository inside and keeps them in your menu bar, neatly grouped and searchable.

## Features

- 🔍 **Instant search** — summon with a global hotkey, type a few letters, hit Enter. Spotlight-fast, keyboard-first.
- 🗂 **Auto-discovery** — recursively finds every Git repo in your folders; new projects appear by themselves.
- 🚀 **Opens in *your* IDE** — auto-detects JetBrains IDEs, VS Code, Cursor, Zed and more; set a default or pick per project.
- 🪟 **Switches, never duplicates** — if a project is already open, naeasy focuses its existing window instead of opening a copy.
- 🟢 **Live status** — see which projects are open right now and when you last touched each one; recent ones float to the top.
- 🫥 **Stays out of the way** — lives in the menu bar, no Dock icon, starts at login.

## Lightweight, really

No Electron inside. naeasy is a native **Rust** binary with the system webview (**Tauri 2**):

|  | naeasy |
|---|---|
| Disk size | ~4 MB |
| Memory | native-app footprint |
| Runtime deps | none |

## Install (macOS)

One command — paste it into Terminal. Prebuilt app: no toolchains, no compiling, ready in seconds:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/install.sh)"
```

It downloads the newest version, replaces any previously installed copy,
strips the quarantine flag (the app is unsigned) and launches the app.

## Update

The installer is idempotent — updating is the **same command**:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/install.sh)"
```

## Uninstall

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/uninstall.sh)"
```

It quits the running app, removes `naeasy.app` from `/Applications` (and `~/Applications`), and disables the login item.

Your settings are kept in case you reinstall. To wipe them too:

```bash
rm -rf "$HOME/Library/Application Support/com.vahagn.naeasy"
```

## Ubuntu / Debian Linux

**Install** — one command (detects amd64/arm64, installs the newest `.deb` with dependencies):

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/vahagncrypto99-netizen/naeasy/main/install-linux.sh)"
```

**Update** — the same command again.

**Uninstall:**

```bash
sudo apt-get remove -y naeasy
```

> 💡 `wmctrl` (window focusing / "open now" detection, X11) installs automatically as a package dependency.

**Placing the popover under the tray icon.** The Linux tray protocol
(StatusNotifierItem) never tells an app where its own icon ended up, so by
default the popover pins itself to the top-right corner. To have it open
centered under the icon instead, record the icon's position once:

```bash
sudo apt-get install -y python3-pyatspi
./scripts/detect-tray-anchor.py --set icon    # naeasy's indicator is named 'icon'
```

That writes `tray_anchor_x` to `~/.config/com.vahagn.naeasy/config.json`.
Re-run it after adding or removing other indicators — they shift each other
along the top bar. Prefer to place the window yourself? Turn on the floating
window in **Settings -> Window** and drag it where you want.

<details>
<summary>Installing from a clone (no curl-pipe)</summary>

```bash
git clone https://github.com/vahagncrypto99-netizen/naeasy.git
cd naeasy && ./install.sh      # downloads the latest prebuilt release
```

</details>

## Building from source

```bash
git clone https://github.com/vahagncrypto99-netizen/naeasy.git
cd naeasy && npm install
./dev-install.sh               # macOS: build + install to /Applications
./scripts/build-linux.sh       # Linux: .deb via Docker (amd64 + arm64)
```

Requires Rust and Node 18+. Architecture and contributor workflow:
[DEVELOPMENT.md](DEVELOPMENT.md) · [CONTRIBUTING.md](CONTRIBUTING.md)

> 💡 For "open right now" detection and window switching, grant Accessibility permission: **System Settings → Privacy & Security → Accessibility → naeasy**.
>
> 💡 If the menu-bar icon doesn't show: **System Settings → Menu Bar → Allow in Menu Bar → naeasy**, then relaunch.

---

<p align="center">
  <sub>
    open any project in seconds · switch between projects instantly · all your git projects in one place ·
    stop digging through folders for that one repo · manage dozens of repositories without chaos ·
    pick up where you left off · your recent work always at hand · one shortcut instead of Finder, terminal and welcome screens
  </sub>
</p>
