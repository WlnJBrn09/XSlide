//! Pure host lifecycle helpers — path/env/health wiring without GUI I/O.
//! Unit-tested so resolution cannot drift from the real native host path.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Product identity for XSlide native host.
pub const PRODUCT_NAME: &str = "XSlide";
pub const BACKEND_STEM: &str = "xslide";
pub const DEFAULT_PORT: u16 = 8789;
pub const DATA_DIR_NAME: &str = "xslide-data";
pub const WINDOW_TITLE: &str = "XSlide";
pub const APP_USER_MODEL_ID: &str = "com.xslide.app";

pub fn backend_exe_name() -> String {
    if cfg!(windows) {
        format!("{BACKEND_STEM}.exe")
    } else {
        BACKEND_STEM.to_string()
    }
}

/// Resolve the backend binary given the application root (directory that owns
/// `static/` and `target/`, or the install root with `backend/` + `static/`).
/// Prefers release over debug; when `packaged` is true, looks under `backend/`.
pub fn resolve_backend_binary(app_root: &Path, packaged: bool) -> PathBuf {
    let name = backend_exe_name();
    if packaged {
        return app_root.join("backend").join(&name);
    }
    let release = app_root.join("target").join("release").join(&name);
    let debug = app_root.join("target").join("debug").join(&name);
    if release.is_file() {
        release
    } else {
        debug
    }
}

pub fn resolve_static_dir(app_root: &Path, packaged: bool) -> PathBuf {
    let _ = packaged;
    app_root.join("static")
}

/// Data directory under this product’s platform user-data base.
pub fn resolve_data_dir(user_data_base: &Path) -> PathBuf {
    user_data_base.join(DATA_DIR_NAME)
}

/// Default user-data base for this product (local app data / home fallback).
pub fn default_user_data_base() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xslide")
}

/// Build the environment map the backend expects (same contract as Electron).
pub fn build_backend_env(port: u16, static_dir: &Path, data_dir: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("PORT".into(), port.to_string());
    env.insert(
        "XSLIDE_STATIC_DIR".into(),
        static_dir.to_string_lossy().into_owned(),
    );
    env.insert(
        "XSLIDE_DATA_DIR".into(),
        data_dir.to_string_lossy().into_owned(),
    );
    if std::env::var_os("RUST_LOG").is_none() {
        env.insert("RUST_LOG".into(), "info".into());
    }
    env
}

pub fn health_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/api/health")
}

pub fn ui_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/")
}

/// Probe HTTP GET /api/health once. Returns Ok(status) if a response was read.
pub fn probe_health(port: u16) -> Result<u16, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let req =
        format!("GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let _ = stream.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| format!("invalid HTTP response: {text}"))?;
    Ok(status)
}

