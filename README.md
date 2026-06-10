<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" alt="naeasy icon">
</p>

<h1 align="center">naeasy</h1>

<p align="center">
  <b>All your Git projects, one keystroke away.</b><br>
  A featherweight menu-bar launcher: find any local project and open it in your IDE — instantly.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/macOS-menu%20bar-black?logo=apple" alt="macOS">
  <img src="https://img.shields.io/badge/Linux-supported-FCC624?logo=linux&logoColor=black" alt="Linux">
  <img src="https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white" alt="Tauri 2">
  <img src="https://img.shields.io/badge/backend-Rust-orange?logo=rust" alt="Rust">
</p>

<!-- screenshot: add docs/screenshot.png and uncomment
<p align="center">
  <img src="docs/screenshot.png" width="420" alt="naeasy window">
</p>
-->

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
| Disk size | ~6 MB |
| Memory | native-app footprint |
| Runtime deps | none |

## Install

One command — paste it into Terminal:

```bash
/bin/bash -c "$(curl -fsSL https://gitlab.com/vahagn.crypto.99-group/naeasy/-/raw/main/install.sh)"
```

What it does:

1. Checks your tools (Xcode CLT, Node.js ≥ 18, Rust, git) — if something is missing, it prints the exact command to fix it and stops.
2. Downloads the source into a **temporary directory**.
3. Builds `naeasy.app` (the first build compiles Rust — a few minutes), installs it into `/Applications` and launches it.
4. Deletes the temporary directory — nothing is left behind except the app itself.

<details>
<summary>Installing from a clone (for development)</summary>

```bash
git clone https://gitlab.com/vahagn.crypto.99-group/naeasy.git
cd naeasy
./install.sh        # builds incrementally on re-runs; --rebuild for a clean build
```

On Linux: `./install-linux.sh`.

</details>

> 💡 For "open right now" detection and window switching, grant Accessibility permission: **System Settings → Privacy & Security → Accessibility → naeasy**.

## Uninstall

Also one command, from anywhere — no clone needed:

```bash
/bin/bash -c "$(curl -fsSL https://gitlab.com/vahagn.crypto.99-group/naeasy/-/raw/main/uninstall.sh)"
```

It quits the running app, removes `naeasy.app` from `/Applications` (and `~/Applications`), and disables the login item.

Your settings are kept in case you reinstall. To wipe them too:

```bash
rm -rf "$HOME/Library/Application Support/com.vahagn.naeasy"
```
