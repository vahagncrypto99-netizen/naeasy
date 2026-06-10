# naeasy — target architecture

Goal: a clean, layered, testable structure that scales, following SOLID and
proven patterns. Today the backend lives in one `src-tauri/src/lib.rs`; this
doc defines the modular target and the incremental migration plan.

## Layered module layout (Rust backend)

```
src-tauri/src/
├── main.rs                 # binary entry → naeasy_lib::run()
├── lib.rs                  # composition root: wires deps, registers commands
│
├── domain/                 # pure business logic — no I/O, no Tauri, no OS
│   ├── mod.rs
│   ├── models.rs           # Workspace, Ide, Recent, TreeNode, WorkspaceView…
│   └── tree.rs             # find_repos, build_tree, sort, count  (+ unit tests)
│
├── infra/                  # adapters to the outside world (I/O, OS, framework)
│   ├── mod.rs
│   ├── config_repository.rs    # ConfigRepository trait + JsonConfigRepository
│   ├── ide_detector.rs         # IdeDetector trait + Mac/Linux/Windows impls
│   ├── project_launcher.rs     # ProjectLauncher trait + per-OS impls
│   └── shortcut.rs             # ShortcutService (register/toggle)
│
├── app/                    # application services (use-cases) — orchestration
│   ├── mod.rs
│   ├── state.rs            # AppState (holds the trait objects + config cache)
│   └── workspace_service.rs    # add/remove workspace, open project, scan…
│
└── ui/                     # Tauri glue (thin)
    ├── mod.rs
    ├── commands.rs         # #[tauri::command] — parse args, call services
    └── tray.rs             # tray icon, window show/hide/position
```

## SOLID mapping

- **S — Single Responsibility:** each module has one reason to change. Tree
  scanning ≠ config persistence ≠ OS launching ≠ Tauri commands.
- **O — Open/Closed:** adding a platform or an IDE = a new strategy impl; no
  edits to callers. Adding a storage backend = a new `ConfigRepository` impl.
- **L — Liskov:** every `IdeDetector`/`ProjectLauncher`/`ConfigRepository`
  implementation is fully substitutable behind its trait.
- **I — Interface Segregation:** small focused traits (`IdeDetector`,
  `ProjectLauncher`, `WindowFocuser`, `ConfigRepository`) instead of one
  god-interface.
- **D — Dependency Inversion:** `app` services depend on traits in `domain`/
  `infra` abstractions, not on concrete impls. `lib.rs` (composition root)
  injects the concrete impls at startup.

## Patterns

- **Repository** — `ConfigRepository` abstracts persistence (JSON today; could
  be SQLite/remote later) and makes services unit-testable with a fake repo.
- **Strategy** — platform behavior (`MacLauncher`, `LinuxLauncher`,
  `WindowsLauncher`; `MacIdeDetector`, …) selected at the composition root,
  not via `cfg!` scattered through business code.
- **Facade / Service layer** — `WorkspaceService` exposes coarse use-cases
  (`open_project`, `add_workspace`, `scan_open`) so Tauri commands stay thin.
- **Command** — Tauri `#[command]` handlers are thin adapters that translate
  IPC ↔ service calls (no business logic inside).
- **Dependency Injection** — wiring happens once in `run()`; services receive
  their collaborators via constructor.

## Frontend (later slice)

`src/main.js` → ES modules (no bundler needed, native `import`):

```
src/
├── index.html
├── styles.css
└── js/
    ├── main.js        # bootstrap
    ├── api.js         # invoke() wrappers (the only place that calls Tauri)
    ├── store.js       # app state + persistence (localStorage)
    ├── tree.js        # tree render + keyboard navigation
    ├── recents.js     # recent section
    ├── settings.js    # IDE list, default select, shortcut recorder
    └── shortcuts.js   # type-to-search + arrow/Enter handling
```

## Incremental migration plan (compile-checkpointed) — ✅ COMPLETED June 2026

All 7 slices below are done (one git commit per slice, build/tests green after
each). Minor deviations from the plan: `ProjectLauncher` exposes `open` /
`scan_open` / `reveal` (focus-or-open is internal to `open` — no caller needs
focus separately); `gen_id` lives in `domain/models.rs` (shared by service and
detectors); the frontend gained a small extra `js/dom.js` helpers module.

Because a macOS `.app` must be compiled on macOS (CI/local), each slice ends
with a `./install.sh` rebuild to keep the app green. Order:

1. **domain/** — move models + tree logic (pure, has tests). `cargo test` green.
2. **infra/config_repository.rs** — Repository pattern; services use the trait.
3. **infra/ide_detector.rs** — Strategy per OS behind `IdeDetector`.
4. **infra/project_launcher.rs** — Strategy per OS (open/focus/scan).
5. **app/** — `WorkspaceService` + `AppState`; commands delegate to it.
6. **ui/** — split commands + tray; `lib.rs` becomes the composition root.
7. **frontend** — split `main.js` into ES modules.

Each step is mechanical and behavior-preserving; we rebuild after each.
```