/// Poll health until HTTP 200 or attempts exhausted.
pub fn wait_for_health(port: u16, attempts: u32, interval_ms: u64) -> Result<(), String> {
    for n in 1..=attempts {
        match probe_health(port) {
            Ok(200) => return Ok(()),
            Ok(code) if n >= attempts => {
                return Err(format!("Backend health check failed (HTTP {code})"));
            }
            Err(e) if n >= attempts => {
                return Err(format!("Backend did not start in time: {e}"));
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(interval_ms));
    }
    Err("Backend did not start in time".into())
}

/// Discover app root: packaged layout (exe beside `backend/`/`static/`) or
/// walk up from the native-host crate / binary to the product root.
pub fn resolve_app_root(exe_dir: &Path, packaged_hint: Option<bool>) -> (PathBuf, bool) {
    if packaged_hint == Some(true) {
        return (exe_dir.to_path_buf(), true);
    }
    if packaged_hint == Some(false) {
        // Dev: native-host lives under product/native-host; product root is parent.
        // Binary often at product/native-host/target/release — walk for static/index.html.
        if let Some(root) = find_app_root_with_static(exe_dir) {
            return (root, false);
        }
        return (exe_dir.to_path_buf(), false);
    }
    // Auto: packaged if backend/ next to exe, else walk for static.
    let backend = exe_dir.join("backend").join(backend_exe_name());
    if backend.is_file() && exe_dir.join("static").join("index.html").is_file() {
        return (exe_dir.to_path_buf(), true);
    }
    if let Some(root) = find_app_root_with_static(exe_dir) {
        return (root, false);
    }
    (exe_dir.to_path_buf(), false)
}

fn find_app_root_with_static(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    for _ in 0..8 {
        let p = cur?;
        if p.join("static").join("index.html").is_file() {
            return Some(p.to_path_buf());
        }
        // Also accept product root when running from native-host/target/...
        if p.file_name().is_some_and(|n| n == "native-host")
            && p.parent()
                .map(|parent| parent.join("static").join("index.html").is_file())
                .unwrap_or(false)
        {
            return Some(p.parent().unwrap().to_path_buf());
        }
        cur = p.parent();
    }
    None
}

/// Effective listen port: PORT env or product default.
pub fn effective_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn backend_exe_name_is_windows_exe() {
        let name = backend_exe_name();
        assert!(name.starts_with(BACKEND_STEM));
        if cfg!(windows) {
            assert!(name.ends_with(".exe"));
        }
    }

    #[test]
    fn resolve_backend_prefers_release_when_present() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let rel = root.join("target").join("release");
        let dbg = root.join("target").join("debug");
        fs::create_dir_all(&rel).unwrap();
        fs::create_dir_all(&dbg).unwrap();
        let name = backend_exe_name();
        fs::write(rel.join(&name), b"rel").unwrap();
        fs::write(dbg.join(&name), b"dbg").unwrap();
        let got = resolve_backend_binary(root, false);
        assert_eq!(got, rel.join(&name));
    }

    #[test]
    fn resolve_backend_falls_back_to_debug() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let dbg = root.join("target").join("debug");
        fs::create_dir_all(&dbg).unwrap();
        let name = backend_exe_name();
        fs::write(dbg.join(&name), b"dbg").unwrap();
        let got = resolve_backend_binary(root, false);
        assert_eq!(got, dbg.join(&name));
    }

    #[test]
    fn resolve_backend_packaged_uses_backend_subdir() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let got = resolve_backend_binary(root, true);
        assert_eq!(got, root.join("backend").join(backend_exe_name()));
    }

    #[test]
    fn resolve_static_dir_is_under_app_root() {
        let root = PathBuf::from("C:\\apps\\wp");
        assert_eq!(resolve_static_dir(&root, false), root.join("static"));
        assert_eq!(resolve_static_dir(&root, true), root.join("static"));
    }

    #[test]
    fn resolve_data_dir_appends_product_folder() {
        let base = PathBuf::from("/tmp/user-data");
        assert_eq!(resolve_data_dir(&base), base.join(DATA_DIR_NAME));
    }

    #[test]
    fn build_backend_env_sets_contract_keys() {
        let env = build_backend_env(8789, Path::new("C:\\pp\\static"), Path::new("C:\\pp\\data"));
        assert_eq!(env.get("PORT").map(String::as_str), Some("8789"));
        assert_eq!(
            env.get("XSLIDE_STATIC_DIR").map(String::as_str),
            Some("C:\\pp\\static")
        );
        assert_eq!(
            env.get("XSLIDE_DATA_DIR").map(String::as_str),
            Some("C:\\pp\\data")
        );
    }

    #[test]
    fn health_and_ui_urls_use_port() {
        assert_eq!(health_url(8789), "http://127.0.0.1:8789/api/health");
        assert_eq!(ui_url(8789), "http://127.0.0.1:8789/");
        assert_eq!(health_url(9999), "http://127.0.0.1:9999/api/health");
    }

    #[test]
    fn resolve_app_root_packaged_when_backend_and_static_present() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backend")).unwrap();
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(root.join("backend").join(backend_exe_name()), b"x").unwrap();
        fs::write(root.join("static").join("index.html"), b"<html>").unwrap();
        let (got, packaged) = resolve_app_root(root, None);
        assert!(packaged);
        assert_eq!(got, root);
    }

    #[test]
    fn resolve_app_root_finds_static_walking_up() {
        let dir = tempdir().unwrap();
        let product = dir.path().join("xslide");
        let nested = product.join("native-host").join("target").join("release");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(product.join("static")).unwrap();
        fs::write(product.join("static").join("index.html"), b"<html>").unwrap();
        let (got, packaged) = resolve_app_root(&nested, None);
        assert!(!packaged);
        assert_eq!(got, product);
    }

    #[test]
    fn product_constants_are_pp() {
        assert_eq!(DEFAULT_PORT, 8789);
        assert_eq!(PRODUCT_NAME, "XSlide");
        assert_eq!(BACKEND_STEM, "xslide");
        assert_eq!(DATA_DIR_NAME, "xslide-data");
    }
}
