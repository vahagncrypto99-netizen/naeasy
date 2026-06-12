# Contributing to naeasy

Thanks for your interest! Issues and pull requests are welcome.

## Quick start

```bash
git clone https://github.com/vahagncrypto99-netizen/naeasy.git
cd naeasy
npm install
npm run dev          # hot-reload dev app (needs Rust + Node 18+)
```

Architecture, layout and the full dev workflow live in
[DEVELOPMENT.md](DEVELOPMENT.md) and [ARCHITECTURE.md](ARCHITECTURE.md).

## Before you open a PR

- `cd src-tauri && cargo build && cargo test` — must be green, no new warnings
- Keep the layering: pure logic in `domain/`, OS/IO behind traits in `infra/`,
  use-cases in `app/`, thin Tauri glue in `ui/`
- Platform-specific code goes behind the existing strategy traits — no
  scattered `#[cfg(target_os)]` in business logic
- The IPC contract (command names, JSON shapes) is frozen — extend, don't break
- One focused change per PR; add a unit test when you touch `domain/` or `app/`

## Bugs & ideas

Open an issue with your OS/version and steps to reproduce. Screenshots help.
"Help wanted" / "good first issue" labels mark good entry points.
