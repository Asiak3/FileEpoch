//! Read-only client for the web server API of [Kopia](https://kopia.io).
//!
//! The API is not documented by Kopia; see `docs/API.md` in this repository for what was verified
//! and against which version.
//!
//! Safety properties enforced by construction:
//!
//! * Only `GET` requests to a fixed whitelist of endpoints can be built ([`Endpoint`] is private,
//!   object IDs are validated by [`Oid`]). Redirects are never followed.
//! * TLS is verified by pinning the SHA-256 fingerprint of the server's end-entity certificate
//!   ([`Fingerprint`]); certificate checks are never simply disabled.
//! * Credentials are kept out of `Debug` output and logs.

mod client;
pub mod credentials;
mod endpoint;
mod error;
mod oid;
pub mod restore;
mod tls;
mod types;

pub use client::{ConnectInfo, KopiaClient, ServerAddress, TESTED_VERSIONS, Warning};
pub use error::{Error, Result};
pub use oid::Oid;
pub use tls::{Fingerprint, probe_certificate};
pub use types::{
    DirEntry, DirManifest, DirectorySummary, EntryType, RepoStatus, Snapshot, SnapshotsResponse, SourceInfo,
    SourceStatus, SourcesResponse,
};
