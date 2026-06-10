# naeasy

A minimal menu-bar navigator for your local Git projects. It lives in the menu bar (like JetBrains Toolbox); a click shows a tree of every project inside the folders you add. Click a project → it opens in your chosen IDE.

Built on **Tauri 2** (Rust + web), with native-app memory usage.

## Features

- **Add workspace** — pick a folder; naeasy recursively finds every `.git` repository and builds a tree (folder → project). It does not descend into repos and skips `node_modules`, `vendor`, `target`, etc.
- **Click a project** — opens it in the default IDE.
- **Badge on a project** — a dropdown to open that specific project in a different IDE.
- **Default IDE** — keep several IDEs; make any of them the default.
- **Auto-detect** — finds installed PhpStorm, GoLand, DataGrip, PyCharm, IntelliJ, WebStorm, VS Code, Cursor, Zed and more in `/Applications` and `~/Applications` (including JetBrains Toolbox launchers).
- **Add IDE…** — point to a `.app` manually.
- Collapsible nodes and a name filter. State and IDE list persist across launches.
- **Launch at login**, plus a **Quit** button in settings.

Config is stored in `~/Library/Application Support/com.vahagn.naeasy/config.json`.

## Install — one step

```bash
cd naeasy
./install.sh
```

The script first checks your environment (Xcode CLT, Node.js ≥ 18, npm, Rust). **If something is missing it prints the exact steps for that specific tool and stops.** Fix it, run again. Once everything is present it builds the release, copies `naeasy.app` into `/Applications` and launches it — no manual dragging.

Re-running `./install.sh` always rebuilds and reinstalls — but Tauri/Cargo builds are **incremental**, so only changed code recompiles (fast). Use `./install.sh --rebuild` only for a full clean recompile (`cargo clean` first).

> Note: the very first build compiles Rust on your Mac (a few minutes). A prebuilt binary can't be shipped cross-platform because a macOS `.app` must be built and signed on macOS. Subsequent builds are incremental.

Example output when Rust is missing:

```
naeasy — environment check

✓ Xcode Command Line Tools
✓ Node.js 22.22.0
✓ npm 10.9.0
✗ Rust (cargo) — not installed
      Install rustup:  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
      Load it in this shell:  source "$HOME/.cargo/env"
      Then run ./install.sh again.

Missing tools: 1. Fix the items above and run ./install.sh again.
```

### Uninstall

```bash
./uninstall.sh
```

Quits the running app, disables login-item autostart and removes the bundle. (You can also quit from **⚙ → Quit naeasy** or the menu-bar icon, then drag it to the Trash.)

### Launch at login — automatic

The app registers itself in Login Items via `tauri-plugin-autostart`. It is enabled automatically on first run; toggle it in **⚙ → Launch at login**. No need to touch System Settings.

### Dev mode (optional)

```bash
npm install
npm run dev      # hot reload, menu-bar icon
```

## Troubleshooting / logs

The app logs to a file and to stdout (`tauri-plugin-log`).

Log file:

```
~/Library/Logs/com.vahagn.naeasy/naeasy.log
tail -f ~/Library/Logs/com.vahagn.naeasy/naeasy.log
```

To see a startup crash immediately, run the binary directly from Terminal (any
Rust panic prints to stderr):

```bash
/Applications/naeasy.app/Contents/MacOS/naeasy
# or, if it was installed to the user folder:
~/Applications/naeasy.app/Contents/MacOS/naeasy
```

You should see lines like `naeasy starting up` and `showing main window on launch`.
If it exits with a panic, that message is the cause — paste it back.

## Open / close / shortcut

naeasy is a **menu-bar app — no Dock icon** (like JetBrains Toolbox).

**To open / show the window:**

- click the **menu-bar icon**, or
- press the **global shortcut** (default **⌘⇧M**, change it in ⚙ → Global shortcut).

**Window buttons** (traffic lights over the header, `titleBarStyle: Overlay`):

- **🔴 red** — hides the window (app keeps running; reopen via menu-bar icon or shortcut);
- **🟡 yellow** — minimize · **🟢 green** — zoom.

**To quit completely:** the menu-bar icon's menu → Quit, or **⚙ → Quit naeasy**.

> Want it in the Dock / Cmd+Tab instead? Change `ActivationPolicy::Accessory` to `Regular` in `src-tauri/src/lib.rs`.

### Project info

Each project row shows usage info:

- **● IDE** (green) — the project is **open right now** in that IDE;
- **"3h ago"** — when you **last opened** it from naeasy (and which IDE, on hover).

"Open right now" detection reads IDE window titles via AppleScript and needs **Accessibility** permission (System Settings → Privacy & Security → Accessibility → add `naeasy`). Without it, only "last opened" is shown.

### Menu-bar icon not showing?

macOS only shows it if **System Settings → Menu Bar → Allow in Menu Bar → naeasy** is ON *and* the app was launched after enabling it (relaunch if you just toggled it). If it's still missing it's hidden behind the **notch** — too many menu-bar items. A free fixer: `brew install --cask ice`.

## Open behavior (switch, not re-open)

On click, naeasy first checks whether the project is already open in that IDE:

1. **Already open** — switches to its window: un-minimizes (`AXMinimized = false`), raises it (`AXRaise`) and focuses the app. If several projects are open in separate windows, it picks the window whose title contains the project folder name.
2. **Not open** — runs `open -a "<.app path>" "<project path>"`, opening the folder as a project.

Step 1 uses AppleScript / System Events and needs **Accessibility** permission:

System Settings → Privacy & Security → Accessibility → add `naeasy.app` (or Terminal in dev mode).

> Without that permission step 1 is silently skipped and `open -a` is used — which already focuses an existing project window for JetBrains and VS Code, but may not un-minimize it. For the full behavior (un-minimize + exact window among several projects) grant Accessibility.

Windows/Linux: runs `<ide> <project_path>` directly (for portability; the main target is macOS).

## Custom icon

A polished icon set is already generated in `src-tauri/icons/` (color app icon + a monochrome menu-bar template icon `tray.png`). To replace it, drop your own `icon.png` (1024×1024) and run:

```bash
npm run tauri icon src-tauri/icons/icon.png
```

## Structure

```
naeasy/
├── install.sh              # one-step build + install with tool checks
├── uninstall.sh            # quit + remove
├── package.json            # npm scripts, Tauri CLI/API, dialog + autostart plugins
├── src/                    # frontend (vanilla HTML/CSS/JS, no bundler)
│   ├── index.html
│   ├── styles.css
│   └── main.js
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json     # popover window, tray, bundle
    ├── capabilities/default.json
    ├── icons/              # app icon set + tray template icon
    └── src/
        ├── main.rs
        └── lib.rs          # scan, IDE detect, focus-or-open, persist, tray
```
