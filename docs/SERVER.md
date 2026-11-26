# Setting up a Kopia server for FileEpoch

FileEpoch talks to a **Kopia web server** (`kopia server start`). The safest setup is a server
that is used only by FileEpoch and opens the repository **read-only**. Flag names below were
checked against the Kopia 0.23.1 source code. Examples use placeholders: replace
`/srv/backup/repo`, `backup.example`, the user name and the passwords with your own.

## 1. A read-only connection to the repository

Read-only is a property of the **repository connection** (its config file), not an option of
`kopia server start`. Give the FileEpoch server its own config file, so that an administration
server or scheduled snapshots keep using theirs:

```sh
# New connection, read-only from the start (filesystem storage shown; other storage types take
# the same --readonly flag).
kopia repository connect filesystem --path /srv/backup/repo \
  --config-file ~/.config/kopia/fileepoch.config --readonly

# Or make an existing (copied) config read-only:
kopia --config-file ~/.config/kopia/fileepoch.config repository set-client --read-only
```

With such a connection Kopia wraps the storage so that no repository data can be written.
FileEpoch checks this at every connection and shows a warning if the server is not read-only.

## 2. Start the server with TLS and a UI user

```sh
kopia server start \
  --config-file ~/.config/kopia/fileepoch.config \
  --address https://0.0.0.0:51515 \
  --tls-cert-file ~/.config/kopia/fileepoch-cert.pem \
  --tls-key-file  ~/.config/kopia/fileepoch-key.pem \
  --tls-generate-cert \
  --server-username fileepoch \
  --server-password '<a long, unique password>'
```

- `--tls-generate-cert` creates a self-signed certificate and saves it to the two files. Use it
  **only the first time**: when the files exist, Kopia refuses to start with it. Remove it from
  the command (or your service file) afterwards.
- The first start prints `SERVER CERT SHA256: <64 hex characters>`. Write it down: FileEpoch shows
  the fingerprint of the server it reaches and you compare both before trusting it. For a
  certificate that already exists you get the same value with
  `openssl x509 -in fileepoch-cert.pem -noout -fingerprint -sha256` (upper case, with colons;
  FileEpoch accepts both forms).
- `--server-username` / `--server-password` are the user and password you type in FileEpoch. The
  password can also come from the environment variable `KOPIA_SERVER_PASSWORD`, which keeps it out
  of the process list. The repository password is a separate thing (`--password` or
  `KOPIA_PASSWORD`).
- Only this UI user can use the endpoints FileEpoch needs. Another user gets "access denied".
- Do not reuse the port or the users of an administration server. FileEpoch only reads, but a
  separate read-only server keeps a mistake (or someone else's tool) from changing anything.

## 3. Network

Open the port (51515 above) only to the computers that run FileEpoch, for example on your local
network or through a VPN. Do not expose it to the internet.

## 4. Connect

In FileEpoch enter `https://backup.example:51515`, the user from `--server-username`, compare the
fingerprint with the one from step 2, and enter the password. You can save it in the system's
credential store.

New snapshots appear in FileEpoch without restarting the server (it reloads the list every few
minutes and when its window gets focus).
