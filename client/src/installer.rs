use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Stop daemon and unregister old integrations to unlock files
    let _ = stop_and_unregister();

    let current_exe = env::current_exe()?;
    let current_dir = current_exe
        .parent()
        .ok_or("Cannot get current exe parent directory")?;

    #[cfg(target_os = "windows")]
    {
        setup_windows_startup(&current_exe)?;
        setup_windows_browser(&current_exe)?;

        // Start the daemon process in background
        std::process::Command::new(&current_exe)
            .arg("--daemon")
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        let target_exe = setup_macos_app_bundle(current_dir, &current_exe)?;
        let app_path = current_dir.join("OpenRemoteUrl.app");
        register_macos_app(&app_path);
        setup_macos_launchagent(&target_exe)?;
    }

    #[cfg(target_os = "linux")]
    {
        setup_linux_systemd(&current_exe)?;
        setup_linux_browser(&current_exe)?;
    }

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Stop daemon and clean up OS integrations
    let _ = stop_and_unregister();

    // 2. Clean up local bundle next to exe on macOS
    #[cfg(target_os = "macos")]
    {
        let current_exe = env::current_exe()?;
        if let Some(parent) = current_exe.parent() {
            let app_path = parent.join("OpenRemoteUrl.app");
            if app_path.exists() {
                let _ = fs::remove_dir_all(&app_path);
            }
        }
    }

    Ok(())
}

pub fn check_status() -> (bool, bool, PathBuf) {
    let current_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    #[cfg(target_os = "windows")]
    let is_installed = {
        let status = std::process::Command::new("reg")
            .args(&[
                "query",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "OpenRemoteUrlClientDaemon",
            ])
            .output();
        status.map(|o| o.status.success()).unwrap_or(false)
    };

    #[cfg(target_os = "macos")]
    let is_installed = {
        let home = env::var("HOME").unwrap_or_default();
        let plist_path = PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("com.open-remote-url.client.plist");
        plist_path.exists()
    };

    #[cfg(target_os = "linux")]
    let is_installed = {
        let home = env::var("HOME").unwrap_or_default();
        let service_path = PathBuf::from(home)
            .join(".config")
            .join("systemd")
            .join("user")
            .join("open-remote-url-client.service");
        service_path.exists()
    };

    let config = shared::config::ClientConfig::load();
    let relay_port = config.relay_port;

    let is_running = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], relay_port)),
        std::time::Duration::from_millis(200),
    )
    .is_ok();

    (is_installed, is_running, current_dir)
}

