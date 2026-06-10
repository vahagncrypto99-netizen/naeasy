# naeasy

A minimal menu-bar launcher for your local Git projects — like JetBrains Toolbox, but for *projects*, not IDEs.

Add your workspace folders once; naeasy finds every Git repository inside and shows them as a tree in your menu bar. Click a project (or hit **⌘⇧M**, type a few letters, press Enter) — it opens in your IDE. If it's already open, naeasy just switches to its window.

- Auto-detects installed IDEs (JetBrains family, VS Code, Cursor, Zed, …); pick a default or choose per project
- Recent projects on top, type-to-search, full keyboard navigation
- Shows which projects are open right now and when you last opened each
- Launches at login, lives quietly in the menu bar — no Dock icon

**Lightweight by design**: built with Tauri 2 (native Rust backend, system webview) — a few MB on disk and native-app memory usage, not an Electron-sized footprint.

## Install

```bash
git clone https://gitlab.com/vahagn.crypto.99-group/naeasy.git
cd naeasy
./install.sh
```

The script checks your tools (Xcode CLT, Node.js ≥ 18, Rust), tells you exactly what to install if something is missing, then builds and installs `naeasy.app` into `/Applications` and launches it. The first build compiles Rust and takes a few minutes; re-runs are incremental and fast.

On Linux, use `./install-linux.sh`.

> **Tip:** "open right now" detection and window switching need the Accessibility permission — System Settings → Privacy & Security → Accessibility → add `naeasy`.

## Uninstall

```bash
./uninstall.sh
```
