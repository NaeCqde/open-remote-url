pub mod cli;
pub mod config;
pub mod gui;
pub mod installer;
pub mod installer_utils;
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
    pub relay_url: String,
}
