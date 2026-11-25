# Kopia web server API (discovery notes, phase 0)

> Kopia **does not document a stable public API**. Everything here was derived by reading the source
> code of **Kopia v0.23.1** (`internal/server/*.go`, `internal/serverapi`, `snapshot/`, `fs/`,
> `repo/object`) and of the web UI **kopia/htmlui** (commit `e9f6ace`, 2026-06-08, which is what
> `htmluibuild v0.0.1-0.20260608231842-7bf9fcc0831d` embedded in v0.23.1 was built from).
> It may change in any Kopia release, which is why the app checks the server version (§3).

Status markers used in the endpoint table:

- **Source** = verified by reading the v0.23.1 source code.
- **Live** = verified against a real 0.23.1 server.

**The JSON examples in this document were built from the Go types** (field names and formats taken
from the `json:"…"` tags); they are not recorded responses. The field names and types were then
compared with real responses (§8.1) and match. Anonymized recorded responses live with the client's
test fixtures.

Placeholder values used in this document (not real data): `https://kopia.example:51515`, UI user
`ui-user`, host `host1`, source user `user1`, source path `/data/docs`.

---

## 1. Summary

| Need                              | Endpoint                                               | CSRF   | Status |
|-----------------------------------|--------------------------------------------------------|--------|--------|
| CSRF token + server version       | `GET /`                                                | no     | Source, Live |
| Repository status (`readonly`)    | `GET /api/v1/repo/status`                              | yes    | Source, Live |
| List sources                      | `GET /api/v1/sources`                                  | yes    | Source, Live |
| List snapshots of a source        | `GET /api/v1/snapshots?host=&userName=&path=[&all=1]`  | yes    | Source, Live |
| List a directory                  | `GET /api/v1/objects/{oid}` (directory OID)            | **no** | Source, Live |
| Download a file (incl. Range)     | `GET /api/v1/objects/{oid}` (file OID)                 | **no** | Source, Live |
| Download a folder                 | *no endpoint*: recursive walk + one download per file  | —      | Source |

"Live" = checked on 2026-11-25 against Kopia 0.23.1 (`--readonly`, filesystem storage, three
sources, up to 12 snapshots per source) with [`tools/verify-api.ps1`](../tools/verify-api.ps1).
All checks in §8 passed. See §8.1 for what the live run added.

Whitelist for the app (method **and** path, nothing else):

```
GET  /
GET  /api/v1/repo/status
GET  /api/v1/sources
GET  /api/v1/snapshots
GET  /api/v1/objects/{oid}      {oid} must match ^(I+|Z)?[g-z]?[0-9a-f]{2,64}$
```

Any other method/path combination must be rejected inside the client, before anything goes out on
the network. See §7 for why: `--readonly` does **not** make the other endpoints safe.

---

## 2. Authentication

### 2.1 How it actually works

There is no "login session". The server requires **HTTP Basic on every request**
(`Server.isAuthenticated`, `internal/server/server.go`):

1. No `Authorization: Basic …` header → `401`, body `Missing credentials.`, header
   `WWW-Authenticate: Basic realm="Kopia"`.
2. Basic **and** a valid `Kopia-Auth` cookie for that user → accepted without checking the password
   (the cookie only caches a successful check, to skip password hashing).
3. Otherwise user/password are checked. Failure → `401`, body `Access denied.` (and the server logs
   a failed login attempt).
4. On success the server replies with `Set-Cookie: Kopia-Auth=<HS256 JWT>; Path=/`, valid for
   **1 minute**. Sending it back is optional; it reduces server load.

On top of that, almost every `/api/v1/*` route (those registered with `handleUI` /
`handleUIPossiblyNotConnected`) requires a **CSRF token** (§2.2). Missing or wrong →
`401`, body `Invalid or missing CSRF token.`

So a `curl -u user …/api/v1/…` with correct Basic credentials but no session cookie and no CSRF
header fails with exactly this last 401. Nothing like a "login" is missing; the session-cookie +
CSRF-token pair is.

There are **three different 401 bodies**, and the client must tell them apart to show a useful
message:

