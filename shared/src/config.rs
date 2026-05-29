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

pub fn find_inactive_env_path(app_type: &str) -> PathBuf {
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            // Check if there is an app_type-specific inactive.env under parent
            let p_app_type = parent.join(app_type).join("inactive.env");
            if p_app_type.exists() {
                return p_app_type;
            }

            let exe_dir_str = parent.to_string_lossy();
            if exe_dir_str.contains(".app/Contents/MacOS") {
                if let Some(contents) = parent.parent() {
                    if let Some(app_root) = contents.parent() {
                        if let Some(app_parent) = app_root.parent() {
                            let p_app_type = app_parent.join(app_type).join("inactive.env");
                            if p_app_type.exists() {
                                return p_app_type;
                            }
                            let p = app_parent.join("inactive.env");
                            if p.exists() {
                                return p;
                            }
                        }
                    }
                }
            }

            // Skip checking parent.join("inactive.env") if we are running from a target directory
            let is_in_target = exe_dir_str.contains("target/release")
                || exe_dir_str.contains("target/debug")
                || exe_dir_str.contains("target/x86_64")
                || exe_dir_str.contains("target/aarch64");
            if !is_in_target {
                let p = parent.join("inactive.env");
                if p.exists() {
                    return p;
                }
            }
        }
    }
    let p_app_type = env::current_dir().unwrap_or_default().join(app_type).join("inactive.env");
    if p_app_type.exists() {
        return p_app_type;
    }
    let p = env::current_dir().unwrap_or_default().join("inactive.env");
    if p.exists() {
        return p;
    }
    p
}

pub fn load_env(app_type: &str) {
    let path = get_config_path(app_type);
    if path.exists() {
        let _ = dotenvy::from_path_override(&path);
    } else {
        let inactive = find_inactive_env_path(app_type);
        if inactive.exists() {
            let _ = dotenvy::from_path_override(&inactive);
        } else {
            let _ = dotenvy::dotenv_override();
        }
    }
}

pub fn open_dir_in_file_manager(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        let path_str = path.to_string_lossy();
        if path_str.contains(".app") {
            let reveal_path = if path.extension().map_or(false, |ext| ext == "app") {
                path.join("Contents")
            } else {
                path.to_path_buf()
            };
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

/// Returns the port that the daemon listens on for the given app_type.
pub fn get_daemon_port(app_type: &str) -> u16 {
    if app_type == "client" {
        ClientConfig::load().client_port
    } else {
        HostConfig::load().port
    }
}

/// Waits up to `timeout` for the daemon to start accepting connections on the given port.
pub fn wait_for_daemon_start(port: u16, timeout: std::time::Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            std::time::Duration::from_millis(200),
        )
        .is_ok()
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
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
