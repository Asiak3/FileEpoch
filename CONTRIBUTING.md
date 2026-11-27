# Contributing to FileEpoch

Thanks for helping! Bug reports, translations, testing on Linux and macOS, and code are all
welcome.

## Reporting bugs

Open an issue with: FileEpoch version (Settings → About), operating system, Kopia server version,
what you did, what you expected and what happened. **Never include passwords, server addresses,
certificate fingerprints, user names or file names from your backups**; replace them with
placeholders. Security problems: see [SECURITY.md](SECURITY.md).

## Development setup

Requirements: Rust (stable, 1.88 or newer), Node.js 20 or newer. On Windows the Visual Studio C++
Build Tools; on Linux the WebKitGTK and tray libraries:

```sh
sudo apt-get install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev build-essential
```

```sh
cd app
npm install
npm run dev          # UI only, with a mock backend: http://127.0.0.1:1420 (no Kopia server needed)
npm run tauri dev    # the desktop app with hot reload
```

`docs/SERVER.md` explains how to run a read-only Kopia server to test against.

## Before you open a pull request

Run what CI runs:

```sh
node tools/set-version.mjs --check
node tools/third-party-notices.mjs --check   # needs: cargo install cargo-about --features cli
cd app && npx svelte-check --threshold warning && npm run build && cd ..
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Rules that are not negotiable

- **Read-only.** Do not add endpoints to the whitelist that change anything on the server
  (see [docs/API.md](docs/API.md) §7). New read-only endpoints must be documented in docs/API.md
  with how they were verified.
- **No real data in the repository**: no real server addresses, user names, fingerprints, paths or
  file names. Test fixtures are anonymized (`tools/anonymize-samples.py`).
- **Passwords** only through the system credential store; never logged.
- **Dependencies:** only licenses listed in `about.toml` are accepted. After adding or updating a
  dependency, run `node tools/third-party-notices.mjs` and commit the regenerated
  `app/public/third-party-notices.html` (shown in Settings → About).
- Keep the three languages (English, Spanish, German) in `app/src/lib/i18n.svelte.ts` complete.
  New strings in English are fine; a maintainer or translator fills in the others.

## Style

- Code, comments, commit messages and docs in English.
- Small commits with a clear message: what changed and why.
- Rust: `cargo fmt` (see `rustfmt.toml`) and no clippy warnings.
- UI: Svelte 5 (runes) and TypeScript; svelte-check without warnings.

## Releases (maintainer)

1. `node tools/set-version.mjs 0.2.0`, move the Unreleased entries of CHANGELOG.md under
   `## [0.2.0] - date`, commit.
2. `git tag v0.2.0 && git push origin v0.2.0`.
3. The Release workflow builds all installers and creates a **draft** release. Its notes are the
   CHANGELOG.md entry of that version plus the download table (`tools/release-notes.mjs`; the
   workflow fails if the entry is missing). Check the draft, then publish it: only then does the
   updater offer it.

By contributing you agree that your contribution is licensed under the Apache License 2.0.
