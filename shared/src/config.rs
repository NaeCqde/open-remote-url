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
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("open-remote-url")
            .join(app_type)
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
                        // Under macOS App Translocation the .app is remounted at a
                        // temporary read-only path.  Use SecTranslocate to resolve the
                        // original location so we can find the inactive.env that sits
                        // next to the real .app in the download folder.
                        #[cfg(target_os = "macos")]
                        let real_app_root = macos_de_translocate(app_root);
                        #[cfg(not(target_os = "macos"))]
                        let real_app_root = app_root.to_path_buf();

                        if let Some(app_parent) = real_app_root.parent() {
                            let p_app_type = app_parent.join(app_type).join("inactive.env");
                            if p_app_type.exists() {
                                return p_app_type;
                            }
                            let p = app_parent.join("inactive.env");
                            if p.exists() {
                                return p;
                            }
                            // Canonical fallback: always a meaningful path even if the
                            // file does not yet exist (avoids "/" when Finder sets cwd).
                            return p_app_type;
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

/// Resolve the real filesystem path of a macOS App-Translocated bundle.
/// If the path is not translocated, returns it unchanged.
#[cfg(target_os = "macos")]
fn macos_de_translocate(path: &Path) -> PathBuf {
    use std::ffi::CString;
    const KCF_STRING_ENCODING_UTF8: u32 = 0x08000100;
    const KCF_URL_POSIX_PATH_STYLE: i64 = 0;

    #[link(name = "CoreFoundation", kind = "framework")]
    #[link(name = "Security", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const std::ffi::c_void,
            c_str: *const std::ffi::c_char,
            encoding: u32,
        ) -> *mut std::ffi::c_void;
        fn CFURLCreateWithFileSystemPath(
            alloc: *const std::ffi::c_void,
            path: *mut std::ffi::c_void,
            style: i64,
            is_dir: bool,
        ) -> *mut std::ffi::c_void;
        fn CFURLGetFileSystemRepresentation(
            url: *const std::ffi::c_void,
            resolve: bool,
            buf: *mut u8,
            len: i64,
        ) -> bool;
        fn SecTranslocateIsTranslocatedURL(
            url: *mut std::ffi::c_void,
            is_translocated: *mut bool,
            error: *mut *mut std::ffi::c_void,
        ) -> bool;
        fn SecTranslocateCreateOriginalPathForURL(
            url: *mut std::ffi::c_void,
            error: *mut *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    let path_str = match path.to_str() {
        Some(s) => s,
        None => return path.to_path_buf(),
    };
    let c_path = match CString::new(path_str) {
        Ok(s) => s,
        Err(_) => return path.to_path_buf(),
    };

    unsafe {
        let cf_str = CFStringCreateWithCString(
            std::ptr::null(),
            c_path.as_ptr(),
            KCF_STRING_ENCODING_UTF8,
        );
        if cf_str.is_null() {
            return path.to_path_buf();
        }

        let cf_url = CFURLCreateWithFileSystemPath(
            std::ptr::null(),
            cf_str,
            KCF_URL_POSIX_PATH_STYLE,
            true,
        );
        CFRelease(cf_str);
        if cf_url.is_null() {
            return path.to_path_buf();
        }

        let mut is_translocated = false;
        let ok = SecTranslocateIsTranslocatedURL(
            cf_url,
            &mut is_translocated,
            std::ptr::null_mut(),
        );
        if !ok || !is_translocated {
            CFRelease(cf_url);
            return path.to_path_buf();
        }

        let original_url =
            SecTranslocateCreateOriginalPathForURL(cf_url, std::ptr::null_mut());
        CFRelease(cf_url);
        if original_url.is_null() {
            return path.to_path_buf();
        }

        let mut buf = vec![0u8; 4096];
        let ok = CFURLGetFileSystemRepresentation(
            original_url,
            true,
            buf.as_mut_ptr(),
            buf.len() as i64,
        );
        CFRelease(original_url);
        if !ok {
            return path.to_path_buf();
        }

        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        match std::str::from_utf8(&buf[..end]) {
            Ok(s) => PathBuf::from(s),
            Err(_) => path.to_path_buf(),
        }
    }
}

pub fn load_env(app_type: &str) {
    // 1. Current working directory .env — highest priority (CLI / dev usage).
    let cwd_env = env::current_dir().unwrap_or_default().join(".env");
    if cwd_env.exists() {
        let _ = dotenvy::from_path_override(&cwd_env);
        return;
    }

    // 2. OS-specific installed config folder.
    let installed = get_config_path(app_type);
    if installed.exists() {
        let _ = dotenvy::from_path_override(&installed);
    }
}

pub fn open_dir_in_file_manager(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        if path.extension().map_or(false, |ext| ext == "app") {
            // open -R on a file inside the bundle triggers Finder's package contents view
            let env_path = path.join(".env");
            let reveal = if env_path.exists() { env_path } else { path.to_path_buf() };
            std::process::Command::new("open").arg("-R").arg(&reveal).spawn()?;
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

fn parse_listen(listen: &str) -> (String, u16) {
    match listen.rsplit_once(':') {
        Some((host, port_str)) => (host.to_string(), port_str.parse::<u16>().unwrap_or(8080)),
        None => (listen.to_string(), 8080),
    }
}

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub listen: String,
    pub bind_host: String,
    pub port: u16,
    pub passphrase: Option<String>,
}

impl HostConfig {
    pub fn load() -> Self {
        let listen = env::var("LISTEN").unwrap_or_else(|_| "0.0.0.0:40000".to_string());
        let (bind_host, port) = parse_listen(&listen);
        let passphrase = env::var("PASSPHRASE").ok().filter(|s| !s.is_empty());
        Self { listen, bind_host, port, passphrase }
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub listen: String,
    pub client_host: String,
    pub client_port: u16,
    pub host_url: String,
    pub relay_url: String,
    pub passphrase: Option<String>,
    /// Extra URL schemes to register as OS handlers (e.g. ["vcc", "unityhub"]).
    pub schemes: Vec<String>,
    /// Whether to register http/https as URL handlers. Default true.
    pub register_http: bool,
}

impl ClientConfig {
    pub fn load() -> Self {
        let strip_slash = |u: String| u.trim_end_matches('/').to_string();
        let listen = env::var("LISTEN").unwrap_or_else(|_| "0.0.0.0:30000".to_string());
        let (client_host, client_port) = parse_listen(&listen);
        let host_url = env::var("HOST_URL").map(strip_slash)
            .unwrap_or_else(|_| "http://localhost:40000".to_string());
        let relay_url = env::var("RELAY_URL").map(strip_slash)
            .unwrap_or_else(|_| "http://localhost:30000".to_string());
        let passphrase = env::var("PASSPHRASE").ok().filter(|s| !s.is_empty());
        let schemes = env::var("SCHEME")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let register_http = env::var("HTTP")
            .map(|v| v.trim().to_lowercase() != "false")
            .unwrap_or(true);
        Self { listen, client_host, client_port, host_url, relay_url, passphrase, schemes, register_http }
    }
}
