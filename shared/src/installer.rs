use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn copy_script_files(target_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        log::info!("Generating macOS scripts in {:?}", target_dir);
        let uninstall_content = r#"#!/bin/bash
cd "$(dirname "$0")"
echo "Uninstalling Open Remote URL..."
exe="./Contents/MacOS/open-remote-url-client"
if [ -f "$exe" ]; then
    chmod +x "$exe"
    "$exe" --uninstall
fi
exe="./Contents/MacOS/open-remote-url-host"
if [ -f "$exe" ]; then
    chmod +x "$exe"
    "$exe" --uninstall
fi
"#;

        let config_content = r#"#!/bin/bash
cd "$(dirname "$0")"
echo "Opening Open Remote URL config..."
exe="./Contents/MacOS/open-remote-url-client"
if [ -f "$exe" ]; then
    chmod +x "$exe"
    "$exe" --config
fi
exe="./Contents/MacOS/open-remote-url-host"
if [ -f "$exe" ]; then
    chmod +x "$exe"
    "$exe" --config
fi
"#;

        let uninstall_path = target_dir.join("uninstall.command");
        let config_path = target_dir.join("config.command");

        fs::write(&uninstall_path, uninstall_content)?;
        fs::write(&config_path, config_content)?;

        use std::os::unix::fs::PermissionsExt;
        for path in &[&uninstall_path, &config_path] {
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms)?;
        }
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "windows")]
        let files = &["uninstall.bat", "config.bat"];
        #[cfg(target_os = "linux")]
        let files = &["uninstall.sh", "config.sh"];

        let current_exe_dir = env::current_exe()?
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let current_dir = env::current_dir()?;

        for file_name in files {
            let mut src = current_exe_dir.join(file_name);
            if !src.exists() {
                src = current_dir.join(file_name);
            }
            if !src.exists() {
                if file_name.starts_with("uninstall") {
                    src = current_dir
                        .join("scripts")
                        .join("uninstall")
                        .join(file_name);
                } else if file_name.starts_with("config") {
                    src = current_dir.join("scripts").join("config").join(file_name);
                }
            }

            if src.exists() {
                let dest = target_dir.join(file_name);
                log::info!("Copying script {:?} to {:?}", src, dest);
                fs::copy(&src, &dest)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&dest)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&dest, perms)?;
                }
            } else {
                log::warn!(
                    "Source script file {:?} not found, skipping copy",
                    file_name
                );
            }
        }
        Ok(())
    }
}

fn remove_script_files(target_dir: &Path) {
    #[cfg(target_os = "windows")]
    let files = &["uninstall.bat", "config.bat"];
    #[cfg(target_os = "macos")]
    let files = &["uninstall.command", "config.command"];
    #[cfg(target_os = "linux")]
    let files = &["uninstall.sh", "config.sh"];

    for file_name in files {
        let path = target_dir.join(file_name);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn binary_name(app_type: &str) -> &'static str {
    if app_type == "client" {
        "open-remote-url-client"
    } else {
        "open-remote-url-host"
    }
}
#[cfg(target_os = "macos")]
fn plist_label(app_type: &str) -> String {
    format!("quest.nae.open-remote-url.{}", app_type)
}

#[cfg(target_os = "linux")]
fn service_name(app_type: &str) -> String {
    format!("open-remote-url-{}.service", app_type)
}

#[cfg(target_os = "windows")]
fn registry_run_key(app_type: &str) -> &'static str {
    if app_type == "client" {
        "OpenRemoteURLClientDaemon"
    } else {
        "OpenRemoteURLHostDaemon"
    }
}

fn move_env_file(app_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    let target_env = crate::config::get_config_path(app_type);
    let config_dir = target_env.parent().ok_or("Invalid config dir")?;
    fs::create_dir_all(config_dir)?;

    let mut source_env = env::current_dir()?.join("inactive.env");
    if !source_env.exists() {
        if let Ok(current_exe) = env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                source_env = parent.join("inactive.env");
            }
        }
    }

    if source_env.exists() {
        log::info!(
            "Moving inactive.env configuration to {:?}",
            target_env
        );
        fs::copy(&source_env, &target_env)?;
        let _ = fs::remove_file(&source_env);
    } else if !target_env.exists() {
        log::info!("Writing a default .env file");
        if app_type == "client" {
            fs::write(
                &target_env,
                "HOST_URL=http://localhost:8080\nCLIENT_HOST=0.0.0.0\nCLIENT_PORT=3000\n",
            )?;
        } else {
            fs::write(&target_env, "HOST=0.0.0.0\nPORT=8080\n")?;
        }
    }
    Ok(())
}

