use super::error::ImapError;
use super::types::ImapCredentials;

// ---- Connection helper ----------------------------------------------------

/// Establish an authenticated IMAP session.
///
/// Returns a `Session` over a TLS stream (when `creds.tls` is true) or a
/// plain TCP stream.  Because the two stream types are different concrete
/// types we use an enum wrapper that implements the traits `async-imap`
/// requires (`tokio::io::AsyncRead + AsyncWrite + Unpin + Debug`).
pub(crate) async fn connect(
    creds: &ImapCredentials,
) -> Result<async_imap::Session<ImapStream>, ImapError> {
    let connect_future = tokio::net::TcpStream::connect((creds.host.as_str(), creds.port));
    // 10 second timeout for the initial TCP connection
    let tcp = tokio::time::timeout(std::time::Duration::from_secs(10), connect_future)
        .await
        .map_err(|_| ImapError::ConnectionFailed("connection timed out".to_string()))?
        .map_err(|e| ImapError::ConnectionFailed(e.to_string()))?;

    if creds.tls {
        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = tls
            .connect(&creds.host, tcp)
            .await
            .map_err(|e| ImapError::ConnectionFailed(e.to_string()))?;
        let client = async_imap::Client::new(ImapStream::Tls(tls_stream));
        let session = client
            .login(&creds.email, &creds.password)
            .await
            .map_err(|(e, _)| classify_login_error(e))?;
        Ok(session)
    } else {
        let client = async_imap::Client::new(ImapStream::Plain(tcp));
        let session = client
            .login(&creds.email, &creds.password)
            .await
            .map_err(|(e, _)| classify_login_error(e))?;
        Ok(session)
    }
}

/// Classify an `async_imap::error::Error` that occurred during LOGIN.
pub(crate) fn classify_login_error(err: async_imap::error::Error) -> ImapError {
    match err {
        async_imap::error::Error::No(_) => ImapError::AuthenticationFailed,
        async_imap::error::Error::Io(e) => ImapError::ConnectionFailed(e.to_string()),
        async_imap::error::Error::ConnectionLost => {
            ImapError::ConnectionFailed("connection lost".to_string())
        }
        other => ImapError::ProtocolError(other.to_string()),
    }
}

/// Map a generic `async_imap` error to our `ImapError`.
pub(crate) fn map_imap_error(err: async_imap::error::Error) -> ImapError {
    match err {
        async_imap::error::Error::No(msg) => ImapError::ProtocolError(format!("NO: {msg}")),
        async_imap::error::Error::Io(e) => ImapError::ConnectionFailed(e.to_string()),
        async_imap::error::Error::ConnectionLost => {
            ImapError::ConnectionFailed("connection lost".to_string())
        }
        other => ImapError::ProtocolError(other.to_string()),
    }
}

// ---- Stream enum ----------------------------------------------------------

/// A wrapper enum so that `Session` can be generic over a single type
/// regardless of whether TLS is used.
#[derive(Debug)]
pub(crate) enum ImapStream {
    Tls(async_native_tls::TlsStream<tokio::net::TcpStream>),
    Plain(tokio::net::TcpStream),
}

impl tokio::io::AsyncRead for ImapStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for ImapStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_flush(cx),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
        }
    }
}
