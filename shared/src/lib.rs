pub mod cli;
pub mod config;
pub mod gui;
pub mod installer;
pub mod installer_utils;
#[cfg(target_os = "macos")]
pub mod mac_apple_events;
pub mod scheme_handler;
pub mod uninstaller;
pub mod utils;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OpenUrlRequest {
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProxyRequest {
    pub port: u16,
    pub method: String,
    pub path_and_query: String,
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String, // Base64 encoded body; absent or "" means no body
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProxyResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String, // Base64 encoded body; absent or "" means no body
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PortAction {
    Add,
    Delete,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PortsRequest {
    pub ports: Vec<u16>,
    pub action: PortAction,
    pub self_url: String,
}

/// Compares two byte strings without stopping at the first difference, so the time does not tell how much matched.
pub fn secure_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Whether the `Authorization` header value is `Bearer <passphrase>`.
pub fn bearer_matches(header: Option<&str>, passphrase: &str) -> bool {
    match header {
        Some(value) => secure_eq(value.as_bytes(), format!("Bearer {passphrase}").as_bytes()),
        None => false,
    }
}

/// Whether the Receiver may hand this URL to the OS to open it.
///
/// `open::that` runs whatever the OS associates with the string (a file path, an executable, `file:` URLs, any
/// registered scheme), so a request must not be able to pick that freely. Only `http://` and `https://` URLs are
/// opened, and the schemes the user listed in `ALLOWED_SCHEMES` (the custom schemes of the Sender's `SCHEME`).
pub fn is_openable_url(url: &str, extra_schemes: &[String]) -> bool {
    if url.is_empty() || url.len() > 8192 || url.chars().any(|c| c.is_control()) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    let valid_scheme = scheme.len() >= 2
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid_scheme {
        return false;
    }
    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        return rest.starts_with("//") && rest.len() > 2;
    }
    extra_schemes.iter().any(|extra| extra.eq_ignore_ascii_case(scheme))
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn secure_eq_compares_whole_values() {
        assert!(secure_eq(b"abc", b"abc"));
        assert!(!secure_eq(b"abc", b"abd"));
        assert!(!secure_eq(b"abc", b"abcd"));
        assert!(bearer_matches(Some("Bearer s3cret"), "s3cret"));
        assert!(!bearer_matches(Some("Bearer other"), "s3cret"));
        assert!(!bearer_matches(None, "s3cret"));
    }

    #[test]
    fn only_web_urls_and_listed_schemes_are_opened() {
        let none: Vec<String> = vec![];
        assert!(is_openable_url("https://example.com/a?b=c", &none));
        assert!(is_openable_url("HTTP://example.com", &none));
        assert!(!is_openable_url("file:///etc/passwd", &none));
        assert!(!is_openable_url("C:\\Windows\\System32\\calc.exe", &none));
        assert!(!is_openable_url("/Applications/Calculator.app", &none));
        assert!(!is_openable_url("calc.exe", &none));
        assert!(!is_openable_url("javascript:alert(1)", &none));
        assert!(!is_openable_url("https://example.com/\nx", &none));
        assert!(!is_openable_url("http:", &none));
        let vcc = vec!["vcc".to_string()];
        assert!(is_openable_url("vcc://vpm/addRepo?url=x", &vcc));
        assert!(!is_openable_url("vcc://vpm/addRepo?url=x", &none));
    }
}