pub fn install(app_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _ = stop_and_unregister(app_type);
    let current_exe = env::current_exe()?;

    #[cfg(target_os = "windows")]
    {
        let local_app_data = env::var("LOCALAPPDATA")?;
        let install_dir = PathBuf::from(local_app_data)
            .join("Programs")
            .join("open-remote-url")
            .join(app_type);
        fs::create_dir_all(&install_dir)?;
        let target_exe = install_dir.join(format!("{}.exe", binary_name(app_type)));

        move_env_file(app_type)?;

        if current_exe != target_exe {
            fs::copy(&current_exe, &target_exe)?;
        }

        setup_windows_startup(app_type, &target_exe)?;
        if app_type == "client" {
            setup_windows_browser(&target_exe)?;
        }

        let _ = copy_script_files(&install_dir);

        std::process::Command::new(&target_exe)
            .arg("--daemon")
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME")?;

        move_env_file(app_type)?;

        let apps_dir = PathBuf::from(home).join("Applications");
        fs::create_dir_all(&apps_dir)?;
        let target_exe = setup_macos_app_bundle(app_type, &apps_dir, &current_exe)?;

        let app_name = if app_type == "client" {
            "OpenRemoteURLClient"
        } else {
            "OpenRemoteURLHost"
        };
        let app_path = apps_dir.join(format!("{}.app", app_name));
        register_macos_app(&app_path);
        setup_macos_launchagent(app_type, &target_exe)?;

        copy_script_files(&app_path)?;
    }

    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME")?;
        let bin_dir = PathBuf::from(home)
            .join(".local")
            .join("bin")
            .join("open-remote-url")
            .join(app_type);
        fs::create_dir_all(&bin_dir)?;

        move_env_file(app_type)?;

        let target_exe = bin_dir.join(binary_name(app_type));
        if current_exe != target_exe {
            fs::copy(&current_exe, &target_exe)?;
        }
        setup_linux_systemd(app_type, &target_exe)?;
        if app_type == "client" {
            setup_linux_browser(&target_exe)?;
        }

        let _ = copy_script_files(&bin_dir);
    }

    Ok(())
}

fn remove_dir_if_empty_or_ds_store(dir: &Path) {
    if !dir.exists() {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        let mut is_empty_or_ds = true;
        let mut ds_store_path = None;
        for entry in entries {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                if file_name == ".DS_Store" {
                    ds_store_path = Some(entry.path());
                } else {
                    is_empty_or_ds = false;
                    break;
                }
            }
        }
        if is_empty_or_ds {
            if let Some(ds_path) = ds_store_path {
                let _ = fs::remove_file(ds_path);
            }
            let _ = fs::remove_dir(dir);
        }
    }
}

pub fn uninstall(app_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _ = stop_and_unregister(app_type);

    #[cfg(target_os = "windows")]
    {
        let local_app_data = env::var("LOCALAPPDATA")?;
        let install_dir = PathBuf::from(local_app_data)
            .join("Programs")
            .join("open-remote-url")
            .join(app_type);
        let target_exe = install_dir.join(format!("{}.exe", binary_name(app_type)));

        remove_script_files(&install_dir);

        if target_exe.exists() {
            let _ = fs::remove_file(&target_exe);
        }
        remove_dir_if_empty_or_ds_store(&install_dir);
        if let Some(parent_dir) = install_dir.parent() {
            remove_dir_if_empty_or_ds_store(parent_dir);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME")?;
        let apps_dir = PathBuf::from(home).join("Applications");
        let app_name = if app_type == "client" {
            "OpenRemoteURLClient"
        } else {
            "OpenRemoteURLHost"
        };
        let app_path = apps_dir.join(format!("{}.app", app_name));

        remove_script_files(&app_path);

        if app_path.exists() {
            let _ = fs::remove_dir_all(&app_path);
        }
    }

    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME")?;
        let bin_dir = PathBuf::from(home)
            .join(".local")
            .join("bin")
            .join("open-remote-url")
            .join(app_type);
        let target_exe = bin_dir.join(binary_name(app_type));

        remove_script_files(&bin_dir);

        if target_exe.exists() {
            let _ = fs::remove_file(&target_exe);
        }
        remove_dir_if_empty_or_ds_store(&bin_dir);
        if let Some(parent_dir) = bin_dir.parent() {
            remove_dir_if_empty_or_ds_store(parent_dir);
        }
    }

    // Delete config file and config directory if empty
    let target_env = crate::config::get_config_path(app_type);
    if target_env.exists() {
        let _ = fs::remove_file(&target_env);
    }
    if let Some(config_dir) = target_env.parent() {
        remove_dir_if_empty_or_ds_store(config_dir);
        if let Some(parent_config_dir) = config_dir.parent() {
            remove_dir_if_empty_or_ds_store(parent_config_dir);
        }
    }

    Ok(())
}

