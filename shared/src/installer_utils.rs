use std::env;
use std::fs;
use std::path::PathBuf;

pub(crate) fn binary_name(app_type: &str) -> &'static str {
    if app_type == "client" {
        "open-remote-url-client"
    } else {
        "open-remote-url-host"
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn plist_label(app_type: &str) -> String {
    format!("quest.nae.open-remote-url.{}", app_type)
}

#[cfg(target_os = "linux")]
pub(crate) fn service_name(app_type: &str) -> String {
    format!("open-remote-url-{}.service", app_type)
}

#[cfg(target_os = "windows")]
pub(crate) fn registry_run_key(app_type: &str) -> &'static str {
    if app_type == "client" {
        "OpenRemoteURLClientDaemon"
    } else {
        "OpenRemoteURLHostDaemon"
    }
}

pub(crate) fn stop_and_unregister(app_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        let current_pid = std::process::id();
        let _ = crate::utils::create_no_window(std::process::Command::new("taskkill"))
            .args(&[
                "/F",
                "/FI",
                &format!("PID ne {}", current_pid),
                "/IM",
                &format!("{}.exe", binary_name(app_type)),
            ])
            .status();

        let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
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
                let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
                    .args(&["delete", key, "/f"])
                    .status();
            }

            let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
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
