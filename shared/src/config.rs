use std::env;

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub host: String,
    pub port: u16,
    pub passphrase: Option<String>,
}

impl HostConfig {
    pub fn load() -> Self {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .unwrap_or(8080);
        let passphrase = env::var("PASSPHRASE").ok();
        Self { host, port, passphrase }
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub host_url: Option<String>,
    pub client_host: String,
    pub client_port: u16,
    pub passphrase: Option<String>,
}

impl ClientConfig {
    pub fn load() -> Self {
        let host_url = env::var("HOST_URL").ok().map(|u| {
            if u.ends_with('/') {
                u.trim_end_matches('/').to_string()
            } else {
                u
            }
        });
        let client_host = env::var("CLIENT_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let client_port = env::var("CLIENT_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .unwrap_or(3000);
        let passphrase = env::var("PASSPHRASE").ok();
        Self {
            host_url,
            client_host,
            client_port,
            passphrase,
        }
    }
}
