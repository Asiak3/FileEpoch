//! Integration tests against a mock Kopia server (TLS with a generated self-signed certificate).
//!
//! The mock reproduces the behaviour documented in docs/API.md §2: Basic auth on every request,
//! the short-lived `Kopia-Auth` cookie, the session cookie + CSRF token issued by `GET /`, a
//! signing key that changes when the server "restarts", the UI-user check, and Range downloads.
//! Responses come from anonymized recordings of a real Kopia 0.23.1 server (tests/fixtures).

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use bytes::Bytes;
use chrono::{Local, TimeZone, Utc};
use http_body_util::Full;
use hyper::{Request, Response, StatusCode};
use std::sync::atomic::AtomicBool;

use kopia_client::restore::{
    ConflictPolicy, Naming, RestoreOptions, SuffixDate, Summary, existing_targets, restore_items, root_entry,
};
use kopia_client::{EntryType, Error, Fingerprint, KopiaClient, Oid, ServerAddress, SnapshotsResponse, Warning};

const USER: &str = "ui-user";
const PASSWORD: &str = "correct horse";
const OTHER_USER: &str = "backup-user";
const FILE_OID: &str = "0a1b2c3d";
const FILE_SIZE: usize = 300_000;
const SHORT_FILE_OID: &str = "0e0f";
const TEST_DIR_OID: &str = "k5555";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn file_bytes() -> Vec<u8> {
    (0..FILE_SIZE).map(|i| (i % 251) as u8).collect()
}

/// A directory with a file whose name is invalid on Windows, a symlink and a truncated file.
/// Small tree for path-following tests: /a/b/c.txt, /a/x.txt, /f (a file).
fn tree_dir(oid: &str) -> Option<String> {
    let entries = match oid {
        "k7001" => r#"{"name":"a","type":"d","obj":"k7002"},{"name":"f","type":"f","size":1,"obj":"0f01"}"#,
        "k7002" => r#"{"name":"b","type":"d","obj":"k7003"},{"name":"x.txt","type":"f","size":1,"obj":"0f02"}"#,
        "k7003" => r#"{"name":"c.txt","type":"f","size":1,"obj":"0f03"}"#,
        _ => return None,
    };
    Some(format!(r#"{{"stream":"kopia:directory","entries":[{entries}]}}"#))
}

fn test_dir_json() -> String {
    format!(
        r#"{{"stream":"kopia:directory","entries":[
            {{"name":"sub","type":"d","mode":"0755","mtime":"2026-01-01T00:00:00Z","obj":"k6666","summ":{{"size":1}}}},
            {{"name":"re:port?.txt","type":"f","mode":"0644","size":{FILE_SIZE},"mtime":"2025-03-04T05:06:07.123456789Z","obj":"{FILE_OID}"}},
            {{"name":"short.bin","type":"f","size":1000,"mtime":"2025-03-04T05:06:07Z","obj":"{SHORT_FILE_OID}"}},
            {{"name":"current","type":"s","size":3,"obj":"0abc"}}
        ]}}"#
    )
}

#[derive(Default)]
struct MockState {
    /// Changes on "restart": invalidates CSRF tokens and auth cookies.
    key: AtomicU64,
    next_session: AtomicU64,
    password_checks: AtomicU32,
    version: Mutex<String>,
    readonly: Mutex<bool>,
    requests: Mutex<Vec<String>>,
}

impl MockState {
    fn token(&self, session: &str) -> String {
        format!("{:016x}{}", self.key.load(Ordering::SeqCst), hex::encode(session))
    }
    fn auth_cookie(&self) -> String {
        format!("jwt-{}", self.key.load(Ordering::SeqCst))
    }
}

fn reply(status: StatusCode, body: impl Into<Bytes>, headers: &[(&str, String)]) -> Response<Full<Bytes>> {
    let mut b = Response::builder().status(status);
    for (k, v) in headers {
        b = b.header(*k, v);
    }
    b.body(Full::new(body.into())).unwrap()
}

fn cookies(req: &Request<hyper::body::Incoming>) -> HashMap<String, String> {
    req.headers()
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('=').map(|(a, b)| (a.to_owned(), b.to_owned())))
        .collect()
}

