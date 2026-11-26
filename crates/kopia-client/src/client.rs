use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use reqwest::header::{AUTHORIZATION, COOKIE, HeaderMap, HeaderValue, RANGE, SET_COOKIE};
use reqwest::{StatusCode, Url};
use serde::de::DeserializeOwned;

use crate::dircache::{DEFAULT_MAX_ENTRIES, DirCache};
use crate::endpoint::Endpoint;
use crate::tls::{MismatchSlot, pinned_config};
use crate::{
    DirEntry, DirManifest, EntryType, Error, Fingerprint, Oid, RepoStatus, Result, SnapshotsResponse, SourceInfo,
    SourcesResponse,
};

/// Kopia server versions this client has been tested against.
pub const TESTED_VERSIONS: &[&str] = &["0.23.1"];

const SESSION_COOKIE: &str = "Kopia-Session-Cookie";
const AUTH_COOKIE: &str = "Kopia-Auth";
const CSRF_HEADER: &str = "X-Kopia-Csrf-Token";
/// The start page is a few kilobytes; refuse to buffer anything absurd.
const MAX_INDEX_BYTES: usize = 1 << 20;

/// Validated server address: `https://host[:port]` with no path, query or credentials.
#[derive(Clone, PartialEq, Eq)]
pub struct ServerAddress(Url);

impl ServerAddress {
    pub fn host(&self) -> String {
        match self.0.host() {
            Some(url::Host::Ipv6(a)) => a.to_string(),
            Some(h) => h.to_string(),
            None => String::new(),
        }
    }

    pub fn port(&self) -> u16 {
        self.0.port_or_known_default().unwrap_or(443)
    }

    /// `host:port`, used as part of the credential-store account name.
    pub fn authority(&self) -> String {
        format!("{}:{}", self.0.host_str().unwrap_or_default(), self.port())
    }

    pub(crate) fn url(&self) -> &Url {
        &self.0
    }
}

impl FromStr for ServerAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let bad = |why: &str| Error::InvalidServerAddress(format!("{s}: {why}"));
        let url = Url::parse(s.trim()).map_err(|e| bad(&e.to_string()))?;
        if url.scheme() != "https" {
            return Err(bad("only https:// is supported"));
        }
        if url.host().is_none() {
            return Err(bad("missing host"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(bad("do not put credentials in the address"));
        }
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(bad("use only scheme, host and port"));
        }
        Ok(ServerAddress(url))
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "https://{}", self.authority())
    }
}

impl fmt::Debug for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServerAddress({self})")
    }
}

/// Conditions worth showing to the user that do not prevent using the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// The server version is not in [`TESTED_VERSIONS`] (or could not be read).
    UntestedVersion(Option<String>),
    /// The server's repository connection is not read-only: it could modify backups.
    /// (This client still only sends whitelisted read requests.)
    NotReadOnly,
}

/// Result of [`KopiaClient::connect`].
#[derive(Debug, Clone)]
pub struct ConnectInfo {
    /// Kopia version from the start page title, e.g. `0.23.1`.
    pub version: Option<String>,
    pub repo: RepoStatus,
    pub warnings: Vec<Warning>,
}

#[derive(Default)]
struct Session {
    session_cookie: Option<String>,
    auth_cookie: Option<String>,
    csrf: Option<String>,
    version: Option<String>,
}

/// Client for one Kopia server and user.
///
/// Cheap to share behind an `Arc`; requests can run concurrently.
pub struct KopiaClient {
    http: reqwest::Client,
    server: ServerAddress,
    auth: HeaderValue,
    mismatch: MismatchSlot,
    session: Mutex<Session>,
    bootstrap_lock: tokio::sync::Mutex<()>,
    cache: DirCache,
}

/// Result of [`KopiaClient::resolve_path`].
#[derive(Debug, Clone)]
pub struct ResolvedPath {
    /// How many components of the requested path exist (0 = only the snapshot root).
    pub depth: usize,
    /// Object ID of the deepest existing folder.
    pub dir: Oid,
    /// Its listing.
    pub listing: Arc<DirManifest>,
}

impl fmt::Debug for KopiaClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KopiaClient").field("server", &self.server).finish_non_exhaustive()
    }
}

