pub mod cli;
pub mod config;
pub mod domain_filter;
pub mod gui;
pub mod installer;
pub mod installer_utils;
pub mod local_browser;
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

// The same messages as `ProxyRequest` / `ProxyResponse` on the wire, but borrowing from what the caller already holds
// (the header map, the method, the path): they are only written or only read, so nothing needs to own the strings.

/// The header map as the JSON object of the messages (value text only; a header that is not text is left out).
pub struct HeadersRef<'a>(pub &'a http::HeaderMap);

impl Serialize for HeadersRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        for (key, value) in self.0.iter() {
            if let Ok(text) = value.to_str() {
                map.serialize_entry(key.as_str(), text)?;
            }
        }
        map.end()
    }
}

/// [`ProxyRequest`], written from borrowed parts.
#[derive(Serialize)]
pub struct ProxyRequestRef<'a> {
    pub port: u16,
    pub method: &'a str,
    pub path_and_query: &'a str,
    pub headers: HeadersRef<'a>,
    pub body: &'a str,
}

/// [`ProxyResponse`], written from borrowed parts.
#[derive(Serialize)]
pub struct ProxyResponseRef<'a> {
    pub status: u16,
    pub headers: HeadersRef<'a>,
    pub body: &'a str,
}

/// [`ProxyResponse`], read in place: the strings are borrowed from the received bytes (only an escaped one is copied).
#[derive(Deserialize)]
pub struct ProxyResponseIn<'a> {
    pub status: u16,
    #[serde(borrow)]
    pub headers: HashMap<std::borrow::Cow<'a, str>, std::borrow::Cow<'a, str>>,
    #[serde(borrow, default)]
    pub body: std::borrow::Cow<'a, str>, // Base64 encoded body; absent or "" means no body
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
