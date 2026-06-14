//! Custom rustls certificate verifiers enforcing the [`HandshakePolicy`].
//!
//! Peers authenticate with self-signed certificates, so there is no CA chain to
//! walk: identity *is* the certificate fingerprint, and trust comes from the
//! allowlist and TOFU pinning behind the policy port. The verifiers still
//! validate the TLS 1.3 handshake signature, which proves the peer holds the
//! certificate's private key.

use crate::policy::HandshakePolicy;
use omni_protocol::Fingerprint;
use omni_security::{LocalIdentity, fingerprint_of};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    CertificateError, DigitallySignedStruct, DistinguishedName, Error as TlsError, SignatureScheme,
};
use std::fmt;
use std::sync::Arc;

/// The ALPN protocol id both sides must offer. Bumped on incompatible wire
/// changes so mismatched versions fail at the handshake, not mid-session.
pub const ALPN: &[u8] = b"omni/1";

/// The crypto backend shared by every TLS config we build.
pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// This machine's certificate chain and key in the form rustls wants.
fn identity_material(
    identity: &LocalIdentity,
) -> (
    Vec<CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
) {
    let chain = vec![CertificateDer::from(identity.certificate_der().to_vec())];
    let key = PrivatePkcs8KeyDer::from(identity.private_key_der().to_vec()).into();
    (chain, key)
}

/// Builds the rustls client config: TLS 1.3 only, our certificate for mutual
/// auth, and the TOFU server verifier.
pub fn client_config(
    identity: &LocalIdentity,
    policy: Arc<dyn HandshakePolicy>,
) -> Result<rustls::ClientConfig, TlsError> {
    let provider = provider();
    let verifier = Arc::new(PolicyServerVerifier {
        policy,
        provider: provider.clone(),
    });
    let (chain, key) = identity_material(identity);
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(chain, key)?;
    config.alpn_protocols = vec![ALPN.to_vec()];
    Ok(config)
}

/// Builds the rustls server config: TLS 1.3 only, our certificate, and
/// mandatory client certificates checked by the policy verifier (mTLS — no
/// anonymous connections).
pub fn server_config(
    identity: &LocalIdentity,
    policy: Arc<dyn HandshakePolicy>,
) -> Result<rustls::ServerConfig, TlsError> {
    let provider = provider();
    let verifier = Arc::new(PolicyClientVerifier {
        policy,
        provider: provider.clone(),
    });
    let (chain, key) = identity_material(identity);
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(verifier)
        .with_single_cert(chain, key)?;
    config.alpn_protocols = vec![ALPN.to_vec()];
    Ok(config)
}

/// The fingerprint a peer presented, as the policy port wants it.
fn presented_fingerprint(end_entity: &CertificateDer<'_>) -> Fingerprint {
    fingerprint_of(end_entity.as_ref())
}

/// The uniform error for a certificate the policy refused. Deliberately a
/// generic verification failure: the refusal reason is logged locally, not
/// leaked to the unauthenticated peer.
fn refused() -> TlsError {
    TlsError::InvalidCertificate(CertificateError::ApplicationVerificationFailure)
}

/// Verifies the *server's* certificate when this machine dials out.
struct PolicyServerVerifier {
    policy: Arc<dyn HandshakePolicy>,
    provider: Arc<CryptoProvider>,
}

impl fmt::Debug for PolicyServerVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyServerVerifier")
            .finish_non_exhaustive()
    }
}

impl ServerCertVerifier for PolicyServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let host = match server_name {
            ServerName::DnsName(name) => name.as_ref().to_string(),
            ServerName::IpAddress(ip) => std::net::IpAddr::from(*ip).to_string(),
            _ => return Err(TlsError::General("unsupported server name".into())),
        };
        let fingerprint = presented_fingerprint(end_entity);
        match self.policy.authorize_server(&host, fingerprint) {
            Ok(()) => Ok(ServerCertVerified::assertion()),
            Err(violation) => {
                tracing::warn!(%host, %fingerprint, %violation, "refused server certificate");
                Err(refused())
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        // TLS 1.2 is never negotiated: only TLS 1.3 is configured.
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Verifies the *client's* certificate when a peer dials in.
struct PolicyClientVerifier {
    policy: Arc<dyn HandshakePolicy>,
    provider: Arc<CryptoProvider>,
}

impl fmt::Debug for PolicyClientVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyClientVerifier")
            .finish_non_exhaustive()
    }
}

impl ClientCertVerifier for PolicyClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        let fingerprint = presented_fingerprint(end_entity);
        match self.policy.authorize_client(fingerprint) {
            Ok(()) => Ok(ClientCertVerified::assertion()),
            Err(violation) => {
                tracing::warn!(%fingerprint, %violation, "refused client certificate");
                Err(refused())
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