impl KopiaClient {
    /// Creates a client. No network traffic happens until [`connect`](Self::connect).
    ///
    /// The password is only kept inside the encoded `Authorization` header, which is marked
    /// sensitive. Callers should zeroize their own copy.
    pub fn new(server: ServerAddress, user: &str, password: &str, fingerprint: Fingerprint) -> Result<Self> {
        if user.is_empty() || user.contains(':') {
            return Err(Error::Other("invalid user name".into()));
        }
        let mut raw = zeroize::Zeroizing::new(format!("{user}:{password}"));
        let encoded = zeroize::Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(raw.as_bytes()));
        raw.clear();
        let mut auth = HeaderValue::from_str(&format!("Basic {}", encoded.as_str()))
            .map_err(|_| Error::Other("invalid characters in credentials".into()))?;
        auth.set_sensitive(true);

        let mismatch: MismatchSlot = Arc::default();
        let tls = pinned_config(fingerprint, mismatch.clone())?;
        let http = reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .no_proxy()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(120))
            .user_agent(concat!("FileEpoch/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| Error::Other(format!("HTTP client: {e}")))?;

        Ok(KopiaClient {
            http,
            server,
            auth,
            mismatch,
            session: Mutex::default(),
            bootstrap_lock: tokio::sync::Mutex::new(()),
            cache: DirCache::new(DEFAULT_MAX_ENTRIES),
        })
    }

    pub fn server(&self) -> &ServerAddress {
        &self.server
    }

    /// Kopia version read from the start page, once connected.
    pub fn server_version(&self) -> Option<String> {
        self.lock_session().version.clone()
    }

    /// Starts a session (CSRF token, version) and reads the repository status.
    ///
    /// Returns warnings instead of failing for an untested version or a server that is not
    /// read-only; the caller decides how to show them.
    pub async fn connect(&self) -> Result<ConnectInfo> {
        self.bootstrap(None).await?;
        let repo = self.repo_status().await?;
        if !repo.connected {
            return Err(Error::NotConnected);
        }
        let version = self.server_version();
        let mut warnings = Vec::new();
        if !version.as_deref().is_some_and(|v| TESTED_VERSIONS.contains(&v)) {
            warnings.push(Warning::UntestedVersion(version.clone()));
        }
        if !repo.readonly {
            warnings.push(Warning::NotReadOnly);
        }
        Ok(ConnectInfo { version, repo, warnings })
    }

    pub async fn repo_status(&self) -> Result<RepoStatus> {
        self.get_json(&Endpoint::RepoStatus).await
    }

    pub async fn sources(&self) -> Result<SourcesResponse> {
        self.get_json(&Endpoint::Sources).await
    }

    /// Snapshots of a source, **oldest first**. With `all == false` the server keeps only the
    /// oldest snapshot of each run of identical content (same `root_id`).
    pub async fn snapshots(&self, source: &SourceInfo, all: bool) -> Result<SnapshotsResponse> {
        self.get_json(&Endpoint::Snapshots { source, all }).await
    }

    /// Lists a directory. The whole manifest is returned at once (Kopia has no pagination).
    /// Listings are cached by object ID (immutable content hashes).
    pub async fn list_directory(&self, dir: &Oid) -> Result<Arc<DirManifest>> {
        if let Some(m) = self.cache.get(dir) {
            return Ok(m);
        }
        let m: Arc<DirManifest> = Arc::new(self.get_json(&Endpoint::Object(dir)).await?);
        self.cache.insert(dir.clone(), m.clone());
        Ok(m)
    }

    /// Drops all cached directory listings.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Follows `path` (folder names) down from the snapshot root `root` **as far as it exists**.
    ///
    /// This is what keeps the user in "the same folder" while moving through time: if a folder
    /// does not exist in this snapshot (or is now a file), the deepest existing ancestor is
    /// returned instead, and [`ResolvedPath::depth`] tells how many components were found.
    pub async fn resolve_path(&self, root: &Oid, path: &[String]) -> Result<ResolvedPath> {
        let mut dir = root.clone();
        let mut listing = self.list_directory(&dir).await?;
        let mut depth = 0;
        for name in path {
            let next = listing.entries.iter().find(|e| e.name == *name && e.entry_type == EntryType::Directory);
            let Some(next) = next else { break };
            dir = next.obj.clone();
            listing = self.list_directory(&dir).await?;
            depth += 1;
        }
        Ok(ResolvedPath { depth, dir, listing })
    }

    /// Walks `path` (components separated by `/`, empty components ignored) down from the
    /// snapshot root `root` and returns the entry it names.
    pub async fn find_entry(&self, root: &Oid, path: &str) -> Result<DirEntry> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            return Err(Error::Other("empty path".into()));
        }
        let mut dir = root.clone();
        for (i, part) in parts.iter().enumerate() {
            let listing = self.list_directory(&dir).await?;
            let entry = listing
                .entries
                .iter()
                .find(|e| e.name == *part)
                .cloned()
                .ok_or_else(|| Error::NotFound(parts[..=i].join("/")))?;
            if i + 1 == parts.len() {
                return Ok(entry);
            }
            if entry.entry_type != EntryType::Directory {
                return Err(Error::NotFound(format!("{} is not a directory", parts[..=i].join("/"))));
            }
            dir = entry.obj;
        }
        unreachable!("loop returns on the last component")
    }

    /// Opens a file object for streaming. With `offset`, requests the rest of the file from that
    /// byte (HTTP Range), which allows resuming downloads.
    pub async fn open_object(&self, oid: &Oid, offset: Option<u64>) -> Result<ObjectStream> {
        let range = offset.map(|o| format!("bytes={o}-"));
        let resp = self.send(&Endpoint::Object(oid), range.as_deref()).await?;
        if offset.is_some_and(|o| o > 0) && resp.status() != StatusCode::PARTIAL_CONTENT {
            return Err(self.incompatible("the server ignored a Range request".into()));
        }
        Ok(ObjectStream { resp })
    }

    // --- internals ------------------------------------------------------------------------------

    fn lock_session(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn incompatible(&self, detail: String) -> Error {
        Error::Incompatible { version: self.server_version(), detail }
    }

    async fn get_json<T: DeserializeOwned>(&self, ep: &Endpoint<'_>) -> Result<T> {
        let resp = self.send(ep, None).await?;
        let bytes = resp.bytes().await.map_err(|e| self.transport_error(e))?;
        serde_json::from_slice(&bytes).map_err(|e| self.incompatible(format!("cannot parse {}: {e}", ep_name(ep))))
    }

    /// GET / : obtains (or refreshes) the CSRF token and reads the server version.
    /// `stale_token` is the token a failed request used; if another task already replaced it,
    /// nothing is done.
    async fn bootstrap(&self, stale_token: Option<&str>) -> Result<()> {
        let _guard = self.bootstrap_lock.lock().await;
        if let Some(stale) = stale_token
            && self.lock_session().csrf.as_deref().is_some_and(|t| t != stale)
        {
            return Ok(());
        }
        let (resp, _) = self.raw_send(&Endpoint::Index, None).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(classify_failure(status, &body, &Endpoint::Index));
        }
        let bytes = resp.bytes().await.map_err(|e| self.transport_error(e))?;
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(self.incompatible("start page too large".into()));
        }
        let page = String::from_utf8_lossy(&bytes);
        let version = parse_version(&page);
        let token = parse_csrf_token(&page);
        let mut s = self.lock_session();
        s.version = version.clone();
        match token {
            Some(t) => {
                s.csrf = Some(t);
                Ok(())
            }
            None => Err(Error::Incompatible { version, detail: "no CSRF token in the start page".into() }),
        }
    }

    /// The only place that sends requests. Retries once with a new CSRF token if the server
    /// rejected the current one (e.g. after a server restart).
    async fn send(&self, ep: &Endpoint<'_>, range: Option<&str>) -> Result<reqwest::Response> {
        let mut retried = false;
        loop {
            if ep.needs_csrf() && self.lock_session().csrf.is_none() {
                self.bootstrap(None).await?;
            }
            let (resp, token_used) = self.raw_send(ep, range).await?;
            let status = resp.status();
            if status.is_success() {
                return Ok(resp);
            }
            let body = resp.text().await.unwrap_or_default();
            match classify_failure(status, &body, ep) {
                Error::CsrfRejected if ep.needs_csrf() && !retried => {
                    retried = true;
                    self.bootstrap(token_used.as_deref()).await?;
                }
                e => return Err(e),
            }
        }
    }

    /// Sends one whitelisted GET request and records cookies. No retries, no bootstrap.
    async fn raw_send(&self, ep: &Endpoint<'_>, range: Option<&str>) -> Result<(reqwest::Response, Option<String>)> {
        let (headers, token_used) = self.request_headers(ep, range);
        let resp = self.http.get(ep.url(self.server.url())).headers(headers).send().await;
        let resp = resp.map_err(|e| self.transport_error(e))?;
        self.store_cookies(resp.headers());
        Ok((resp, token_used))
    }

    fn request_headers(&self, ep: &Endpoint<'_>, range: Option<&str>) -> (HeaderMap, Option<String>) {
        let s = self.lock_session();
        let mut h = HeaderMap::new();
        h.insert(AUTHORIZATION, self.auth.clone());
        let cookies: Vec<String> = [(SESSION_COOKIE, &s.session_cookie), (AUTH_COOKIE, &s.auth_cookie)]
            .into_iter()
            .filter_map(|(n, v)| v.as_ref().map(|v| format!("{n}={v}")))
            .collect();
        if !cookies.is_empty()
            && let Ok(mut v) = HeaderValue::from_str(&cookies.join("; "))
        {
            v.set_sensitive(true);
            h.insert(COOKIE, v);
        }
        let mut token_used = None;
        if ep.needs_csrf()
            && let Some(t) = &s.csrf
            && let Ok(mut v) = HeaderValue::from_str(t)
        {
            v.set_sensitive(true);
            h.insert(CSRF_HEADER, v);
            token_used = Some(t.clone());
        }
        if let Some(r) = range.and_then(|r| HeaderValue::from_str(r).ok()) {
            h.insert(RANGE, r);
        }
        (h, token_used)
    }

    fn store_cookies(&self, headers: &HeaderMap) {
        let mut s = self.lock_session();
        for value in headers.get_all(SET_COOKIE) {
            let Ok(value) = value.to_str() else { continue };
            let first = value.split(';').next().unwrap_or_default();
            let Some((name, val)) = first.split_once('=') else { continue };
            let val = val.trim().to_owned();
            match name.trim() {
                SESSION_COOKIE => s.session_cookie = Some(val),
                AUTH_COOKIE => s.auth_cookie = Some(val),
                _ => {}
            }
        }
    }

    fn transport_error(&self, e: reqwest::Error) -> Error {
        if let Some(fp) = self.mismatch.lock().unwrap_or_else(|e| e.into_inner()).take() {
            return Error::CertificateMismatch { presented: fp.to_hex() };
        }
        let mut msg = e.to_string();
        let mut src = std::error::Error::source(&e);
        while let Some(s) = src {
            msg.push_str(": ");
            msg.push_str(&s.to_string());
            src = s.source();
        }
        Error::Connection(msg)
    }
}

