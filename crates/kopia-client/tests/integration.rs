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
use kopia_client::restore::{Naming, restore_file};
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

#[tokio::test]
async fn restores_a_file_without_overwriting_and_sets_mtime() {
    let mock = Mock::start().await;
    let client = mock.client();
    let root: Oid = TEST_DIR_OID.parse().unwrap();
    let entry = client.find_entry(&root, "re:port?.txt").await.unwrap();
    let out = tempfile::tempdir().unwrap();
    let naming =
        Naming::Suffix { label: "restored".into(), time: Local.with_ymd_and_hms(2026, 11, 25, 19, 0, 0).unwrap() };

    let first = restore_file(&client, &entry, out.path(), &naming).await.unwrap();
    let second = restore_file(&client, &entry, out.path(), &naming).await.unwrap();

    let expected_name = if cfg!(windows) {
        "re_port_ (restored 2026-11-25 19-00).txt"
    } else {
        "re:port? (restored 2026-11-25 19-00).txt"
    };
    assert_eq!(first.path.file_name().unwrap().to_str().unwrap(), expected_name);
    assert_eq!(first.sanitized, cfg!(windows));
    assert!(second.path.file_name().unwrap().to_str().unwrap().ends_with("19-00) (2).txt"), "{second:?}");
    assert_eq!(std::fs::read(&first.path).unwrap(), file_bytes());
    let mtime: chrono::DateTime<Utc> = std::fs::metadata(&first.path).unwrap().modified().unwrap().into();
    assert_eq!(mtime.timestamp(), entry.mtime.unwrap().timestamp());
    let leftovers: Vec<_> = std::fs::read_dir(out.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(leftovers.len(), 2, "{leftovers:?}");
}

#[tokio::test]
async fn size_mismatch_leaves_no_files_behind() {
    let mock = Mock::start().await;
    let client = mock.client();
    let entry = client.find_entry(&TEST_DIR_OID.parse().unwrap(), "short.bin").await.unwrap();
    let out = tempfile::tempdir().unwrap();
    let err = restore_file(&client, &entry, out.path(), &Naming::Original).await.unwrap_err();
    assert!(matches!(err, Error::SizeMismatch { expected: 1000, actual: 10, .. }), "{err:?}");
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn refuses_to_restore_symlinks() {
    let mock = Mock::start().await;
    let client = mock.client();
    let entry = client.find_entry(&TEST_DIR_OID.parse().unwrap(), "current").await.unwrap();
    let out = tempfile::tempdir().unwrap();
    assert!(restore_file(&client, &entry, out.path(), &Naming::Original).await.is_err());
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
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