| Body (text/plain)                 | Meaning                               | Client action                                     |
|-----------------------------------|---------------------------------------|---------------------------------------------------|
| `Missing credentials.`            | No Basic header sent                  | Programming error                                 |
| `Access denied.`                  | Wrong user name or password           | Ask for the password again                        |
| `Invalid or missing CSRF token.`  | Token expired / server restarted      | Redo the CSRF bootstrap (§2.2) once, then retry   |

### 2.2 CSRF bootstrap

Source: `ServeStaticFiles` and `patchIndexBytes` in `server.go`, `generateCSRFToken` /
`validateCSRFToken` in `server_authz_checks.go`, `App.jsx` in htmlui.

1. `GET /` with Basic. The server:
   - sets `Set-Cookie: Kopia-Session-Cookie=<uuid>; Path=/` (unless the request already carried it);
   - returns `index.html` with `<meta name="kopia-csrf-token" content="<token>" />` inserted right
     before `</head>`.
2. Every `/api/v1/*` call then sends:
   - `Authorization: Basic …`
   - `Cookie: Kopia-Session-Cookie=<uuid>` (optionally also `Kopia-Auth=<jwt>`)
   - `X-Kopia-Csrf-Token: <token>`

The token is `hex(HMAC-SHA256(key, sessionID))`. The key (`authCookieSigningKey`) is a random UUID
generated **every time the server starts**, unless fixed with `--auth-cookie-signing-key`. So after
a server restart all tokens and cookies become invalid, and the client must repeat step 1 when it
gets `Invalid or missing CSRF token.`

`GET /api/v1/objects/{oid}` is registered with `csrfTokenNotRequired`: Basic is enough.

### 2.3 Which user may use these endpoints

Besides authenticating, the user must be **the server's UI user**, i.e. the value of
`--server-username` the server was started with (`requireUIUser`). Otherwise:

- `GET /` → `403`, `UI Access denied. See https://github.com/kopia/kopia/issues/880#…`
- `/api/v1/*` → `403`, JSON `{"code":"ACCESS_DENIED","error":"access is denied"}`
- `/api/v1/objects/{oid}` → `403`, text `access denied`

Users defined via `--htpasswd-file` or `kopia server user add` can authenticate, but only the UI
user can use these endpoints.

### 2.4 JSON error format

`handleUI*` routes return errors with `Content-Type: text/plain` (sic) and a JSON body:

```json
{ "code": "NOT_CONNECTED", "error": "not connected" }
```

Possible codes (`serverapi.APIErrorCode`): `INTERNAL`, `ALREADY_CONNECTED`, `ALREADY_INITIALIZED`,
`INVALID_PASSWORD`, `INVALID_TOKEN`, `MALFORMED_REQUEST`, `NOT_CONNECTED`, `NOT_FOUND`,
`NOT_INITIALIZED`, `PATH_NOT_FOUND`, `STORAGE_CONNECTION`, `ACCESS_DENIED`.
HTTP: 400 (bad request), 403 (access), 404, 500.

---

## 3. Server version and safety checks

**There is no version endpoint.** The version comes from the same `GET /` used for the CSRF
bootstrap: `patchIndexBytes` replaces

- `</title>` → ` v<version></title>` (e.g. `<title>KopiaUI v0.23.1</title>`)
- `<p class="version-info">Version ` → `<p class="version-info">Version v<version> <buildinfo> `

Development builds without a version get nothing appended. With `--ui-title-prefix` the prefix
appears before `KopiaUI`. The app extracts `v\d+\.\d+\.\d+` from `<title>` and, if it is not in the
list of tested versions, shows a clear warning.

`GET /api/v1/repo/status` embeds the `ClientOptions` fields, including `readonly`:

```json
{
  "connected": true,
  "configFile": "<path on the server>",
  "formatVersion": 3,
  "hash": "BLAKE2B-256-128",
  "encryption": "AES256-GCM-HMAC-SHA256",
  "splitter": "DYNAMIC-4M-BUZHASH",
  "storage": "filesystem",
  "supportsContentCompression": true,
  "hostname": "host1",
  "username": "user1",
  "readonly": true,
  "description": "My Repository",
  "enableActions": false
}
```

**App behaviour (decided):** if `readonly` is not `true`, the app connects but shows a clear,
persistent **warning** that the server can modify the repository (most likely it is pointed at an
admin server by mistake). The client-side whitelist still applies, so the app itself never sends a
modifying request either way.

