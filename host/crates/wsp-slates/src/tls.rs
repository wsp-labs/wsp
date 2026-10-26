// SPDX-License-Identifier: AGPL-3.0-only
//! The one socket type the crate's two HTTP clients read and write: plain or TLS over a connected TCP stream.
use std::io;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use url::Url;

pub(crate) trait Wire: AsyncRead + AsyncWrite + Unpin {}
impl<T: AsyncRead + AsyncWrite + Unpin> Wire for T {}

pub(crate) type Socket = Box<dyn Wire>;

/// `tcp` as it is for the url's scheme: kept for `http`, a TLS session for `https` that checks the certificate against
/// the url's host, whatever address `tcp` was dialled at.
pub(crate) async fn over(url: &Url, tcp: TcpStream) -> io::Result<Socket> {
    if url.scheme() != "https" {
        return Ok(Box::new(tcp));
    }
    let name = match url.host() {
        Some(url::Host::Domain(domain)) => ServerName::try_from(domain.to_string()).ok(),
        Some(url::Host::Ipv4(ip)) => Some(ServerName::IpAddress(ip.into())),
        Some(url::Host::Ipv6(ip)) => Some(ServerName::IpAddress(ip.into())),
        None => None,
    };
    let name = name.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no host to dial"))?;
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| io::Error::other(e.to_string()))?
        .with_root_certificates(RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() })
        .with_no_client_auth();
    Ok(Box::new(TlsConnector::from(Arc::new(config)).connect(name, tcp).await?))
}