async fn handle(st: Arc<MockState>, req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path().to_owned();
    st.requests.lock().unwrap().push(format!("{} {}", req.method(), req.uri()));
    let jar = cookies(&req);
    let mut set_cookies: Vec<(&str, String)> = Vec::new();

    // --- authentication (server.go isAuthenticated) ---
    let basic = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|v| base64::engine::general_purpose::STANDARD.decode(v).ok())
        .and_then(|v| String::from_utf8(v).ok());
    let Some((user, pass)) = basic.as_deref().and_then(|b| b.split_once(':')) else {
        return Ok(reply(StatusCode::UNAUTHORIZED, "Missing credentials.\n", &[]));
    };
    let cookie_ok = jar.get("Kopia-Auth").is_some_and(|c| *c == st.auth_cookie()) && user == USER;
    if !cookie_ok {
        st.password_checks.fetch_add(1, Ordering::SeqCst);
        let valid = (user == USER || user == OTHER_USER) && pass == PASSWORD;
        if !valid {
            return Ok(reply(StatusCode::UNAUTHORIZED, "Access denied.\n", &[]));
        }
        set_cookies.push(("set-cookie", format!("Kopia-Auth={}; Path=/", st.auth_cookie())));
    }
    if user != USER {
        return Ok(reply(StatusCode::FORBIDDEN, "UI Access denied.\n", &set_cookies));
    }

    // --- start page ---
    if path == "/" {
        let session = match jar.get("Kopia-Session-Cookie") {
            Some(s) => s.clone(),
            None => {
                let s = format!("session-{}", st.next_session.fetch_add(1, Ordering::SeqCst));
                set_cookies.push(("set-cookie", format!("Kopia-Session-Cookie={s}; Path=/")));
                s
            }
        };
        let page = format!(
            r#"<!doctype html><html><head><title>KopiaUI {}</title><meta name="kopia-csrf-token" content="{}" /></head><body></body></html>"#,
            st.version.lock().unwrap(),
            st.token(&session)
        );
        return Ok(reply(StatusCode::OK, page, &set_cookies));
    }

    // --- objects (no CSRF) ---
    if let Some(oid) = path.strip_prefix("/api/v1/objects/") {
        let body: Vec<u8> = match oid {
            TEST_DIR_OID => test_dir_json().into_bytes(),
            o if tree_dir(o).is_some() => tree_dir(o).unwrap().into_bytes(),
            o if restore_tree(o).is_some() => restore_tree(o).unwrap().into_bytes(),
            "0a01" | "0a02" | "0a03" | "0a04" | "0a05" => HELLO.to_vec(),
            FILE_OID => file_bytes(),
            SHORT_FILE_OID => vec![0u8; 10], // entry says 1000 bytes
            o if o.starts_with('k') => fixture("dir-root.json").into_bytes(),
            _ => return Ok(reply(StatusCode::NOT_FOUND, "object not found\n", &set_cookies)),
        };
        if let Some(start) = req
            .headers()
            .get("range")
            .and_then(|v| v.to_str().ok())
            .and_then(|r| r.strip_prefix("bytes="))
            .and_then(|r| r.strip_suffix('-'))
            .and_then(|r| r.parse::<usize>().ok())
        {
            set_cookies.push(("content-range", format!("bytes {start}-{}/{}", body.len() - 1, body.len())));
            return Ok(reply(StatusCode::PARTIAL_CONTENT, body[start..].to_vec(), &set_cookies));
        }
        return Ok(reply(StatusCode::OK, body, &set_cookies));
    }

    // --- everything else needs CSRF (server_authz_checks.go validateCSRFToken) ---
    let csrf_ok = jar.get("Kopia-Session-Cookie").is_some_and(|s| {
        req.headers().get("x-kopia-csrf-token").and_then(|v| v.to_str().ok()) == Some(st.token(s).as_str())
    });
    if !csrf_ok {
        return Ok(reply(StatusCode::UNAUTHORIZED, "Invalid or missing CSRF token.\n", &set_cookies));
    }
    let query = req.uri().query().unwrap_or_default().to_owned();
    let body = match path.as_str() {
        "/api/v1/repo/status" => {
            let mut v: serde_json::Value = serde_json::from_str(&fixture("repo-status.json")).unwrap();
            v["readonly"] = serde_json::Value::Bool(*st.readonly.lock().unwrap());
            v.to_string()
        }
        "/api/v1/sources" => fixture("sources.json"),
        "/api/v1/snapshots" if query.contains("path=%2Fdata%2Fsource-3") => {
            if query.contains("all=1") {
                fixture("snapshots-all-3.json")
            } else {
                fixture("snapshots-3.json")
            }
        }
        "/api/v1/snapshots" => r#"{"snapshots":[],"unfilteredCount":0,"uniqueCount":0}"#.to_owned(),
        _ => return Ok(reply(StatusCode::NOT_FOUND, "404 page not found\n", &set_cookies)),
    };
    set_cookies.push(("content-type", "application/json".into()));
    Ok(reply(StatusCode::OK, body, &set_cookies))
}