If `connected` is `false` the server has no repository open (`initTaskID` may say it is being
opened): show "server has no repository" and retry.

### 3.1 TLS certificate and fingerprint

Kopia compares `hex(SHA-256(DER certificate))`, lower case, no separators
(`internal/tlsutil/tlsutil.go`, `verifyPeerCertificate`), and prints it that way on startup:
`SERVER CERT SHA256: 0a1b2c…` (64 hex characters). `openssl x509 -fingerprint -sha256` gives the
same value in upper case with `:`. The app accepts both formats (normalize: strip `:` and spaces,
lower-case).

Kopia accepts a match on **any** certificate in the presented chain. Rewind deliberately pins only
the **end-entity (leaf)** certificate: without chain validation, "any certificate" would let an
attacker present their own leaf followed by the (public) pinned certificate. For Kopia's
self-signed certificates the leaf is the only certificate, so the fingerprint is the same.
Handshake signatures are still verified, so knowing the public certificate is not enough to
impersonate the server. Host names are not checked: the pin identifies the server, and generated
Kopia certificates often name a host other than the address used to connect.

Rewind checks the fingerprint with a TLS handshake **before** asking for the password, so a
password is never typed for, or sent to, an unexpected server.

---

## 4. Sources

`GET /api/v1/sources` — CSRF: yes. Optional query filters: `host`, `userName`, `path` (exact match).

```json
{
  "localUsername": "user1",
  "localHost": "host1",
  "multiUser": true,
  "sources": [
    {
      "source": { "host": "host1", "userName": "user1", "path": "/data/docs" },
      "status": "IDLE",
      "schedule": { "intervalSeconds": 7200 },
      "lastSnapshot": { "id": "…", "source": { … }, "startTime": "…", "endTime": "…",
                        "stats": { … }, "rootEntry": { "name": "docs", "type": "d", "obj": "k…", "summ": { … } } },
      "nextSnapshotTime": "2026-01-01T12:00:00Z"
    }
  ]
}
```

Notes:

- Returns **all** sources in the repository (every user@host:path that has snapshots or its own
  policy), not only the local host's. The Kopia web UI filters by `localHost`/`localUsername` on the
  client side; our app shows all of them, grouped by `user@host`.
- The list is computed when the server starts and on every server *refresh* (every 4 h by default,
  `--refresh-interval`). A new source may take a while to show up, and `lastSnapshot` may be stale.
- `status` and `nextSnapshotTime` are irrelevant on a read-only server (it does not schedule
  snapshots: `isLocal()` is false when `readonly`).

---

## 5. Snapshots of a source

`GET /api/v1/snapshots?host=<h>&userName=<u>&path=<p>[&all=1]` — CSRF: yes.

- All three parameters must be given; they are used as an exact `SourceInfo`.
- Order: **oldest first** (`snapshot.SortByTime(manifests, false)`: ascending by `startTime`, ties
  broken by `endTime`).
- Without `all`: the server **groups** snapshots with the same `rootID` (identical content). Because
  it walks the list in ascending order it keeps the **oldest** snapshot of each group and merges the
  others' `retention`/`pins` into it. This is the web UI's "hide identical snapshots". With
  `all=<any non-empty value>` it returns all of them.
- Consequence for the timeline: a group of identical snapshots is shown with the date of the first
  one ("content unchanged since…"). To also show "until…", the app requests `all=1` and groups by
  `rootID` itself (cheap: same list).

```json
{
  "snapshots": [
    {
      "id": "a1b2c3…",
      "description": "",
      "startTime": "2026-01-01T10:00:00.123456789Z",
      "endTime":   "2026-01-01T10:02:13.5Z",
      "incomplete": "checkpoint",
      "summary": { "size": 123456789, "files": 1200, "symlinks": 0, "dirs": 80,
                   "maxTime": "2025-12-31T18:00:00Z", "numFailed": 0 },
      "rootID": "k0123456789abcdef0123456789abcdef",
      "retention": ["latest-1", "hourly-1", "daily-1"],
      "pins": []
    }
  ],
  "unfilteredCount": 240,
  "uniqueCount": 37
}
```

