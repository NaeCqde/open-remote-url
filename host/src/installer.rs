use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Stop daemon and unregister old integrations to unlock files
    let _ = stop_and_unregister();

    let current_exe = env::current_exe()?;

    #[cfg(target_os = "windows")]
    {
        setup_windows_startup(&current_exe)?;

        // Start in background
        std::process::Command::new(&current_exe)
            .arg("--daemon")
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        setup_macos_launchagent(&current_exe)?;
    }

    #[cfg(target_os = "linux")]
    {
        setup_linux_systemd(&current_exe)?;
    }

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Stop daemon and clean up OS integrations
    let _ = stop_and_unregister();
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
                "OpenRemoteUrlHostDaemon",
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
            .join("com.open-remote-url.host.plist");
        plist_path.exists()
    };

    #[cfg(target_os = "linux")]
    let is_installed = {
        let home = env::var("HOME").unwrap_or_default();
        let service_path = PathBuf::from(home)
            .join(".config")
            .join("systemd")
            .join("user")
            .join("open-remote-url-host.service");
        service_path.exists()
    };

    let config = shared::config::HostConfig::load();
    let port = config.port;

    let is_running = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(200),
    )
    .is_ok();

    (is_installed, is_running, current_dir)
}

fn stop_and_unregister() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let current_pid = std::process::id();
        // Force kill any running open-remote-url-host processes (except this one)
        let _ = std::process::Command::new("taskkill")
            .args(&[
                "/F",
                "/FI",
                &format!("PID ne {}", current_pid),
                "/IM",
                "open-remote-url-host.exe",
            ])
            .status();

        // Delete startup run registry key
        let _ = std::process::Command::new("reg")
            .args(&[
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "OpenRemoteUrlHostDaemon",
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
            .join("com.open-remote-url.host.plist");

        // Unload launchctl
        let _ = std::process::Command::new("launchctl")
            .args(&["unload", &plist_path.to_string_lossy()])
            .status();

        if plist_path.exists() {
            let _ = fs::remove_file(&plist_path);
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "stop", "open-remote-url-host.service"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "disable", "open-remote-url-host.service"])
            .status();

        let home = env::var("HOME")?;
        let service_path = PathBuf::from(home)
            .join(".config")
            .join("systemd")
            .join("user")
            .join("open-remote-url-host.service");
        if service_path.exists() {
            let _ = fs::remove_file(&service_path);
        }

        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "daemon-reload"])
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
            "OpenRemoteUrlHostDaemon",
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

#[cfg(target_os = "macos")]
fn setup_macos_launchagent(binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let home = env::var("HOME")?;
    let plist_dir = PathBuf::from(home).join("Library").join("LaunchAgents");
    fs::create_dir_all(&plist_dir)?;
    let plist_path = plist_dir.join("com.open-remote-url.host.plist");

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.open-remote-url.host</string>
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
        .arg("com.open-remote-url.host")
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
    let service_path = service_dir.join("open-remote-url-host.service");

    let service_content = format!(
        r#"[Unit]
Description=Open Remote URL Host Daemon
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
        .arg("open-remote-url-host.service")
        .status();
    let _ = std::process::Command::new("systemctl")
        .arg("--user")
        .arg("restart")
        .arg("open-remote-url-host.service")
        .status();

    Ok(())
}
