use std::fmt;

/// Result type of this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors returned by the client.
///
/// The variants distinguish the cases a user interface has to handle differently (wrong password,
/// wrong certificate, incompatible server, ...). Messages never contain credentials.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid server address: {0}")]
    InvalidServerAddress(String),

    #[error("invalid certificate fingerprint: expected 64 hexadecimal characters (SHA-256)")]
    InvalidFingerprint,

    #[error("invalid Kopia object ID: {0:?}")]
    InvalidOid(String),

    /// The server presented a certificate whose fingerprint does not match the pinned one.
    #[error("the server certificate does not match the pinned fingerprint (server presented {presented})")]
    CertificateMismatch { presented: String },

    /// Network or TLS failure other than a certificate mismatch.
    #[error("cannot reach the server: {0}")]
    Connection(String),

    /// The client sent no Basic credentials (`Missing credentials.`). Indicates a bug.
    #[error("the server reports missing credentials")]
    MissingCredentials,

    /// Wrong user name or password (`Access denied.`).
    #[error("access denied: wrong user name or password")]
    AccessDenied,

    /// The CSRF token was rejected even after a fresh session was started.
    #[error("the server rejected the session token (CSRF)")]
    CsrfRejected,

    /// Authenticated, but the user is not the server's UI user (`--server-username`).
    #[error("this user may not use the Kopia web UI API (it must match the server's --server-username)")]
    NotUiUser,

    #[error("not found: {0}")]
    NotFound(String),

    /// The server has no repository open.
    #[error("the server is not connected to a repository")]
    NotConnected,

    /// An error response with Kopia's JSON error body (or plain text).
    #[error("server error (HTTP {status}{}): {message}", code.as_deref().map(|c| format!(", {c}")).unwrap_or_default())]
    Api { status: u16, code: Option<String>, message: String },

    /// The server answered in a way this client does not understand. Most likely an untested
    /// Kopia version.
    #[error("{}", Incompatible { version, detail })]
    Incompatible { version: Option<String>, detail: String },

    /// A file system error while restoring.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    /// A restored file does not have the expected size.
    #[error("size mismatch for {path}: expected {expected} bytes, got {actual}")]
    SizeMismatch { path: String, expected: u64, actual: u64 },

    #[error("credential store: {0}")]
    Credentials(String),

    #[error("{0}")]
    Other(String),
}

struct Incompatible<'a> {
    version: &'a Option<String>,
    detail: &'a String,
}

impl fmt::Display for Incompatible<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.version {
            Some(v) => write!(
                f,
                "unexpected response from Kopia server v{v} (tested: {}): {}",
                crate::client::TESTED_VERSIONS.join(", "),
                self.detail
            ),
            None => write!(f, "unexpected response from the server (is it a Kopia server?): {}", self.detail),
        }
    }
}

impl Error {
    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io { context: context.into(), source }
    }
}
