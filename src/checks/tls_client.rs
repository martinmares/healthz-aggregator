use anyhow::{Result, anyhow};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as RustlsError, RootCertStore, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use std::sync::Arc;

/// A server cert verifier that accepts anything.
///
/// Used only when `tls_verify: false` is configured.
#[derive(Debug, Default)]
struct AcceptAnyVerifier;

impl ServerCertVerifier for AcceptAnyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // Broad list (good enough for “danger mode”).
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

pub fn client_config(tls_verify: bool) -> Result<Arc<ClientConfig>> {
    // rustls 0.23 uses an explicit crypto provider.
    let provider = rustls::crypto::ring::default_provider();

    // Root store (native certs). In minimal containers, make sure ca-certificates are present.
    let mut roots = RootCertStore::empty();
    let native_certs = rustls_native_certs::load_native_certs();

    if native_certs.errors.is_empty() {
        for e in native_certs.errors {
            tracing::warn!(error = %e, "failed to load native certs; TLS verification may fail");
        }
    } else {
        for cert in native_certs.certs {
            // Ignore individual parse failures; keep going.
            let _ = roots.add(cert);
        }
    }

    // Builder flow in rustls 0.23:
    // WantsVersions -> (select protocol versions) -> WantsVerifier -> (set verifier) -> WantsClientCert -> config
    let builder = ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .map_err(|_| anyhow!("failed to select safe default TLS protocol versions"))?;

    let cfg = if tls_verify {
        builder.with_root_certificates(roots).with_no_client_auth()
    } else {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyVerifier::default()))
            .with_no_client_auth()
    };

    Ok(Arc::new(cfg))
}