/// A file being downloaded. Read it chunk by chunk; never buffer whole files.
pub struct ObjectStream {
    resp: reqwest::Response,
}

impl ObjectStream {
    /// Number of bytes the server will send, if known.
    pub fn content_length(&self) -> Option<u64> {
        self.resp.content_length()
    }

    /// Next chunk, or `None` at the end.
    pub async fn chunk(&mut self) -> Result<Option<bytes::Bytes>> {
        self.resp.chunk().await.map_err(|e| Error::Connection(format!("download interrupted: {e}")))
    }
}

fn ep_name(ep: &Endpoint<'_>) -> String {
    match ep {
        Endpoint::Index => "start page".into(),
        Endpoint::RepoStatus => "repository status".into(),
        Endpoint::Sources => "sources".into(),
        Endpoint::Snapshots { source, .. } => format!("snapshots of {source}"),
        Endpoint::Object(oid) => format!("object {oid}"),
    }
}

/// Maps a non-success response to an error (see `docs/API.md` §2.1, §2.3, §6.3).
fn classify_failure(status: StatusCode, body: &str, ep: &Endpoint<'_>) -> Error {
    match status {
        StatusCode::UNAUTHORIZED if body.contains("CSRF") => Error::CsrfRejected,
        StatusCode::UNAUTHORIZED if body.starts_with("Missing credentials") => Error::MissingCredentials,
        StatusCode::UNAUTHORIZED => Error::AccessDenied,
        StatusCode::FORBIDDEN => Error::NotUiUser,
        StatusCode::NOT_FOUND => Error::NotFound(ep_name(ep)),
        _ => api_error(status, body),
    }
}

