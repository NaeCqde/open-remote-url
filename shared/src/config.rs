use std::env;
use std::path::{Path, PathBuf};

pub fn get_config_dir(app_type: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            PathBuf::from(appdata)
                .join("open-remote-url")
                .join(app_type)
        } else {
            let user_profile = env::var("USERPROFILE").unwrap_or_default();
            PathBuf::from(user_profile)
                .join("AppData")
                .join("Roaming")
                .join("open-remote-url")
                .join(app_type)
        }
    }
    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_default();
        let app_name = if app_type == "client" {
            "OpenRemoteURLClient"
        } else {
            "OpenRemoteURLHost"
        };
        PathBuf::from(home)
            .join("Applications")
            .join(format!("{}.app", app_name))
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg).join("open-remote-url").join(app_type)
        } else {
            let home = env::var("HOME").unwrap_or_default();
            PathBuf::from(home)
                .join(".config")
                .join("open-remote-url")
                .join(app_type)
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from(".").join(app_type)
    }
}

pub fn get_config_path(app_type: &str) -> PathBuf {
    get_config_dir(app_type).join(".env")
}

pub fn load_env(app_type: &str) {
    let path = get_config_path(app_type);
    if path.exists() {
        let _ = dotenvy::from_path_override(&path);
    } else {
        let _ = dotenvy::dotenv_override();
    }
}

pub fn open_dir_in_file_manager(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        // If the path is (or is inside) a .app bundle, `open` would launch the app.
        // Instead, use `open -R` to reveal it in Finder, or open the parent directory.
        let path_str = path.to_string_lossy();
        if path_str.contains(".app") {
            // Find the first ancestor that is NOT inside a .app bundle
            let mut reveal_path = path.to_path_buf();
            for ancestor in path.ancestors() {
                let name = ancestor.to_string_lossy();
                if !name.contains(".app") {
                    reveal_path = ancestor.to_path_buf();
                    break;
                }
            }
            std::process::Command::new("open")
                .arg(&reveal_path)
                .spawn()?;
        } else {
            std::process::Command::new("open").arg(path).spawn()?;
        }
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok() {
            std::process::Command::new("xdg-open").arg(path).spawn()?;
        }
    }
    Ok(())
}

pub fn show_config(app_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = get_config_path(app_type);
    println!("{}", config_path.to_string_lossy());
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
        open_dir_in_file_manager(parent)?;
    }
    Ok(())
}

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
        Self {
            host,
            port,
            passphrase,
        }
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
