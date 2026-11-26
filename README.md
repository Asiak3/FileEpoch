# Rewind

A Time Machine–style viewer and restore tool for [Kopia](https://kopia.io) backups.
Rewind connects to a Kopia web server (ideally one started with `--readonly`), lets you move
through snapshots on a timeline, browse files as they were, and restore them **to your own
computer**.

> **Status: early development.** Browsing and restoring work against Kopia 0.23.1: connect,
> pick a source, move along the timeline, explore folders and restore files, folders or a whole
> snapshot to your computer. Packaging (installer) comes next.

## Safety by design

- **Read-only.** The client can only build `GET` requests to five whitelisted endpoints
  ([docs/API.md](docs/API.md) §1). Endpoints that act on the server (restore on the server,
  mounts, disconnect, policies, …) cannot be called, even if the server would allow them.
- **Restores are local.** Files are downloaded through the API and written on your computer.
  Existing files are never overwritten.
- **Pinned TLS certificate.** Self-signed Kopia certificates are trusted by their SHA-256
  fingerprint, checked before any password is typed. Certificate checks are never just disabled.
- **Passwords** live only in the operating system's credential store (Windows Credential
  Manager, macOS Keychain, Secret Service). They are never accepted as command-line options or
  environment variables, and never logged.

## Tested Kopia versions

| Kopia  | Status |
|--------|--------|
| 0.23.1 | Tested (read-only server, filesystem storage) |

Other versions trigger a warning. See [docs/API.md](docs/API.md) for the (undocumented) Kopia web
API and how it was verified.

## Building

Requirements: Rust 1.88 or newer. On Windows, the Visual Studio C++ Build Tools.

```sh
cargo build --release
cargo test
```

## Desktop app

Requirements in addition to Rust: Node.js 20 or newer. On Windows, WebView2 (included in
Windows 10/11).

```sh
cd app
npm install
npm run tauri dev      # development, with hot reload
npm run tauri build    # optimized build: target/release/rewind-app.exe
```

On first start Rewind asks for the server address and user, shows the server certificate
fingerprint for you to compare with the Kopia server log, and asks for the password (optionally
saved in the system credential store). A password saved with `rewind-cli --save-password` is used
by the app too.

Using it:

- **Timeline** (top): click or drag to move through time; arrow keys, Home/End and PgUp/PgDn work
  when it has focus. Consecutive snapshots with identical content are grouped ("unchanged until …");
  switch off *Group identical* to see each one. The selected version shows its total size and
  number of files, and how much the total changed since the previous version (Kopia stores
  unchanged data only once, so this is not the space it takes on the server).
- **Files** (below): double-click or Enter opens a folder, Backspace goes up, Alt+←/→ go back and
  forward, Ctrl+F searches the current folder, Ctrl/Shift+click selects several items.
- **Staying in the folder while moving in time:** Rewind remembers the folder you opened. If it does
  not exist at the selected time, the nearest existing parent folder is shown (the missing part of
  the path appears struck through); move to a time where it exists and you are back in it.
- **Views:** details, list, large icons and tiles. In details, drag a column separator to resize
  it and right-click the header to show or hide columns (date, type, size, files, permissions,
  owner). Large icons and tiles show thumbnails of images (JPEG, PNG, GIF, WebP, BMP, TIFF up to
  64 MB): each image is downloaded and shrunk in memory, only while it is on screen, and nothing is
  saved to disk. They can be switched off in the settings.
- **Restoring:** select items and press *Restore* (or right-click → *Restore…*); with nothing
  selected the button restores the folder you are looking at (right-click on empty space does the
  same). Rewind asks for the destination folder every time. Restores run one after another in the
  background; the *Restores* panel shows progress, lets you cancel, and summarizes renamed items,
  skipped links and errors.
- **Settings** (gear icon, top right) open in a window with tabs: System (language, tray, start
  with Windows), Display (theme, timeline style, thumbnails), Restore, Connection and About.
- **Restore options** (settings, *Restore* tab): name restored items `report (version of 2026-11-25 03h02).pdf`
  (date of the snapshot), `report (modified 2026-11-24 16h46).pdf` (the file's own modification
  date) or keep the original name; the menu shows an example from the current folder. If an item already
  exists, keep both (default), skip it, or overwrite it — overwriting always asks first and lists
  what would be replaced. Optionally verify size and date of every restored file.
- **Tray and start with Windows** (settings, *System* tab; both off by default): closing the window can keep
  Rewind running next to the clock (click the icon to open it; *Quit* in its menu). Started with
  Windows and with the tray option on, it starts hidden in the tray. Quitting while a restore runs
  asks first. The list of versions is reloaded every 5 minutes and when the window gets focus; the
  round arrow next to the number of versions checks right away (its tooltip shows the last check).
- All times are shown in your local time zone. Languages: English, Spanish, German.

To work on the user interface without a Kopia server, `npm run dev` serves it at
http://127.0.0.1:1420 with a mock backend (sample data, password `demo`). URL parameters such as
`?lang=de&theme=dark&source=2` select the sample; see `app/src/lib/mock.ts`.

## Command-line tool

`rewind-cli` exercises the client library. Connection settings can be passed as options or
environment variables (`REWIND_SERVER`, `REWIND_USER`, `REWIND_FINGERPRINT`).

```sh
# 1. Read the server's certificate fingerprint (no login, no HTTP request).
#    Compare it with "SERVER CERT SHA256" in the Kopia server log.
rewind-cli --server https://backup.example:51515 fingerprint

# 2. Use it. The password is asked once; --save-password stores it in the credential store.
rewind-cli --server https://backup.example:51515 --user ui-user --fingerprint <sha256> --save-password status
rewind-cli ... sources
rewind-cli ... snapshots --source docs            # identical snapshots grouped; --all shows each
rewind-cli ... ls --source docs --snapshot latest --path some/folder
rewind-cli ... download --source docs --snapshot "2026-11-25 19:00" --path some/folder/file.pdf --out .
rewind-cli ... download --source docs --path some/folder --out .   # a whole folder (no --path: whole snapshot)
rewind-cli ... forget-password
```

Times are shown in your local time zone. `download` writes `file (version of <snapshot time>).pdf`
(or `--keep-name`), never overwrites, verifies sizes and restores modification times.

## Repository layout

| Path                     | Contents |
|--------------------------|----------|
| `crates/kopia-client`    | Read-only Kopia API client library (TLS pinning, session/CSRF handling, restore) |
| `crates/rewind-cli`      | Command-line test tool |
| `app/`                   | Desktop app: Svelte 5 user interface (`src/`) and Tauri 2 backend (`src-tauri/`) |
| `docs/API.md`            | Kopia web API notes: endpoints, authentication, what is forbidden and why |
| `tools/verify-api.ps1`   | Checks the documented API against a live server (read-only) |
| `tools/anonymize-samples.py` | Turns recorded responses into anonymized test fixtures |

## Author

Rewind is written by A.C. — https://github.com/Asiak3/Rewind

## License

[Apache License 2.0](LICENSE), like Kopia. Rewind is not affiliated with the Kopia project.
