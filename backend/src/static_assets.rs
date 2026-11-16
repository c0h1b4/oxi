//! Resolve the published frontend's build-time placeholder once, before serving it.
//!
//! Next.js embeds basePath in HTML, JS (including its router/chunk loader), CSS,
//! and RSC payloads. Replace the explicit placeholder rather than guessing URLs.
//! The source export stays immutable so read-only images and restarts work.

use std::io;
use std::path::Path;

use crate::config::AppConfig;

pub const BASE_PATH_PLACEHOLDER: &str = "/__OXI_BASE_PATH__";

/// Fixed-prefix exports (including local/Nix builds) are served unchanged.
/// The returned guard owns the resolved export for the server's lifetime.
pub fn prepare(config: &mut AppConfig) -> io::Result<Option<tempfile::TempDir>> {
    if !config.serve_static {
        return Ok(None);
    }
    let source = Path::new(&config.static_dir);
    let index = match std::fs::read_to_string(source.join("index.html")) {
        Ok(index) => index,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !index.contains(BASE_PATH_PLACEHOLDER) {
        return Ok(None);
    }

    let prefix = match config.normalized_base_path() {
        "/" => "",
        prefix => prefix,
    };
    let resolved = tempfile::Builder::new().prefix("oxi-static-").tempdir()?;
    resolve_directory(source, resolved.path(), prefix)?;
    tracing::info!(base_path = prefix, "resolved runtime frontend base path");
    config.static_dir = resolved.path().to_string_lossy().into_owned();
    Ok(Some(resolved))
}

fn resolve_directory(source: &Path, destination: &Path, prefix: &str) -> io::Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir(&target)?;
            resolve_directory(&path, &target, prefix)?;
        } else {
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("html" | "js" | "css" | "txt" | "json" | "svg" | "map") => {
                    let text = std::fs::read_to_string(&path)?;
                    std::fs::write(target, text.replace(BASE_PATH_PLACEHOLDER, prefix))?;
                }
                _ => {
                    std::fs::copy(path, target)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_export_resolves_root_and_multiple_prefixes_without_mutating_source() {
        let source = tempfile::tempdir().unwrap();
        let template = format!("{BASE_PATH_PLACEHOLDER}/_next/chunk.js");
        for name in [
            "index.html",
            "router.js",
            "fonts.css",
            "index.txt",
            "build.json",
        ] {
            std::fs::write(source.path().join(name), &template).unwrap();
        }
        std::fs::create_dir(source.path().join("login")).unwrap();
        std::fs::write(source.path().join("login/index.html"), &template).unwrap();
        std::fs::write(source.path().join("font.woff2"), [0, 255, 128, 1]).unwrap();

        for (value, expected) in [
            (None, ""),
            (Some(""), ""),
            (Some("/"), ""),
            (Some("/mail"), "/mail"),
            (Some("/apps/mail/"), "/apps/mail"),
        ] {
            let mut config: AppConfig = figment::Figment::new().extract().unwrap();
            config.static_dir = source.path().to_str().unwrap().to_owned();
            config.base_path = value.map(str::to_string);
            let guard = prepare(&mut config).unwrap().unwrap();
            for name in [
                "index.html",
                "router.js",
                "fonts.css",
                "index.txt",
                "build.json",
                "login/index.html",
            ] {
                assert_eq!(
                    std::fs::read_to_string(Path::new(&config.static_dir).join(name)).unwrap(),
                    format!("{expected}/_next/chunk.js")
                );
            }
            assert_eq!(
                std::fs::read(guard.path().join("font.woff2")).unwrap(),
                [0, 255, 128, 1]
            );
            assert_eq!(
                std::fs::read_to_string(source.path().join("index.html")).unwrap(),
                template
            );
            let generated = guard.path().to_owned();
            drop(guard);
            assert!(!generated.exists());
        }
    }

    #[test]
    fn fixed_exports_and_api_only_servers_are_not_rewritten() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("index.html"), "fixed export").unwrap();
        let mut config: AppConfig = figment::Figment::new().extract().unwrap();
        config.static_dir = source.path().to_str().unwrap().to_owned();
        let original = config.static_dir.clone();
        assert!(prepare(&mut config).unwrap().is_none());
        assert_eq!(config.static_dir, original);
        std::fs::write(source.path().join("index.html"), BASE_PATH_PLACEHOLDER).unwrap();
        config.serve_static = false;
        assert!(prepare(&mut config).unwrap().is_none());
        assert_eq!(config.static_dir, original);
    }
}
