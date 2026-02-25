use async_imap::error::Error as ImapError;

/// Result of validating credentials against an IMAP server.
#[derive(Debug, PartialEq)]
pub enum AuthResult {
    /// The credentials are valid; login succeeded (and we immediately logged out).
    Success,
    /// The server rejected the credentials (wrong email or password).
    InvalidCredentials,
    /// Could not reach or communicate with the IMAP server.
    ServerUnreachable(String),
}

/// Validate an email/password pair against a real IMAP server.
///
/// Connects to the given `host:port`, optionally using TLS, attempts a LOGIN
/// command with the provided credentials, and immediately logs out on success.
///
/// This function does **not** maintain a persistent IMAP connection; it is
/// intended solely for credential validation at login time.
pub async fn validate_imap_credentials(
    host: &str,
    port: u16,
    tls: bool,
    email: &str,
    password: &str,
) -> AuthResult {
    if tls {
        validate_tls(host, port, email, password).await
    } else {
        validate_plain(host, port, email, password).await
    }
}

/// TLS path: connect via `tokio::net::TcpStream`, upgrade with
/// `async_native_tls::TlsConnector`, then wrap in `async_imap::Client`.
async fn validate_tls(host: &str, port: u16, email: &str, password: &str) -> AuthResult {
    let addr = format!("{host}:{port}");

    let tcp_stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => return AuthResult::ServerUnreachable(e.to_string()),
    };

    let tls_connector = async_native_tls::TlsConnector::new();
    let tls_stream = match tls_connector.connect(host, tcp_stream).await {
        Ok(s) => s,
        Err(e) => return AuthResult::ServerUnreachable(e.to_string()),
    };

    let mut client = async_imap::Client::new(tls_stream);

    // Read the server greeting before attempting login.
    match client.read_response().await {
        Some(Ok(_)) => {}
        Some(Err(e)) => return AuthResult::ServerUnreachable(e.to_string()),
        None => return AuthResult::ServerUnreachable("connection closed before greeting".into()),
    }

    attempt_login(client, email, password).await
}

/// Non-TLS path: connect via `tokio::net::TcpStream`, then wrap directly in
/// `async_imap::Client`.
async fn validate_plain(host: &str, port: u16, email: &str, password: &str) -> AuthResult {
    let addr = format!("{host}:{port}");

    let tcp_stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => return AuthResult::ServerUnreachable(e.to_string()),
    };

    let mut client = async_imap::Client::new(tcp_stream);

    // Read the server greeting before attempting login.
    match client.read_response().await {
        Some(Ok(_)) => {}
        Some(Err(e)) => return AuthResult::ServerUnreachable(e.to_string()),
        None => return AuthResult::ServerUnreachable("connection closed before greeting".into()),
    }

    attempt_login(client, email, password).await
}

/// Shared login logic: attempt LOGIN, logout on success, classify errors.
async fn attempt_login<T>(client: async_imap::Client<T>, email: &str, password: &str) -> AuthResult
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    match client.login(email, password).await {
        Ok(mut session) => {
            // Successfully authenticated; clean up.
            let _ = session.logout().await;
            AuthResult::Success
        }
        Err((err, _client)) => classify_login_error(err),
    }
}

/// Classify an IMAP error into `InvalidCredentials` or `ServerUnreachable`.
///
/// A `No` response from the server means the credentials were rejected.
/// Everything else (I/O errors, BAD responses, parse errors, etc.) is
/// treated as a server/network problem.
fn classify_login_error(err: ImapError) -> AuthResult {
    match err {
        ImapError::No(_) => AuthResult::InvalidCredentials,
        other => AuthResult::ServerUnreachable(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Connecting to an unreachable server should return `ServerUnreachable`.
    ///
    /// We use 127.0.0.1:19999, a port that (with very high probability) has
    /// nothing listening on localhost.
    #[tokio::test]
    async fn unreachable_server_returns_server_unreachable() {
        let result =
            validate_imap_credentials("127.0.0.1", 19999, false, "user@test.com", "pass").await;

        assert!(
            matches!(result, AuthResult::ServerUnreachable(_)),
            "expected ServerUnreachable, got {result:?}",
        );
    }

    /// Same test but with TLS enabled: should also return `ServerUnreachable`.
    #[tokio::test]
    async fn unreachable_server_tls_returns_server_unreachable() {
        let result =
            validate_imap_credentials("127.0.0.1", 19999, true, "user@test.com", "pass").await;

        assert!(
            matches!(result, AuthResult::ServerUnreachable(_)),
            "expected ServerUnreachable, got {result:?}",
        );
    }

    /// Verify the error classifier maps `No` to `InvalidCredentials`.
    #[test]
    fn classify_no_as_invalid_credentials() {
        let err = ImapError::No("Authentication failed".into());
        assert_eq!(classify_login_error(err), AuthResult::InvalidCredentials);
    }

    /// Verify the error classifier maps I/O errors to `ServerUnreachable`.
    #[test]
    fn classify_io_as_server_unreachable() {
        let err = ImapError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "refused",
        ));
        let result = classify_login_error(err);
        assert!(
            matches!(result, AuthResult::ServerUnreachable(_)),
            "expected ServerUnreachable, got {result:?}",
        );
    }

    /// Verify the error classifier maps `Bad` responses to `ServerUnreachable`.
    #[test]
    fn classify_bad_as_server_unreachable() {
        let err = ImapError::Bad("server error".into());
        let result = classify_login_error(err);
        assert!(
            matches!(result, AuthResult::ServerUnreachable(_)),
            "expected ServerUnreachable, got {result:?}",
        );
    }
}
