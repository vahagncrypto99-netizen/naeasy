# Releasing naeasy

Two-repo model: **source stays private, only the built app is public.**

```
PRIVATE repo: naeasy-dev            PUBLIC repo: naeasy
(~/workspace/personal/naeasy-dev)   (~/workspace/personal/naeasy)
─────────────────────────          ─────────────────────────────────────
full source, dev history           bin/naeasy-macos-<version>.zip (all versions)
build outputs are .gitignored       install.sh   (idempotent, picks newest zip)
release.sh produces ./dist  ──────▶ README.md
```

`release.sh` publishes automatically: if a checkout of the public repo exists
next to this one (`../naeasy`, override with `PUBLIC_DIR=...`), it copies the
new zip into `bin/`, refreshes `install.sh` and **commits** there — you only
`git push` it.

Source is never published; users only ever see a prebuilt, zipped `.app` plus a
one-command installer.

## Cut a release (from the private repo)

```bash
./release.sh            # build current version → ./dist
./release.sh 0.2.0      # bump version everywhere, then build → ./dist
```

`./dist` then contains:

- `naeasy-macos-<version>.zip` — the prebuilt app bundle
- `install.sh` — the public installer
- `README.md` — short user instructions

`dist/` is git-ignored here (private repo stays source-only).

## Publish

With `../naeasy` checked out, `./release.sh` already committed the new version
there — just push:

```bash
git -C ../naeasy push
```

Users:
```bash
git clone <public-repo> naeasy && cd naeasy && ./install.sh
```

Fallback (no local public checkout): copy `dist/naeasy-macos-<version>.zip` →
`<public>/bin/`, `dist/install.sh` → `<public>/install.sh`, commit & push; or
`gh release create v<version> dist/naeasy-macos-<version>.zip dist/install.sh`.

## Install / upgrade (users)

```bash
./install.sh
```

The installer is **idempotent** — it works the same whether naeasy is:

- **not installed** → fresh install;
- **already installed (older version)** → it quits the running app, removes the
  old bundle (in `/Applications` or `~/Applications`), installs the new one,
  strips the quarantine flag (so the unsigned app opens cleanly), and launches.

So rolling out a new version is just: `./release.sh X.Y.Z` → publish → users
re-run `./install.sh`.

## Versioning

`release.sh X.Y.Z` keeps the version in sync across `tauri.conf.json`,
`package.json` and `Cargo.toml`. `tauri.conf.json` is the source of truth used
for the artifact name.

## Linux

```bash
./install-linux.sh      # builds .deb + .AppImage (see README "Linux")
```

Attach the `.deb`/`.AppImage` from `src-tauri/target/release/bundle/` to the
same public release.

## Code signing (optional, recommended later)

The app is currently unsigned, so the installer strips `com.apple.quarantine`
to avoid Gatekeeper prompts. For wider distribution, sign + notarize with an
Apple Developer ID and Tauri's `bundle.macOS.signingIdentity` — then the
quarantine workaround is no longer needed.
```
