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
    pub relay_host: String,
    pub relay_port: u16,
    pub passphrase: Option<String>,
}

impl ClientConfig {
    pub fn load() -> Self {
        let host_url = env::var("HOST_URL").ok();
        let relay_host = env::var("RELAY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let relay_port = env::var("RELAY_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .unwrap_or(3000);
        let passphrase = env::var("PASSPHRASE").ok();
        Self {
            host_url,
            relay_host,
            relay_port,
            passphrase,
        }
    }
}
