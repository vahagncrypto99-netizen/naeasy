# Releasing naeasy

Two-repo model: **source stays private, only the built app is public.**

```
PRIVATE repo  (this one)            PUBLIC repo  (users install from here)
─────────────────────────          ─────────────────────────────────────
full source, dev history           dist/naeasy-macos-<version>.zip
build outputs are .gitignored       install.sh   (idempotent installer)
release.sh produces ./dist  ──────▶ README.md
```

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

## Publish (pick one)

- **Public repo:** copy `dist/*` into the public repo, commit, push. Users:
  ```bash
  git clone <public-repo> && cd <public-repo> && ./install.sh
  ```
- **GitHub release:** `gh release create v<version> dist/naeasy-macos-<version>.zip dist/install.sh`

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
