//! TLS certificate pinning.
//!
//! Kopia servers usually run with a self-signed certificate (`kopia server start
//! --tls-generate-cert`). Instead of disabling verification, the client pins the SHA-256
//! fingerprint of the server's **end-entity** certificate, the same value Kopia prints as
//! `SERVER CERT SHA256: …`.
//!
//! Differences from Kopia's own `verifyPeerCertificate` (deliberate):
//! * Kopia accepts a match on *any* certificate of the presented chain. Without chain validation
//!   that would let an attacker present their own leaf plus the (public) pinned certificate, so
//!   only the end-entity certificate is compared here.
//! * Host names are not checked (the pinned certificate fully identifies the server, and
//!   self-signed Kopia certificates rarely carry the address used to connect).
//!
//! Handshake signatures are still verified with the crypto provider, so possessing the public
//! certificate is not enough to impersonate the server.

use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};

use crate::{Error, Result};

/// SHA-256 fingerprint of a DER certificate.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Fingerprint of a DER-encoded certificate.
    pub fn of_der(der: &[u8]) -> Self {
        let d = ring::digest::digest(&ring::digest::SHA256, der);
        let mut out = [0u8; 32];
        out.copy_from_slice(d.as_ref());
        Fingerprint(out)
    }

    /// Lower-case hex without separators (Kopia's format).
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Upper-case hex with `:` separators (OpenSSL's format).
    pub fn to_colon_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
    }

    fn ct_eq(&self, other: &Fingerprint) -> bool {
        self.0.iter().zip(other.0.iter()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }
}

impl FromStr for Fingerprint {
    type Err = Error;

    /// Accepts Kopia's format (`0a1b2c…`) and OpenSSL's (`0A:1B:2C:…`), ignoring spaces.
    fn from_str(s: &str) -> Result<Self> {
        let cleaned: String = s.chars().filter(|c| !matches!(c, ':' | ' ' | '\t')).collect();
        let bytes = hex::decode(cleaned.to_ascii_lowercase()).map_err(|_| Error::InvalidFingerprint)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| Error::InvalidFingerprint)?;
        Ok(Fingerprint(arr))
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({})", self.to_hex())
    }
}

pub(crate) fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Shared slot where the verifier records the fingerprint of a rejected certificate, so the
/// client can report [`Error::CertificateMismatch`] instead of a generic TLS error.
pub(crate) type MismatchSlot = Arc<Mutex<Option<Fingerprint>>>;

#[derive(Debug)]
struct PinnedVerifier {
    expected: Fingerprint,
    provider: Arc<CryptoProvider>,
    mismatch: MismatchSlot,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let presented = Fingerprint::of_der(end_entity.as_ref());
        if presented.ct_eq(&self.expected) {
            Ok(ServerCertVerified::assertion())
        } else {
            *self.mismatch.lock().unwrap_or_else(|e| e.into_inner()) = Some(presented);
            Err(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// rustls client configuration that only trusts the pinned end-entity certificate.
pub(crate) fn pinned_config(expected: Fingerprint, mismatch: MismatchSlot) -> Result<rustls::ClientConfig> {
    let provider = provider();
    let verifier = Arc::new(PinnedVerifier { expected, provider: provider.clone(), mismatch });
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Other(format!("TLS configuration: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(config)
}

/// Verifier used only by [`probe_certificate`]: records the leaf fingerprint and aborts the
/// handshake, so no application data is ever exchanged with an unverified server.
#[derive(Debug)]
struct ProbeVerifier {
    provider: Arc<CryptoProvider>,
    seen: MismatchSlot,
}

impl ServerCertVerifier for ProbeVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(Fingerprint::of_der(end_entity.as_ref()));
        Err(rustls::Error::General("certificate probe: handshake intentionally aborted".into()))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// Connects to the server, reads the fingerprint of its end-entity certificate and closes the
/// connection during the handshake. No HTTP request is sent. Used to show the user which
/// certificate they are about to trust on first connection.
pub async fn probe_certificate(server: &crate::ServerAddress) -> Result<Fingerprint> {
    let provider = provider();
    let seen: MismatchSlot = Arc::default();
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Other(format!("TLS configuration: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(ProbeVerifier { provider, seen: seen.clone() }))
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));

    let (host, port) = (server.host(), server.port());
    let tcp =
        tokio::time::timeout(std::time::Duration::from_secs(15), tokio::net::TcpStream::connect((host.as_str(), port)))
            .await
            .map_err(|_| Error::Connection(format!("timeout connecting to {host}:{port}")))?
            .map_err(|e| Error::Connection(format!("{host}:{port}: {e}")))?;

    let name = ServerName::try_from(host.clone()).map_err(|_| Error::InvalidServerAddress(host.clone()))?;
    let _ = connector.connect(name, tcp).await; // expected to fail after the certificate is seen

    let fp = *seen.lock().unwrap_or_else(|e| e.into_inner());
    fp.ok_or_else(|| Error::Connection("the server did not present a TLS certificate".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_formats() {
        let kopia = "00112233445566778899aabbccddeeff0123456789abcdeffedcba9876543210";
        let openssl = "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:01:23:45:67:89:AB:CD:EF:FE:DC:BA:98:76:54:32:10";
        let a: Fingerprint = kopia.parse().unwrap();
        let b: Fingerprint = openssl.parse().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.to_hex(), kopia);
        assert_eq!(a.to_colon_hex(), openssl);
    }

    #[test]
    fn rejects_bad_fingerprints() {
        for bad in ["", "abc", &"0".repeat(63), &"0".repeat(66), &"g".repeat(64)] {
            assert!(bad.parse::<Fingerprint>().is_err(), "{bad:?}");
        }
    }
}
