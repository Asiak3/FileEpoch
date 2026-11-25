//! The complete whitelist of requests this client can make.
//!
//! This module is private: the rest of the crate can only build requests through [`Endpoint`],
//! and every endpoint is a `GET`. Adding a route means adding a variant here, which is the single
//! place to review. See `docs/API.md` §1 and §7.

use reqwest::Url;

use crate::{Oid, SourceInfo};

#[derive(Debug, Clone)]
pub(crate) enum Endpoint<'a> {
    /// `GET /` — HTML page carrying the CSRF token and the server version.
    Index,
    /// `GET /api/v1/repo/status`
    RepoStatus,
    /// `GET /api/v1/sources`
    Sources,
    /// `GET /api/v1/snapshots?host=&userName=&path=[&all=1]`
    Snapshots { source: &'a SourceInfo, all: bool },
    /// `GET /api/v1/objects/{oid}` — directory manifest or file content.
    Object(&'a Oid),
}

impl Endpoint<'_> {
    /// Whether the server requires the `X-Kopia-Csrf-Token` header for this route.
    pub(crate) fn needs_csrf(&self) -> bool {
        matches!(self, Endpoint::RepoStatus | Endpoint::Sources | Endpoint::Snapshots { .. })
    }

    /// Builds the absolute URL. `base` is the validated server origin (scheme, host, port).
    pub(crate) fn url(&self, base: &Url) -> Url {
        let mut url = base.clone();
        match self {
            Endpoint::Index => url.set_path("/"),
            Endpoint::RepoStatus => url.set_path("/api/v1/repo/status"),
            Endpoint::Sources => url.set_path("/api/v1/sources"),
            Endpoint::Snapshots { source, all } => {
                url.set_path("/api/v1/snapshots");
                let mut q = url.query_pairs_mut();
                q.append_pair("host", &source.host)
                    .append_pair("userName", &source.user_name)
                    .append_pair("path", &source.path);
                if *all {
                    q.append_pair("all", "1");
                }
            }
            // Oid is validated to contain only [IZg-z0-9a-f], so it is safe in a path segment.
            Endpoint::Object(oid) => url.set_path(&format!("/api/v1/objects/{oid}")),
        }
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://kopia.example:51515").unwrap()
    }

    #[test]
    fn builds_whitelisted_urls() {
        let src =
            SourceInfo { host: "host1".into(), user_name: "user1".into(), path: "/data/my docs & more/ä?#".into() };
        let oid: Oid = "Ik0123abcd".parse().unwrap();
        assert_eq!(Endpoint::Index.url(&base()).as_str(), "https://kopia.example:51515/");
        assert_eq!(Endpoint::RepoStatus.url(&base()).path(), "/api/v1/repo/status");
        assert_eq!(Endpoint::Sources.url(&base()).path(), "/api/v1/sources");
        let u = Endpoint::Snapshots { source: &src, all: true }.url(&base());
        assert_eq!(u.path(), "/api/v1/snapshots");
        let pairs: Vec<(String, String)> = u.query_pairs().into_owned().collect();
        assert_eq!(
            pairs,
            vec![
                ("host".into(), "host1".into()),
                ("userName".into(), "user1".into()),
                ("path".into(), "/data/my docs & more/ä?#".into()),
                ("all".into(), "1".into()),
            ]
        );
        assert!(u.fragment().is_none());
        let u = Endpoint::Snapshots { source: &src, all: false }.url(&base());
        assert!(!u.query().unwrap().contains("all="));
        assert_eq!(Endpoint::Object(&oid).url(&base()).path(), "/api/v1/objects/Ik0123abcd");
    }

    #[test]
    fn csrf_requirements_match_server() {
        let src = SourceInfo { host: String::new(), user_name: String::new(), path: String::new() };
        let oid: Oid = "k0123".parse().unwrap();
        assert!(!Endpoint::Index.needs_csrf());
        assert!(Endpoint::RepoStatus.needs_csrf());
        assert!(Endpoint::Sources.needs_csrf());
        assert!(Endpoint::Snapshots { source: &src, all: false }.needs_csrf());
        assert!(!Endpoint::Object(&oid).needs_csrf());
    }
}