fn stop_and_unregister() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let current_pid = std::process::id();
        // Force kill any running open-remote-url processes (except this one)
        let _ = std::process::Command::new("taskkill")
            .args(&[
                "/F",
                "/FI",
                &format!("PID ne {}", current_pid),
                "/IM",
                "open-remote-url.exe",
            ])
            .status();

        // Delete startup run registry key
        let _ = std::process::Command::new("reg")
            .args(&[
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "OpenRemoteUrlClientDaemon",
                "/f",
            ])
            .status();

        // Delete browser registration registry keys
        let keys = vec![
            "HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteUrl",
            "HKCU\\Software\\Classes\\OpenRemoteUrlURL",
        ];
        for key in keys {
            let _ = std::process::Command::new("reg")
                .args(&["delete", key, "/f"])
                .status();
        }

        let _ = std::process::Command::new("reg")
            .args(&[
                "delete",
                "HKCU\\Software\\RegisteredApplications",
                "/v",
                "OpenRemoteUrl",
                "/f",
            ])
            .status();
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME")?;
        let plist_path = PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("com.open-remote-url.client.plist");

        // Unload launchctl
        let _ = std::process::Command::new("launchctl")
            .args(&["unload", &plist_path.to_string_lossy()])
            .status();

        if plist_path.exists() {
            let _ = fs::remove_file(&plist_path);
        }

        // Unregister app bundle from LaunchServices
        if let Ok(current_exe) = env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let app_path = parent.join("OpenRemoteUrl.app");
                if app_path.exists() {
                    let lsregister_path = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";
                    let _ = std::process::Command::new(lsregister_path)
                        .args(&["-u", &app_path.to_string_lossy()])
                        .status();
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "stop", "open-remote-url-client.service"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "disable", "open-remote-url-client.service"])
            .status();

        let home = env::var("HOME")?;
        let service_path = PathBuf::from(home)
            .join(".config")
            .join("systemd")
            .join("user")
            .join("open-remote-url-client.service");
        if service_path.exists() {
            let _ = fs::remove_file(&service_path);
        }

        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "daemon-reload"])
            .status();

        let desktop_path = PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("applications")
            .join("open-remote-url.desktop");
        if desktop_path.exists() {
            let _ = fs::remove_file(&desktop_path);
        }
        let app_dir = PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("applications");
        let _ = std::process::Command::new("update-desktop-database")
            .arg(&app_dir)
            .status();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn setup_windows_startup(binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let binary_str = format!("\"{}\" --daemon", binary_path.to_string_lossy());
    let status = std::process::Command::new("reg")
        .args(&[
            "add",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            "/v",
            "OpenRemoteUrlClientDaemon",
            "/t",
            "REG_SZ",
            "/d",
            &binary_str,
            "/f",
        ])
        .status()?;
    if !status.success() {
        return Err("Failed to register startup in Windows registry".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn setup_windows_browser(binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bin_path_escaped = format!("\"{}\"", binary_path.to_string_lossy());
    let bin_path_with_arg = format!("\"{}\" \"%1\"", binary_path.to_string_lossy());

    let keys = vec![
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteUrl", "", bin_path_escaped.clone()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteUrl\\Capabilities", "ApplicationName", "Open Remote URL".to_string()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteUrl\\Capabilities", "ApplicationDescription", "Redirect URLs to remote Host".to_string()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteUrl\\Capabilities\\URLAssociations", "http", "OpenRemoteUrlURL".to_string()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteUrl\\Capabilities\\URLAssociations", "https", "OpenRemoteUrlURL".to_string()),
        ("HKCU\\Software\\Classes\\OpenRemoteUrlURL", "", "Open Remote URL HTML Document".to_string()),
        ("HKCU\\Software\\Classes\\OpenRemoteUrlURL\\shell\\open\\command", "", bin_path_with_arg),
        ("HKCU\\Software\\RegisteredApplications", "OpenRemoteUrl", "Software\\Clients\\StartMenuInternet\\OpenRemoteUrl\\Capabilities".to_string()),
    ];

    for (key, value_name, value_data) in keys {
        let mut args = vec!["add", key, "/t", "REG_SZ", "/d", &value_data, "/f"];
        if !value_name.is_empty() {
            args.push("/v");
            args.push(value_name);
        }
        std::process::Command::new("reg").args(&args).status()?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn setup_macos_app_bundle(
    install_dir: &Path,
    current_exe: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_dir = install_dir.join("OpenRemoteUrl.app");
    let macos_dir = app_dir.join("Contents").join("MacOS");
    fs::create_dir_all(&macos_dir)?;

    let target_exe = macos_dir.join("open-remote-url");
    if current_exe != target_exe {
        fs::copy(current_exe, &target_exe)?;
    }

    let info_plist = app_dir.join("Contents").join("Info.plist");
    let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.open-remote-url.client</string>
    <key>CFBundleName</key>
    <string>OpenRemoteUrl</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>open-remote-url</string>
    <key>LSUIElement</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>Web site URL</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>http</string>
                <string>https</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
"#;
    fs::write(info_plist, plist_content)?;

    Ok(target_exe)
}

#[cfg(target_os = "macos")]
fn register_macos_app(app_path: &Path) {
    let lsregister_path = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";
    let _ = std::process::Command::new(lsregister_path)
        .arg("-f")
        .arg(app_path)
        .status();
}

#[cfg(target_os = "macos")]
fn setup_macos_launchagent(binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let home = env::var("HOME")?;
    let plist_dir = PathBuf::from(home).join("Library").join("LaunchAgents");
    fs::create_dir_all(&plist_dir)?;
    let plist_path = plist_dir.join("com.open-remote-url.client.plist");

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.open-remote-url.client</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--daemon</string>
    </array>
    <key>WorkingDirectory</key>
    <string>{}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
"#,
        binary_path.to_string_lossy(),
        binary_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
    );

    fs::write(&plist_path, plist_content)?;

    let _ = std::process::Command::new("launchctl")
        .arg("unload")
        .arg(&plist_path)
        .status();
    let _ = std::process::Command::new("launchctl")
        .arg("load")
        .arg(&plist_path)
        .status();
    let _ = std::process::Command::new("launchctl")
        .arg("start")
        .arg("com.open-remote-url.client")
        .status();

    Ok(())
}

#[cfg(target_os = "linux")]
fn setup_linux_systemd(binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let home = env::var("HOME")?;
    let service_dir = PathBuf::from(home)
        .join(".config")
        .join("systemd")
        .join("user");
    fs::create_dir_all(&service_dir)?;
    let service_path = service_dir.join("open-remote-url-client.service");

    let service_content = format!(
        r#"[Unit]
Description=Open Remote URL Client Daemon
After=network.target

[Service]
ExecStart={} --daemon
WorkingDirectory={}
Restart=always

[Install]
WantedBy=default.target
"#,
        binary_path.to_string_lossy(),
        binary_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
    );

    fs::write(&service_path, service_content)?;

    let _ = std::process::Command::new("systemctl")
        .arg("--user")
        .arg("daemon-reload")
        .status();
    let _ = std::process::Command::new("systemctl")
        .arg("--user")
        .arg("enable")
        .arg("open-remote-url-client.service")
        .status();
    let _ = std::process::Command::new("systemctl")
        .arg("--user")
        .arg("restart")
        .arg("open-remote-url-client.service")
        .status();

    Ok(())
}

#[cfg(target_os = "linux")]
fn setup_linux_browser(binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let home = env::var("HOME")?;
    let app_dir = PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("applications");
    fs::create_dir_all(&app_dir)?;
    let desktop_path = app_dir.join("open-remote-url.desktop");

    let desktop_content = format!(
        r#"[Desktop Entry]
Type=Application
Name=Open Remote URL
Exec={} %u
Icon=html
Terminal=false
NoDisplay=true
MimeType=x-scheme-handler/http;x-scheme-handler/https;
"#,
        binary_path.to_string_lossy()
    );

    fs::write(&desktop_path, desktop_content)?;

    let _ = std::process::Command::new("update-desktop-database")
        .arg(&app_dir)
        .status();

    let _ = std::process::Command::new("xdg-mime")
        .args(&[
            "default",
            "open-remote-url.desktop",
            "x-scheme-handler/http",
        ])
        .status();

    let _ = std::process::Command::new("xdg-mime")
        .args(&[
            "default",
            "open-remote-url.desktop",
            "x-scheme-handler/https",
        ])
        .status();

    Ok(())
}