fn api_error(status: StatusCode, body: &str) -> Error {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        code: Option<String>,
        error: Option<String>,
    }
    match serde_json::from_str::<ErrorBody>(body) {
        Ok(b) if b.code.as_deref() == Some("NOT_CONNECTED") => Error::NotConnected,
        Ok(b) => Error::Api { status: status.as_u16(), code: b.code, message: b.error.unwrap_or_default() },
        Err(_) => Error::Api { status: status.as_u16(), code: None, message: body.trim().chars().take(200).collect() },
    }
}

/// `<meta name="kopia-csrf-token" content="…" />`
pub(crate) fn parse_csrf_token(page: &str) -> Option<String> {
    let marker = r#"name="kopia-csrf-token" content=""#;
    let start = page.find(marker)? + marker.len();
    let len = page[start..].find('"')?;
    let token = &page[start..start + len];
    (!token.is_empty() && token.bytes().all(|b| b.is_ascii_hexdigit())).then(|| token.to_owned())
}

/// Version from `<title>KopiaUI v0.23.1</title>`.
pub(crate) fn parse_version(page: &str) -> Option<String> {
    let start = page.find("<title>")? + "<title>".len();
    let end = start + page[start..].find("</title>")?;
    let title = &page[start..end];
    title.match_indices('v').find_map(|(i, _)| {
        let rest = &title[i + 1..];
        let candidate: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let parts: Vec<&str> = candidate.split('.').collect();
        (parts.len() >= 3 && parts.iter().take(3).all(|p| !p.is_empty())).then(|| parts[..3].join("."))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_start_page() {
        let page = r#"<html><head><title>KopiaUI v0.23.1</title><meta name="kopia-csrf-token" content="0a1b2c" /></head></html>"#;
        assert_eq!(parse_version(page).as_deref(), Some("0.23.1"));
        assert_eq!(parse_csrf_token(page).as_deref(), Some("0a1b2c"));
        let prefixed = "<title>Backup server KopiaUI v0.24.0-beta1</title>";
        assert_eq!(parse_version(prefixed).as_deref(), Some("0.24.0"));
        assert_eq!(parse_version("<title>KopiaUI</title>"), None);
        assert_eq!(parse_csrf_token(r#"name="kopia-csrf-token" content="<script>""#), None);
    }

    #[test]
    fn server_address_validation() {
        assert!("https://kopia.example:51515".parse::<ServerAddress>().is_ok());
        assert!("https://kopia.example:51515/".parse::<ServerAddress>().is_ok());
        assert!("https://[::1]:51515".parse::<ServerAddress>().is_ok());
        for bad in [
            "http://kopia.example:51515",
            "https://user:pw@kopia.example",
            "https://kopia.example/api",
            "https://kopia.example/?x=1",
            "kopia.example",
        ] {
            assert!(bad.parse::<ServerAddress>().is_err(), "{bad}");
        }
        let a: ServerAddress = "https://kopia.example:51515".parse().unwrap();
        assert_eq!(a.authority(), "kopia.example:51515");
        assert_eq!(a.to_string(), "https://kopia.example:51515");
    }

    #[test]
    fn api_error_mapping() {
        assert!(matches!(
            api_error(StatusCode::BAD_REQUEST, r#"{"code":"NOT_CONNECTED","error":"not connected"}"#),
            Error::NotConnected
        ));
        match api_error(StatusCode::INTERNAL_SERVER_ERROR, r#"{"code":"INTERNAL","error":"boom"}"#) {
            Error::Api { status: 500, code: Some(c), message } => {
                assert_eq!(c, "INTERNAL");
                assert_eq!(message, "boom");
            }
            other => panic!("{other:?}"),
        }
    }
}