struct Mock {
    state: Arc<MockState>,
    address: ServerAddress,
    fingerprint: Fingerprint,
}

impl Mock {
    async fn start() -> Mock {
        let cert = rcgen::generate_simple_self_signed(vec!["kopia-mock".to_owned()]).unwrap();
        let cert_der = cert.cert.der().clone();
        let fingerprint = Fingerprint::of_der(cert_der.as_ref());
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key)
            .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let state = Arc::new(MockState::default());
        *state.version.lock().unwrap() = "v0.23.1".into();
        *state.readonly.lock().unwrap() = true;
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else { break };
                let (acceptor, st) = (acceptor.clone(), st.clone());
                tokio::spawn(async move {
                    if let Ok(tls) = acceptor.accept(tcp).await {
                        let svc = hyper::service::service_fn(move |req| handle(st.clone(), req));
                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(hyper_util::rt::TokioIo::new(tls), svc)
                            .await;
                    }
                });
            }
        });
        let address = format!("https://127.0.0.1:{port}").parse().unwrap();
        Mock { state, address, fingerprint }
    }

    fn client(&self) -> KopiaClient {
        KopiaClient::new(self.address.clone(), USER, PASSWORD, self.fingerprint).unwrap()
    }

    fn requests(&self) -> Vec<String> {
        self.state.requests.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn connects_and_reads_version_and_status() {
    let mock = Mock::start().await;
    let info = mock.client().connect().await.unwrap();
    assert_eq!(info.version.as_deref(), Some("0.23.1"));
    assert!(info.repo.connected && info.repo.readonly);
    assert!(info.warnings.is_empty(), "{:?}", info.warnings);
    assert_eq!(mock.requests(), vec!["GET /", "GET /api/v1/repo/status"]);
}

#[tokio::test]
async fn warns_about_untested_version_and_writable_repository() {
    let mock = Mock::start().await;
    *mock.state.version.lock().unwrap() = "v0.99.0".into();
    *mock.state.readonly.lock().unwrap() = false;
    let info = mock.client().connect().await.unwrap();
    assert_eq!(info.warnings, vec![Warning::UntestedVersion(Some("0.99.0".into())), Warning::NotReadOnly]);
}

#[tokio::test]
async fn rejects_wrong_certificate_without_sending_anything() {
    let mock = Mock::start().await;
    let wrong: Fingerprint = "00".repeat(32).parse().unwrap();
    let client = KopiaClient::new(mock.address.clone(), USER, PASSWORD, wrong).unwrap();
    match client.connect().await {
        Err(Error::CertificateMismatch { presented }) => assert_eq!(presented, mock.fingerprint.to_hex()),
        other => panic!("expected CertificateMismatch, got {other:?}"),
    }
    assert!(mock.requests().is_empty(), "no HTTP request may reach an unverified server");
}

#[tokio::test]
async fn probe_reports_the_certificate_fingerprint() {
    let mock = Mock::start().await;
    assert_eq!(kopia_client::probe_certificate(&mock.address).await.unwrap(), mock.fingerprint);
    assert!(mock.requests().is_empty());
}

#[tokio::test]
async fn distinguishes_authentication_failures() {
    let mock = Mock::start().await;
    let bad = KopiaClient::new(mock.address.clone(), USER, "wrong", mock.fingerprint).unwrap();
    assert!(matches!(bad.connect().await, Err(Error::AccessDenied)));
    let other = KopiaClient::new(mock.address.clone(), OTHER_USER, PASSWORD, mock.fingerprint).unwrap();
    assert!(matches!(other.connect().await, Err(Error::NotUiUser)));
}

#[tokio::test]
async fn reuses_auth_cookie_instead_of_password() {
    let mock = Mock::start().await;
    let client = mock.client();
    client.connect().await.unwrap();
    for _ in 0..5 {
        client.sources().await.unwrap();
    }
    assert_eq!(mock.state.password_checks.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn recovers_from_server_restart_by_refreshing_the_csrf_token() {
    let mock = Mock::start().await;
    let client = mock.client();
    client.connect().await.unwrap();
    mock.state.key.fetch_add(1, Ordering::SeqCst); // "restart": new signing key
    let sources = client.sources().await.unwrap();
    assert_eq!(sources.sources.len(), 3);
    let reqs = mock.requests();
    assert_eq!(&reqs[2..], ["GET /api/v1/sources", "GET /", "GET /api/v1/sources"]);
}

#[tokio::test]
async fn only_whitelisted_get_requests_are_sent() {
    let mock = Mock::start().await;
    let client = mock.client();
    client.connect().await.unwrap();
    let sources = client.sources().await.unwrap();
    for s in &sources.sources {
        let snaps = client.snapshots(&s.source, true).await.unwrap();
        if let Some(last) = snaps.snapshots.last() {
            client.list_directory(&last.root_id).await.unwrap();
        }
    }
    let allowed = |r: &str| {
        r == "GET /"
            || r == "GET /api/v1/repo/status"
            || r == "GET /api/v1/sources"
            || r.starts_with("GET /api/v1/snapshots?")
            || r.starts_with("GET /api/v1/objects/")
    };
    let reqs = mock.requests();
    assert!(reqs.len() > 5);
    assert!(reqs.iter().all(|r| allowed(r)), "{reqs:?}");
}

#[tokio::test]
async fn lists_sources_snapshots_and_directories_from_recorded_responses() {
    let mock = Mock::start().await;
    let client = mock.client();
    client.connect().await.unwrap();
    let sources = client.sources().await.unwrap();
    assert_eq!(sources.sources.len(), 3);
    assert!(sources.sources.iter().all(|s| s.status == "REMOTE"));
    let src = &sources.sources.iter().find(|s| s.source.path == "/data/source-3").unwrap().source;

    let grouped = client.snapshots(src, false).await.unwrap();
    let all = client.snapshots(src, true).await.unwrap();
    assert_eq!(all.snapshots.len(), 12);
    assert_eq!(grouped.snapshots.len(), 4);
    assert_eq!((all.unfiltered_count, all.unique_count), (12, 4));

    let dir = client.list_directory(&all.snapshots.last().unwrap().root_id).await.unwrap();
    assert_eq!(dir.stream, "kopia:directory");
    assert_eq!(dir.entries.len(), 10);
    assert!(dir.entries.iter().all(|e| e.entry_type == EntryType::Directory && e.summ.is_some()));
}

#[test]
fn recorded_snapshots_are_oldest_first_and_grouping_keeps_the_oldest() {
    let all: SnapshotsResponse = serde_json::from_str(&fixture("snapshots-all-3.json")).unwrap();
    let grouped: SnapshotsResponse = serde_json::from_str(&fixture("snapshots-3.json")).unwrap();
    assert!(all.snapshots.windows(2).all(|w| w[0].start_time <= w[1].start_time));
    for g in &grouped.snapshots {
        let first = all.snapshots.iter().find(|s| s.root_id == g.root_id).unwrap();
        assert_eq!(first.start_time, g.start_time);
    }
}

#[tokio::test]
async fn finds_entries_by_path() {
    let mock = Mock::start().await;
    let client = mock.client();
    let root: Oid = TEST_DIR_OID.parse().unwrap();
    let e = client.find_entry(&root, "/re:port?.txt").await.unwrap();
    assert_eq!(e.size, FILE_SIZE as u64);
    assert!(matches!(client.find_entry(&root, "missing").await, Err(Error::NotFound(_))));
    assert!(matches!(client.find_entry(&root, "re:port?.txt/x").await, Err(Error::NotFound(_))));
}

// --- restore -------------------------------------------------------------------------------------

const HELLO: &[u8] = b"hello";

fn restore_tree(oid: &str) -> Option<String> {
    let entries = match oid {
        // /docs (2025-01-01) with tricky names, a link, a subfolder.
        "k8001" => r#"{"name":"docs","type":"d","mtime":"2025-01-01T10:00:00Z","obj":"k8002"}"#.to_owned(),
        "k8002" => r#"{"name":"a.txt","type":"f","size":5,"mtime":"2024-01-02T03:04:05Z","obj":"0a01"},
               {"name":"A.TXT","type":"f","size":5,"mtime":"2024-01-02T03:04:05Z","obj":"0a02"},
               {"name":"x:y.txt","type":"f","size":5,"obj":"0a03"},
               {"name":"x_y.txt","type":"f","size":5,"obj":"0a04"},
               {"name":"..","type":"f","size":5,"obj":"0a05"},
               {"name":"a/b","type":"f","size":5,"obj":"0a05"},
               {"name":"","type":"f","size":5,"obj":"0a05"},
               {"name":"link","type":"s","size":3,"obj":"0abc"},
               {"name":"sub","type":"d","mtime":"2024-06-01T12:00:00Z","obj":"k8003"}"#
            .to_owned(),
        "k8003" => format!(
            r#"{{"name":"deep.bin","type":"f","size":{FILE_SIZE},"mtime":"2024-05-05T05:05:05Z","obj":"{FILE_OID}"}},
               {{"name":"broken.bin","type":"f","size":1000,"obj":"{SHORT_FILE_OID}"}}"#
        ),
        _ => return None,
    };
    Some(format!(r#"{{"stream":"kopia:directory","entries":[{entries}]}}"#))
}

fn opts(naming: Naming, conflict: ConflictPolicy) -> RestoreOptions {
    RestoreOptions::new(naming, conflict)
}

fn suffix() -> Naming {
    Naming::Suffix {
        label: "version of".into(),
        date: SuffixDate::Snapshot(Local.with_ymd_and_hms(2026, 11, 25, 19, 0, 0).unwrap()),
    }
}

async fn docs_entry(client: &KopiaClient) -> kopia_client::DirEntry {
    client.find_entry(&"k8001".parse().unwrap(), "docs").await.unwrap()
}

/// Everything below `dir` (relative paths with `/`), sorted.
fn tree(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p.clone());
            }
            out.push(p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
    out.sort();
    out
}

fn mtime_of(p: &std::path::Path) -> i64 {
    chrono::DateTime::<Utc>::from(std::fs::metadata(p).unwrap().modified().unwrap()).timestamp()
}

async fn run(
    client: &KopiaClient,
    items: &[kopia_client::DirEntry],
    dest: &std::path::Path,
    o: &RestoreOptions,
) -> kopia_client::Result<Summary> {
    let cancel = AtomicBool::new(false);
    restore_items(client, items, dest, o, &cancel, &|_| {}).await
}

#[tokio::test]
async fn restores_a_folder_tree_safely() {
    let mock = Mock::start().await;
    let client = mock.client();
    let out = tempfile::tempdir().unwrap();
    let docs = docs_entry(&client).await;

    let s = run(&client, &[docs], out.path(), &opts(suffix(), ConflictPolicy::Rename)).await.unwrap();

    let top = "docs (version of 2026-11-25 19h00)";
    assert_eq!(s.top_level, vec![out.path().join(top)]);
    let files = tree(out.path());
    assert!(files.iter().all(|f| f.starts_with(top)), "everything stays inside the restored folder: {files:?}");
    assert!(!files.iter().any(|f| f.ends_with(".part")), "no temporary files: {files:?}");
    // Folder contents keep their names; collisions get a counter instead of overwriting.
    let prefix = format!("{top}/");
    let docs_files: Vec<&str> =
        files.iter().filter_map(|f| f.strip_prefix(prefix.as_str())).filter(|f| !f.contains('/')).collect();
    assert!(docs_files.contains(&"a.txt"));
    if cfg!(windows) {
        assert!(docs_files.contains(&"A (2).TXT"), "{docs_files:?}");
        assert!(docs_files.contains(&"x_y.txt") && docs_files.contains(&"x_y (2).txt"), "{docs_files:?}");
    } else {
        assert!(docs_files.contains(&"A.TXT") && docs_files.contains(&"x:y.txt"), "{docs_files:?}");
    }
    // Hostile names ("..", "a/b", "") became harmless names inside the folder.
    assert!(docs_files.iter().filter(|f| f.starts_with('_') || f.starts_with("a_b")).count() >= 3, "{docs_files:?}");
    for f in docs_files.iter().filter(|f| **f != "sub") {
        assert_eq!(std::fs::read(out.path().join(top).join(f)).unwrap(), HELLO, "{f}");
    }
    assert_eq!(std::fs::read(out.path().join(top).join("sub/deep.bin")).unwrap(), file_bytes());

    // Truncated file: reported, removed, the rest completed.
    assert!(!out.path().join(top).join("sub/broken.bin").exists());
    assert_eq!(s.errors.len(), 1, "{:?}", s.errors);
    assert_eq!(s.errors[0].0, "docs/sub/broken.bin");
    assert_eq!(s.skipped_links, vec!["docs/link".to_owned()]);
    assert!(s.renamed.len() >= 3, "{:?}", s.renamed);
    assert_eq!(s.files_restored, 8);
    assert_eq!(s.dirs_created, 2);
    assert!(!s.cancelled);

    // Dates: files and folders (folders set last, so adding files did not change them).
    assert_eq!(mtime_of(&out.path().join(top).join("a.txt")), 1704164645);
    assert_eq!(mtime_of(&out.path().join(top).join("sub")), 1717243200);
    assert_eq!(mtime_of(&out.path().join(top)), 1735725600);
}

#[tokio::test]
async fn restoring_again_never_overwrites() {
    let mock = Mock::start().await;
    let client = mock.client();
    let out = tempfile::tempdir().unwrap();
    let docs = docs_entry(&client).await;
    let o = opts(Naming::Original, ConflictPolicy::Rename);
    run(&client, std::slice::from_ref(&docs), out.path(), &o).await.unwrap();
    std::fs::write(out.path().join("docs/a.txt"), b"mine").unwrap();
    let s = run(&client, &[docs], out.path(), &o).await.unwrap();
    assert_eq!(s.top_level, vec![out.path().join("docs (2)")]);
    assert_eq!(std::fs::read(out.path().join("docs/a.txt")).unwrap(), b"mine");
}

#[tokio::test]
async fn skip_policy_merges_and_keeps_existing_files() {
    let mock = Mock::start().await;
    let client = mock.client();
    let out = tempfile::tempdir().unwrap();
    std::fs::create_dir(out.path().join("docs")).unwrap();
    std::fs::write(out.path().join("docs/a.txt"), b"mine").unwrap();
    let docs = docs_entry(&client).await;
    let s = run(&client, &[docs], out.path(), &opts(Naming::Original, ConflictPolicy::Skip)).await.unwrap();
    assert_eq!(std::fs::read(out.path().join("docs/a.txt")).unwrap(), b"mine");
    assert!(s.skipped_existing.contains(&"docs/a.txt".to_owned()), "{:?}", s.skipped_existing);
    assert!(out.path().join("docs/sub/deep.bin").exists());
}

#[tokio::test]
async fn overwrite_needs_confirmation() {
    let mock = Mock::start().await;
    let client = mock.client();
    let out = tempfile::tempdir().unwrap();
    std::fs::create_dir(out.path().join("docs")).unwrap();
    std::fs::write(out.path().join("docs/a.txt"), b"mine").unwrap();
    let docs = docs_entry(&client).await;

    assert_eq!(
        existing_targets(std::slice::from_ref(&docs), out.path(), &Naming::Original),
        vec![out.path().join("docs")]
    );
    let refused = run(
        &client,
        std::slice::from_ref(&docs),
        out.path(),
        &opts(Naming::Original, ConflictPolicy::Overwrite { confirmed: false }),
    )
    .await;
    assert!(refused.is_err());
    assert_eq!(std::fs::read(out.path().join("docs/a.txt")).unwrap(), b"mine");
    assert_eq!(tree(out.path()), vec!["docs".to_owned(), "docs/a.txt".to_owned()], "nothing written");

    run(&client, &[docs], out.path(), &opts(Naming::Original, ConflictPolicy::Overwrite { confirmed: true }))
        .await
        .unwrap();
    assert_eq!(std::fs::read(out.path().join("docs/a.txt")).unwrap(), HELLO);
}

#[tokio::test]
async fn never_replaces_something_of_another_type() {
    let mock = Mock::start().await;
    let client = mock.client();
    let out = tempfile::tempdir().unwrap();
    std::fs::create_dir(out.path().join("docs")).unwrap();
    std::fs::write(out.path().join("docs/sub"), b"a file where a folder should go").unwrap();
    let docs = docs_entry(&client).await;
    let s = run(&client, &[docs], out.path(), &opts(Naming::Original, ConflictPolicy::Overwrite { confirmed: true }))
        .await
        .unwrap();
    assert_eq!(std::fs::read(out.path().join("docs/sub")).unwrap(), b"a file where a folder should go");
    assert!(s.errors.iter().any(|(p, _)| p == "docs/sub"), "{:?}", s.errors);
}

#[tokio::test]
async fn cancelling_keeps_finished_files_and_removes_partial_ones() {
    let mock = Mock::start().await;
    let client = mock.client();
    let out = tempfile::tempdir().unwrap();
    let deep = client.find_entry(&"k8001".parse().unwrap(), "docs/sub/deep.bin").await.unwrap();
    let cancel = AtomicBool::new(false);
    let mut o = opts(Naming::Original, ConflictPolicy::Rename);
    o.file_concurrency = 1;
    // Cancel as soon as the first bytes arrive.
    let s = restore_items(&client, &[deep], out.path(), &o, &cancel, &|p| {
        if p.bytes_done > 0 {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    })
    .await
    .unwrap();
    assert!(s.cancelled);
    assert_eq!(s.files_restored, 0);
    assert!(tree(out.path()).is_empty(), "{:?}", tree(out.path()));
}

#[tokio::test]
async fn restores_a_whole_snapshot_and_single_files() {
    let mock = Mock::start().await;
    let client = mock.client();
    let out = tempfile::tempdir().unwrap();
    let root = root_entry("/data/source-1", "k8001".parse().unwrap(), Utc::now());
    let s = run(&client, &[root], out.path(), &opts(Naming::Original, ConflictPolicy::Rename)).await.unwrap();
    assert!(out.path().join("source-1/docs/sub/deep.bin").exists());
    assert_eq!(s.dirs_created, 3);

    let file = client.find_entry(&"k8001".parse().unwrap(), "docs/a.txt").await.unwrap();
    let s = run(&client, &[file.clone(), file], out.path(), &opts(suffix(), ConflictPolicy::Rename)).await.unwrap();
    let names: Vec<_> = s.top_level.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["a (version of 2026-11-25 19h00).txt", "a (version of 2026-11-25 19h00) (2).txt"]);
}

#[tokio::test]
async fn resumes_downloads_with_range() {
    let mock = Mock::start().await;
    let client = mock.client();
    let mut stream = client.open_object(&FILE_OID.parse().unwrap(), Some(1000)).await.unwrap();
    let mut got = Vec::new();
    while let Some(c) = stream.chunk().await.unwrap() {
        got.extend_from_slice(&c);
    }
    assert_eq!(got, file_bytes()[1000..]);
}

fn path(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| s.to_string()).collect()
}

#[tokio::test]
async fn resolve_path_follows_the_folder_as_far_as_it_exists() {
    let mock = Mock::start().await;
    let client = mock.client();
    let root: Oid = "k7001".parse().unwrap();

    let r = client.resolve_path(&root, &path(&["a", "b"])).await.unwrap();
    assert_eq!((r.depth, r.dir.as_str()), (2, "k7003"));
    assert_eq!(r.listing.entries[0].name, "c.txt");

    let r = client.resolve_path(&root, &path(&["a", "gone", "b"])).await.unwrap();
    assert_eq!((r.depth, r.dir.as_str()), (1, "k7002"), "missing middle folder: stop at its parent");

    let r = client.resolve_path(&root, &path(&["f", "b"])).await.unwrap();
    assert_eq!((r.depth, r.dir.as_str()), (0, "k7001"), "a file with the folder's name does not count");

    let r = client.resolve_path(&root, &[]).await.unwrap();
    assert_eq!((r.depth, r.dir.as_str()), (0, "k7001"));
    assert_eq!(r.listing.entries.len(), 2);
}

#[tokio::test]
async fn directory_listings_are_cached_by_object_id() {
    let mock = Mock::start().await;
    let client = mock.client();
    let root: Oid = "k7001".parse().unwrap();
    for _ in 0..3 {
        client.resolve_path(&root, &path(&["a", "b"])).await.unwrap();
    }
    let fetched = mock.requests().iter().filter(|r| r.starts_with("GET /api/v1/objects/")).count();
    assert_eq!(fetched, 3, "each of the three folders is fetched once");
}
