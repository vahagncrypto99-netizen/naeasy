# naeasy — dev (source)

Private source repository for **naeasy**, a macOS menu-bar launcher for local
Git projects. Users never see this repo — prebuilt binaries are published to
the public repo: <https://github.com/vahagncrypto99-netizen/naeasy>
(see [RELEASE.md](RELEASE.md) for the two-repo release model).

**Stack:** Tauri 2 — Rust backend + vanilla ES-module frontend (no bundler,
`withGlobalTauri: true`). macOS first; Linux/Windows code paths exist behind
platform strategies.

## Architecture

Clean layered backend (full rationale and SOLID/pattern mapping in
[ARCHITECTURE.md](ARCHITECTURE.md)):

```
src-tauri/src/
├── lib.rs                  # composition root: DI wiring + command registration ONLY
├── domain/                 # pure logic — no I/O, no Tauri, no OS, unit-tested
│   ├── models.rs           #   Workspace, Ide, Config, Recent, TreeNode, AppData…
│   └── tree.rs             #   find_repos / build_tree / build_app_data (+ tests)
├── infra/                  # adapters to the outside world
│   ├── config_repository.rs#   ConfigRepository trait + JsonConfigRepository
│   ├── ide_detector.rs     #   IdeDetector trait + Mac/Linux/Windows strategies
│   ├── project_launcher.rs #   ProjectLauncher trait + per-OS strategies
│   └── shortcut.rs         #   ShortcutService (global hotkey registration)
├── app/                    # use-cases / orchestration
│   ├── workspace_service.rs#   WorkspaceService facade (+ tests vs fake repo)
│   └── state.rs            #   AppState { service } — Tauri-managed
└── ui/                     # Tauri glue, thin
    ├── commands.rs         #   13 #[tauri::command] handlers → service calls
    └── tray.rs             #   tray icon, window show/hide/position
```

Key rules the structure enforces:

- **Dependency direction:** `ui → app → infra-traits/domain`; concrete infra
  impls are injected once in `lib.rs::run()` (`Arc<dyn Trait>`).
- **Platform code** lives only inside `infra/` impls; `#[cfg(target_os)]`
  never appears in domain/app/ui. Adding an OS = new strategy impl.
- **IPC contract is frozen** in `ui/commands.rs`: command names and JSON
  shapes are what the frontend (and any future client) depends on.
- **Config** is JSON at `~/Library/Application Support/com.vahagn.naeasy/`,
  abstracted by `ConfigRepository` — services are tested against an in-memory
  fake (`cargo test`, 15 tests).

Frontend mirrors the same idea (`src/js/`): `api.js` is the **only** module
touching `window.__TAURI__`; `store.js` holds state; `tree.js` / `recents.js`
/ `settings.js` / `shortcuts.js` split rendering and input; `main.js` is the
bootstrap that wires them; `dom.js` — shared helpers.

## Develop

```bash
cd src-tauri && cargo build     # compile backend
cd src-tauri && cargo test      # domain + service unit tests
npm run dev                     # hot-reload dev app
./install.sh                    # local release build → /Applications (incremental)
```

## Release

```bash
./release.sh 0.2.0              # bump version, build, copy zip into ../naeasy/bin,
                                # refresh installer, commit in the public repo
git -C ../naeasy push           # publish
```

Old versions stay in the public `bin/`; the public `install.sh` always
installs the newest (`sort -V`). Details: [RELEASE.md](RELEASE.md).

## More docs

- [ARCHITECTURE.md](ARCHITECTURE.md) — target architecture, patterns, the
  completed migration plan
- [RELEASE.md](RELEASE.md) — private-source / public-binary release model
- `openspec/` — spec-driven change history (local only, not pushed)