pub fn check_status(app_type: &str) -> (bool, bool, PathBuf, PathBuf) {
    #[cfg(target_os = "windows")]
    let exe_path = {
        let local_app_data = env::var("LOCALAPPDATA").unwrap_or_default();
        PathBuf::from(local_app_data)
            .join("Programs")
            .join("open-remote-url")
            .join(app_type)
            .join(format!("{}.exe", binary_name(app_type)))
    };

    #[cfg(target_os = "macos")]
    let exe_path = {
        let home = env::var("HOME").unwrap_or_default();
        let apps_dir = PathBuf::from(home).join("Applications");
        let app_name = if app_type == "client" {
            "OpenRemoteURLClient"
        } else {
            "OpenRemoteURLHost"
        };
        apps_dir
            .join(format!("{}.app", app_name))
            .join("Contents")
            .join("MacOS")
            .join(binary_name(app_type))
    };

    #[cfg(target_os = "linux")]
    let exe_path = {
        let home = env::var("HOME").unwrap_or_default();
        PathBuf::from(home)
            .join(".local")
            .join("bin")
            .join("open-remote-url")
            .join(app_type)
            .join(binary_name(app_type))
    };

    #[cfg(target_os = "windows")]
    let is_installed = {
        let status = std::process::Command::new("reg")
            .args(&[
                "query",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                registry_run_key(app_type),
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
            .join(format!("{}.plist", plist_label(app_type)));
        plist_path.exists()
    };

    #[cfg(target_os = "linux")]
    let is_installed = {
        let home = env::var("HOME").unwrap_or_default();
        let service_path = PathBuf::from(home)
            .join(".config")
            .join("systemd")
            .join("user")
            .join(service_name(app_type));
        service_path.exists()
    };

    crate::config::load_env(app_type);

    let port = if app_type == "client" {
        crate::config::ClientConfig::load().client_port
    } else {
        crate::config::HostConfig::load().port
    };

    let is_running = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(200),
    )
    .is_ok();

    let config_path = crate::config::get_config_path(app_type);

    (is_installed, is_running, exe_path, config_path)
}

fn stop_and_unregister(app_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let current_pid = std::process::id();
        let _ = std::process::Command::new("taskkill")
            .args(&[
                "/F",
                "/FI",
                &format!("PID ne {}", current_pid),
                "/IM",
                &format!("{}.exe", binary_name(app_type)),
            ])
            .status();

        let _ = std::process::Command::new("reg")
            .args(&[
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                registry_run_key(app_type),
                "/f",
            ])
            .status();

        if app_type == "client" {
            let keys = vec![
                "HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient",
                "HKCU\\Software\\Classes\\OpenRemoteURLClient",
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
                    "OpenRemoteURLClient",
                    "/f",
                ])
                .status();
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME")?;
        let plist_path = PathBuf::from(home.clone())
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", plist_label(app_type)));

        let _ = std::process::Command::new("launchctl")
            .args(&["unload", &plist_path.to_string_lossy()])
            .status();

        if plist_path.exists() {
            let _ = fs::remove_file(&plist_path);
        }

        let app_name = if app_type == "client" {
            "OpenRemoteURLClient"
        } else {
            "OpenRemoteURLHost"
        };
        let app_path = PathBuf::from(home)
            .join("Applications")
            .join(format!("{}.app", app_name));
        if app_path.exists() {
            let lsregister_path = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";
            let _ = std::process::Command::new(lsregister_path)
                .args(&["-u", &app_path.to_string_lossy()])
                .status();
        }
    }

    #[cfg(target_os = "linux")]
    {
        let service = service_name(app_type);
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "stop", &service])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "disable", &service])
            .status();

        let home = env::var("HOME")?;
        let service_path = PathBuf::from(home.clone())
            .join(".config")
            .join("systemd")
            .join("user")
            .join(&service);
        if service_path.exists() {
            let _ = fs::remove_file(&service_path);
        }

        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "daemon-reload"])
            .status();

        let desktop_name = format!("open-remote-url-{}.desktop", app_type);
        let desktop_path = PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("applications")
            .join(&desktop_name);
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
fn setup_windows_startup(
    app_type: &str,
    binary_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let binary_str = format!("\"{}\" --daemon", binary_path.to_string_lossy());
    let status = std::process::Command::new("reg")
        .args(&[
            "add",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            "/v",
            registry_run_key(app_type),
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
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient", "", bin_path_escaped.clone()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities", "ApplicationName", "Open Remote URL Client".to_string()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities", "ApplicationDescription", "Redirect URLs to remote Host".to_string()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities\\URLAssociations", "http", "OpenRemoteURLClient".to_string()),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities\\URLAssociations", "https", "OpenRemoteURLClient".to_string()),
        ("HKCU\\Software\\Classes\\OpenRemoteURLClient", "", "Open Remote URL Client HTML Document".to_string()),
        ("HKCU\\Software\\Classes\\OpenRemoteURLClient\\shell\\open\\command", "", bin_path_with_arg),
        ("HKCU\\Software\\RegisteredApplications", "OpenRemoteURLClient", "Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities".to_string()),
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
    app_type: &str,
    install_dir: &Path,
    current_exe: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_name = if app_type == "client" {
        "OpenRemoteURLClient"
    } else {
        "OpenRemoteURLHost"
    };
    let app_dir = install_dir.join(format!("{}.app", app_name));
    let macos_dir = app_dir.join("Contents").join("MacOS");
    fs::create_dir_all(&macos_dir)?;

    let bin_name = binary_name(app_type);
    let target_exe = macos_dir.join(bin_name);
    if current_exe != target_exe {
        fs::copy(current_exe, &target_exe)?;
    }

    let info_plist = app_dir.join("Contents").join("Info.plist");

    let url_types_str = if app_type == "client" {
        r#"    <key>CFBundleURLTypes</key>
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
    </array>"#
    } else {
        ""
    };

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>quest.nae.open-remote-url.{}</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>{}</string>
    <key>LSUIElement</key>
    <true/>
{}
</dict>
</plist>
"#,
        app_type, app_name, bin_name, url_types_str
    );
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
fn setup_macos_launchagent(
    app_type: &str,
    binary_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let home = env::var("HOME")?;
    let plist_dir = PathBuf::from(home).join("Library").join("LaunchAgents");
    fs::create_dir_all(&plist_dir)?;
    let label = plist_label(app_type);
    let plist_path = plist_dir.join(format!("{}.plist", label));

    let work_dir = binary_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap_or(binary_path);

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
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
        label,
        binary_path.to_string_lossy(),
        work_dir.to_string_lossy()
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
        .arg(&label)
        .status();

    Ok(())
}

#[cfg(target_os = "linux")]
fn setup_linux_systemd(
    app_type: &str,
    binary_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let home = env::var("HOME")?;
    let service_dir = PathBuf::from(home)
        .join(".config")
        .join("systemd")
        .join("user");
    fs::create_dir_all(&service_dir)?;
    let service = service_name(app_type);
    let service_path = service_dir.join(&service);

    let service_content = format!(
        r#"[Unit]
Description=Open Remote URL {} Daemon
After=network.target

[Service]
ExecStart={} --daemon
WorkingDirectory={}
Restart=always

[Install]
WantedBy=default.target
"#,
        app_type,
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
        .arg(&service)
        .status();
    let _ = std::process::Command::new("systemctl")
        .arg("--user")
        .arg("restart")
        .arg(&service)
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
    let desktop_path = app_dir.join("open-remote-url-client.desktop");

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
            "open-remote-url-client.desktop",
            "x-scheme-handler/http",
        ])
        .status();

    let _ = std::process::Command::new("xdg-mime")
        .args(&[
            "default",
            "open-remote-url-client.desktop",
            "x-scheme-handler/https",
        ])
        .status();

    Ok(())
}
