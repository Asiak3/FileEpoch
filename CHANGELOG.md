# Changelog

All notable changes to FileEpoch. The format follows [Keep a Changelog](https://keepachangelog.com/)
and versions follow [Semantic Versioning](https://semver.org/) (0.x while in beta).

## [Unreleased]

## [0.1.0] - 2026-11-26

First beta.

### Added
- Connect to a Kopia web server with a pinned TLS certificate fingerprint; the password is kept
  only in the system credential store.
- Timeline of the versions (snapshots) of each source, grouping identical consecutive versions,
  with the size and file count of each version.
- File browser with details, list, large icons (with image thumbnails) and tiles views; sortable,
  resizable and optional columns; search; stays in the same folder while moving in time.
- Restore files, folders or whole versions to this computer, with a queue, progress, cancel and a
  summary; restored items are named "(version of …)" or keep their name; existing files are never
  overwritten without asking.
- Settings: language (English, Spanish, German), theme, timeline style, tray, start with the
  system, automatic update checks.
- Windows installer; Linux (AppImage, deb) and macOS (dmg) builds; signed updates through GitHub
  Releases.
- `fileepoch-cli`, a command-line tool for the same read-only client.
