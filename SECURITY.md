# Security policy

FileEpoch handles access to backups, so security reports are very welcome.

## Reporting a vulnerability

Please **do not open a public issue**. Use GitHub's private reporting instead: on the repository
page go to **Security → Report a vulnerability**. Include what you found, how to reproduce it and
which version you used. You will get an answer as soon as possible; this is a one-person project,
so please allow a few days.

## What FileEpoch promises

- **Read-only towards Kopia.** The client can only build `GET` requests to a fixed list of
  endpoints ([docs/API.md](docs/API.md) §1, enforced in `crates/kopia-client/src/endpoint.rs`).
  Anything that would make the server write, delete, restore on the server, mount or change
  settings is out of scope by construction. A way around this whitelist is a vulnerability.
- **Pinned certificate.** The server is identified by the SHA-256 fingerprint of its TLS
  certificate, checked before a password is sent. Certificate checks are never just switched off.
- **Passwords** only live in the system credential store; never in files, logs, command-line
  arguments or environment variables.
- **Restores write only below the folder the user chose.** Names from the backup cannot escape it
  (`..`, absolute paths, links, reserved names), and existing files are not overwritten without
  explicit confirmation.
- **Updates** are downloaded from GitHub Releases and verified against a public key built into
  the app before they are installed. This is the only network access besides the Kopia server.

Reports that break any of these are in scope. Issues in Kopia itself should go to the
[Kopia project](https://github.com/kopia/kopia/security).

## Supported versions

Only the latest release gets fixes.