- `incomplete` only appears on incomplete snapshots (checkpoints); the app marks them.
- `rootID` is the OID of the snapshot's root directory → entry point for §6.
- **Timestamps:** RFC 3339 with nanoseconds, always **UTC** (`fs.UTCTimestamp.MarshalJSON`). The app
  converts them to the user's local time for display.

---

## 6. Directories and files: `GET /api/v1/objects/{oid}`

A single endpoint lists directories and downloads files (`handleObjectGet`, `api_object_get.go`).
CSRF: **no**. Registered for GET only. Note that a HEAD request does not get a 405: it falls through
to the static-file handler (`PathPrefix("/")`), so HEAD cannot be used to query sizes. Always use
GET (the size is already in the directory entry).

**OID grammar** (`repo/object/objectid.go`, `repo/content/index/id.go`):

```
OID      = "I"* ["Z"] CONTENT          ("I" and "Z" are mutually exclusive)
CONTENT  = [prefix g..z] HEX           (HEX: hash in hex, even length, up to 64)
```

- `I` (one or more) = chunked object with an index: used by **large files** and by large directories.
- `Z` = compressed object.
- Content prefix `k` = directory metadata; regular file contents have no prefix.

**Rule for the client:** decide whether something is a file or a directory from the `type` field of
its entry (or because it is a snapshot's `rootID`), **never** from the OID prefix. The whitelist
check on `{oid}` accepts the whole grammar above, `^(I+|Z)?[g-z]?[0-9a-f]{2,64}$`, and nothing else
(which also rules out injecting `/`, `?` or `..`).

### 6.1 Directory

For a directory OID the response is `application/json` containing the `DirManifest` exactly as
stored in the repository (the server does not generate it; it is Kopia's internal format):

```json
{
  "stream": "kopia:directory",
  "entries": [
    { "name": "Report.pdf", "type": "f", "mode": "0644", "size": 3355443,
      "mtime": "2025-11-15T16:50:00Z", "uid": 1000, "gid": 1000, "obj": "8f3e…" },
    { "name": "Photos", "type": "d", "mode": "0755",
      "mtime": "2025-11-14T10:00:00Z", "uid": 1000, "gid": 1000, "obj": "k9a1b…",
      "summ": { "size": 987654321, "files": 4200, "symlinks": 0, "dirs": 35,
                "maxTime": "2025-11-14T09:59:00Z", "numFailed": 0 } },
    { "name": "current", "type": "s", "mode": "0777", "size": 12,
      "mtime": "2025-11-01T00:00:00Z", "obj": "c41d…" }
  ],
  "summary": { "size": …, "files": …, "dirs": …, "maxTime": "…", "numFailed": 0 }
}
```

- `type`: `f` file, `d` directory, `s` symbolic link (empty = unknown).
- `mode`: permissions as an **octal string**; omitted when 0.
- `size`, `uid`, `gid`, `summ` are omitted when 0/null (`omitempty`): the parser must tolerate
  missing fields.
- `mtime`: UTC, RFC 3339 with nanoseconds.
- **No pagination.** A directory with 100,000 entries arrives as a single JSON document (tens of MB).
  Lazy loading has to happen on the client: parse in the background (streaming if needed),
  virtualize the list, and only load subdirectories when opened.
- There is no endpoint to resolve a path (`/a/b/c`) inside a snapshot: navigate down from `rootID`
  one entry at a time. (`POST /api/v1/paths/resolve` exists but resolves paths on the **server's
  file system**, not inside a snapshot, and is not whitelisted.)

### 6.2 File

For a file OID the response is the content, served with `http.ServeContent`:

- Supports **Range** (`Range: bytes=…` → 206), so large downloads can be resumed.
- `Content-Type` is guessed by Go from the `fname` extension or by sniffing; do not rely on it.
- Optional query `fname=<name>` → adds `Content-Disposition: attachment; filename="<name>"`.
  The app does not need it (and the name is not escaped, so better not to use it).
- Optional query `mtime=<RFC3339Nano>` → used as `Last-Modified`. Not needed.
- For symbolic links (`type: "s"`) the object contains the link **target as text**.

### 6.3 Errors

| Case                             | Response                                       |
|----------------------------------|------------------------------------------------|
| Malformed OID                    | 400 `invalid object id`                        |
| Unknown OID                      | 404 `object not found`                         |
| User is not the UI user          | 403 `access denied`                            |
| Any other error opening object   | Not handled in the code (see §9): probably 500 or a dropped connection |

### 6.4 Downloading folders

There is **no** folder download endpoint (zip/tar) that writes on the client. The only zip/tar
support is `POST /api/v1/restore`, which **writes to the server's disk** and is forbidden (§7).

Restoring a folder = walk it recursively from its OID (`GET objects/{oid}` for every subdirectory)
and download each file with `GET objects/{oid}`. The entry metadata (`mtime`, `mode`, `size`) lets
the app restore modification times and check sizes.

**Verification:** Kopia OIDs are derived from a hash keyed with a secret of the repository, so the
client **cannot** recompute a comparable hash. Possible checks: size (`size`) and modification time
(`mtime`). TLS already protects integrity in transit.

### 6.5 Restore rules that follow from the data (decided)

- **File names that are invalid on Windows** (snapshots usually come from Linux): every invalid
  character (`< > : " / \ | ? *`, control characters) is replaced with `_`; reserved names (`CON`,
  `PRN`, `AUX`, `NUL`, `COM1`…`COM9`, `LPT1`…`LPT9`, with or without extension) and names ending in a
  dot or space get a `_` appended. Renamed items are listed in the restore summary. Names that only
  differ in case and would collide on Windows are handled by the conflict policy (no overwrite by
  default) and also reported.
- **Symbolic links** are **not** restored; they are skipped and listed in the restore summary.

---

## 7. Forbidden endpoints, and why `--readonly` is not enough

`kopia server start --readonly` opens the repository with its storage wrapped in a `readonly`
wrapper (`repo/open.go`): this **prevents writing repository blobs**, but the server still registers
every route, and several of them act on the **server host** or on server state rather than on the
repository:

| Route                                                         | Effect even with `--readonly`                          |
|---------------------------------------------------------------|--------------------------------------------------------|
| `POST /api/v1/restore`                                        | Creates files/zip/tar on the server's disk             |
| `POST /api/v1/mounts`, `DELETE /api/v1/mounts/{id}`           | Mounts/unmounts snapshots on the server                |
| `POST /api/v1/repo/disconnect`                                | Disconnects the repository (takes the server offline)  |
| `POST /api/v1/repo/connect`, `/repo/create`, `/repo/exists`   | Connects/creates repositories from the server          |
| `PUT /api/v1/ui-preferences`                                  | Writes a JSON file on the server                       |
| `PUT /api/v1/repo/throttle`                                   | Changes server throttling                              |
| `POST /api/v1/tasks/{id}/cancel`                              | Cancels server tasks                                   |
| `POST /api/v1/refresh`, `POST /api/v1/repo/sync`              | Forces a server refresh (probably harmless, see §9)    |
| `POST /api/v1/sources`, `/sources/upload`, `/sources/cancel`  | Create sources/snapshots (fail on readonly, must not be attempted) |
| `POST /api/v1/snapshots/delete`, `/snapshots/edit`            | Delete/edit snapshots                                  |
| `PUT/DELETE /api/v1/policy`, `POST /api/v1/policy/resolve`    | Policies                                               |
| `POST/DELETE /api/v1/notificationProfiles…`, `POST /api/v1/testNotificationProfile` | Notifications (sends messages)   |
| `POST /api/v1/estimate`                                       | Starts a task that walks the server's file system      |
| `/api/v1/control/*`                                           | Server control API (separate `server-control` user)    |
| gRPC `/kopia_repository.KopiaRepository/*`                    | Repository API for kopia clients                       |

Hence the whitelist (§1) is **exact and GET-only**, and the client enforces it by construction
(there is no way to build a request to any other route), not by convention.

The following are GET and read-only but are not used, because they are not needed:
`/api/v1/policy`, `/api/v1/policies`, `/api/v1/cli` (reveals the server's executable and config
paths), `/api/v1/repo/algorithms`, `/api/v1/repo/throttle`, `/api/v1/mounts…`,
`/api/v1/current-user`, `/api/v1/ui-preferences`, `/api/v1/tasks…`, `/api/v1/notificationProfiles…`.
They can be whitelisted later if there is a reason.

---

## 8. Live verification

Run [`tools/verify-api.ps1`](../tools/verify-api.ps1) (PowerShell 7.5+). It asks for the password
without echoing it, pins the certificate fingerprint, only sends the whitelisted GET requests, and
checks the claims in this document:

```powershell
pwsh -File tools/verify-api.ps1 -Server https://kopia.example:51515 -User ui-user `
     -Fingerprint <64 hex chars> [-Source <part of a source path>]
```

| Step | Checks |
|------|--------|
| 1 | `GET /` → 200, CSRF token in page, version in `<title>`, session cookie set |
| 2 | `/api/v1/sources` without the CSRF header → 401 `Invalid or missing CSRF token.` |
| 3 | `repo/status`: `connected`, `readonly` |
| 4 | `sources` |
| 5 | `snapshots` grouped and `all=1` per source; order oldest → newest; newest snapshot vs. the `lastSnapshot` cached in `sources` (freshness, §9 item 2) |
| 6 | Root directory of the newest snapshot without CSRF header; size and time of the JSON |
| 7 | `Range: bytes=0-99` on one file → 206 with 100 bytes |

Raw responses are saved to `local/api-samples/` (git-ignored). They contain real file names: do not
commit them without anonymizing.

### 8.1 Findings from the live run (2026-11-25, Kopia 0.23.1)

- Every step passed: CSRF bootstrap, the `Invalid or missing CSRF token.` 401, `readonly: true`,
  sources, snapshots, a directory listing without the CSRF header, and a `Range` request (206).
- `GET /` sets both `Kopia-Session-Cookie` and `Kopia-Auth`.
- `<title>` is exactly `KopiaUI v0.23.1`.
- Snapshots come back **oldest first**, and the grouped list keeps the **oldest** snapshot of each
  `rootID` group (checked: 12 snapshots → 4 groups).
- `retention` values look like `latest-7..12`, `daily-2` (ranges are compacted when grouping).
- Fractional seconds in timestamps vary in length (`…:19.636406861Z`, `…:17.514896Z`): the parser
  must accept 0–9 digits.
- On a read-only server `sources[].status` is `REMOTE`, and `schedule` may contain
  `intervalSeconds`, `timeOfDay: [{hour, min}]` and `runMissed`.
- `lastSnapshot` in `/api/v1/sources` is only updated when the server starts or refreshes
  (`sourceManager.runReadOnly` never refreshes it), so the app takes the newest snapshot from
  `/api/v1/snapshots`, never from `sources`.
- A file OID without any prefix (plain hex) was downloaded, which confirms the OID grammar in §6.
- Field names and JSON types of `repo/status`, `sources`, `snapshots` and the directory manifest
  match §3–§6.

---

## 9. Open questions

1. **Not yet exercised live:** the `Access denied.` 401 (wrong password; deliberately not tested, it
   writes a failed-login entry to the server log), the 403 for a non-UI user, behaviour after a
   server restart (CSRF re-bootstrap), and a very large directory.
2. **Data freshness.** Sources are only recomputed on each server *refresh*
   (`--refresh-interval`, 4 h by default). Snapshot lists are read on demand, but the repository
   index may be cached. Do new snapshots show up without calling `POST /api/v1/repo/sync`? The
   first live run was inconclusive (a server refresh had happened after the newest snapshot). Rerun
   `tools/verify-api.ps1` a few minutes after a scheduled snapshot: it prints `FRESHNESS: …` when the
   snapshot list is ahead of the sources cache. If new snapshots do not appear, choose between
   (a) whitelisting `POST /api/v1/repo/sync` (does not write to the repository, but is a POST and
   triggers work on the server) or (b) recommending a shorter `--refresh-interval`.
3. **Unhandled errors in `handleObjectGet`.** If `OpenObject` fails with anything other than "not
   found", the code carries on with a nil object. Expect a 500 or a dropped connection; the client
   treats it as a recoverable error and retries once.
4. **Huge directories.** No pagination; measure the size/time of the largest directory's JSON live
   and decide whether streaming parsing is needed.
5. **Concurrency.** The HTTP server has no explicit limit on parallel downloads
   (`--max-concurrency` only affects gRPC). Proposed default: 4 concurrent downloads, configurable.
6. **Other Kopia versions.** Only 0.23.1 has been read. The routes used have existed for many
   releases, but no other version has been tested.
