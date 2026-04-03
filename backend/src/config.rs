use std::net::IpAddr;

use figment::{Figment, providers::Env};
use serde::Deserialize;

/// Application configuration loaded via figment.
///
/// Layers (lowest to highest priority):
///   1. Serde defaults
///   2. Environment variables (flat, e.g. `PORT`, `IMAP_HOST`)
#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct AppConfig {
    /// Bind address for the HTTP server.
    #[serde(default = "default_host")]
    pub host: String,

    /// Port for the HTTP server.
    #[serde(default = "default_port")]
    pub port: u16,

    /// IMAP server hostname. Required in production.
    #[serde(default)]
    pub imap_host: Option<String>,

    /// IMAP server port.
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,

    /// SMTP server hostname. Required in production.
    #[serde(default)]
    pub smtp_host: Option<String>,

    /// SMTP server port.
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,

    /// Whether TLS is enabled for mail connections.
    #[serde(default = "default_tls_enabled")]
    pub tls_enabled: bool,

    /// Directory for persistent data storage.
    #[serde(default = "default_data_dir")]
    pub data_dir: String,

    /// Session timeout in hours.
    #[serde(default = "default_session_timeout_hours")]
    pub session_timeout_hours: u64,

    /// Directory to serve static frontend files from.
    #[serde(default = "default_static_dir")]
    pub static_dir: String,

    /// Environment the application is running in (development, production)
    #[serde(default = "default_environment")]
    pub environment: String,

    /// Optional base path prefix (e.g. "/oxi") for serving behind a reverse proxy subpath.
    #[serde(default)]
    pub base_path: Option<String>,

    /// Whether to serve static frontend files. Disable for dev-mode (separate frontend dev server).
    #[serde(default = "default_serve_static")]
    pub serve_static: bool,

    /// Allowed CORS origin for dev-mode cross-port requests (e.g. "http://localhost:3000").
    #[serde(default)]
    pub cors_origin: Option<String>,

    /// Comma-separated list of trusted proxy IPs that are allowed to set X-Forwarded-For.
    #[serde(default)]
    pub trusted_proxies: Option<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3001
}

fn default_imap_port() -> u16 {
    993
}

fn default_smtp_port() -> u16 {
    587
}

fn default_tls_enabled() -> bool {
    true
}

fn default_data_dir() -> String {
    "/data".to_string()
}

fn default_session_timeout_hours() -> u64 {
    24
}

fn default_static_dir() -> String {
    "./static".to_string()
}

fn default_environment() -> String {
    "development".to_string()
}

fn default_serve_static() -> bool {
    true
}

impl AppConfig {
    /// Parse the `trusted_proxies` field into a list of `IpAddr` values.
    /// Logs warnings for unparseable entries.
    pub fn parsed_trusted_proxies(&self) -> Vec<IpAddr> {
        let Some(ref proxies) = self.trusted_proxies else {
            return vec![];
        };
        proxies
            .split(',')
            .filter_map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return None;
                }
                match trimmed.parse::<IpAddr>() {
                    Ok(ip) => Some(ip),
                    Err(e) => {
                        tracing::warn!(entry = trimmed, error = %e, "ignoring unparseable trusted proxy");
                        None
                    }
                }
            })
            .collect()
    }

    /// Load configuration by layering serde defaults with environment variables.
    ///
    /// Environment variables are read without a prefix and mapped directly to
    /// struct fields via case-insensitive matching (e.g. `IMAP_HOST` → `imap_host`).
    #[allow(clippy::result_large_err)]
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::raw())
            .extract()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values_load_correctly() {
        // With no env vars set, all defaults should apply.
        // We use figment with no providers (only serde defaults).
        let config: AppConfig = Figment::new().extract().expect("defaults should load");

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3001);
        assert!(config.imap_host.is_none());
        assert_eq!(config.imap_port, 993);
        assert!(config.smtp_host.is_none());
        assert_eq!(config.smtp_port, 587);
        assert!(config.tls_enabled);
        assert_eq!(config.data_dir, "/data");
        assert_eq!(config.session_timeout_hours, 24);
        assert_eq!(config.static_dir, "./static");
        assert_eq!(config.environment, "development");
        assert!(config.serve_static);
        assert!(config.cors_origin.is_none());
        assert!(config.trusted_proxies.is_none());
    }

    #[test]
    fn env_var_overrides_work() {
        // Simulate env overrides via figment's tuple provider
        // to verify all fields are overridable.
        let config: AppConfig = Figment::new()
            .merge(("host", "127.0.0.1"))
            .merge(("port", 8080u16))
            .merge(("imap_host", "mail.example.com"))
            .merge(("imap_port", 143u16))
            .merge(("smtp_host", "smtp.example.com"))
            .merge(("smtp_port", 465u16))
            .merge(("tls_enabled", false))
            .merge(("data_dir", "/var/oxi"))
            .merge(("session_timeout_hours", 48u64))
            .merge(("static_dir", "/srv/static"))
            .merge(("environment", "production"))
            .merge(("serve_static", false))
            .merge(("cors_origin", "http://localhost:3000"))
            .merge(("trusted_proxies", "127.0.0.1,::1"))
            .extract()
            .expect("overrides should load");

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.imap_host.as_deref(), Some("mail.example.com"));
        assert_eq!(config.imap_port, 143);
        assert_eq!(config.smtp_host.as_deref(), Some("smtp.example.com"));
        assert_eq!(config.smtp_port, 465);
        assert!(!config.tls_enabled);
        assert_eq!(config.data_dir, "/var/oxi");
        assert_eq!(config.session_timeout_hours, 48);
        assert_eq!(config.static_dir, "/srv/static");
        assert_eq!(config.environment, "production");
        assert!(!config.serve_static);
        assert_eq!(config.cors_origin.as_deref(), Some("http://localhost:3000"));
        assert_eq!(config.trusted_proxies.as_deref(), Some("127.0.0.1,::1"));
    }

    #[test]
    fn real_env_vars_override_multi_word_fields() {
        // Verify that actual environment variables with underscores
        // (e.g. IMAP_HOST) correctly map to struct fields (imap_host).
        // SAFETY: This test runs single-threaded (--test-threads=1)
        // and cleans up env vars after use.
        unsafe {
            std::env::set_var("IMAP_HOST", "test-imap.example.com");
            std::env::set_var("DATA_DIR", "/tmp/oxi-test");
        }

        let config = AppConfig::load().expect("load with env vars should succeed");

        assert_eq!(config.imap_host.as_deref(), Some("test-imap.example.com"));
        assert_eq!(config.data_dir, "/tmp/oxi-test");

        // Clean up
        unsafe {
            std::env::remove_var("IMAP_HOST");
            std::env::remove_var("DATA_DIR");
        }
    }
}
