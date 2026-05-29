#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod daemon;

use std::env;
use std::error::Error;
use std::process::exit;

fn print_status() {
    let (is_installed, is_running, exe_path, config_path) =
        shared::installer::check_status("client");
    let config = shared::config::ClientConfig::load();
    let host_url = config.host_url.unwrap_or_else(|| "".to_string());
    let host_url_display = if host_url.is_empty() {
        "(please set)".to_string()
    } else {
        format!("{}/", host_url.trim_end_matches('/'))
    };

    let mut status_msg = format!(
        "Open Remote URL - Client Status\n\n\
        [Status]\n\
        - Installed:  {}\n\
        - Running:    {}\n\
        - HOST:       {}\n\
        - CLIENT:     http://{}:{}/",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        host_url_display,
        config.client_host,
        config.client_port,
    );

    if is_installed {
        status_msg.push_str(&format!(
            "\n\
            - Executable: {}\n\
            - Config:     {}",
            exe_path.to_string_lossy(),
            config_path.to_string_lossy()
        ));
    }

    let usage_msg = if cfg!(target_os = "windows") {
        "\n\n\
        [Usage]\n\
        - To install / start client:\n  Double-click the executable to open GUI Control Panel\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall"
    } else if cfg!(target_os = "macos") {
        "\n\n\
        [Usage]\n\
        - To install / start client:\n  Double-click the OpenRemoteURLClient.app bundle to open GUI Control Panel\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall"
    } else {
        "\n\n\
        [Usage]\n\
        - To install / start client:\n  Double-click the executable to open GUI Control Panel (GUI Desktop)\n  Or run ./install.sh in the release folder (CLI/Headless)\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall (GUI)\n  Or run ./uninstall.sh in the release folder (CLI/Headless)"
    };

    status_msg.push_str(usage_msg);

    println!("{}", status_msg);
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.contains("://")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    shared::config::load_env("client");

    let args: Vec<String> = env::args().collect();

    #[cfg(target_os = "windows")]
    {
        if args.len() < 2 {
            shared::gui::run_gui("client");
            exit(0);
        } else {
            shared::utils::attach_console();
        }
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if args.len() < 2 && shared::utils::is_double_clicked() {
            shared::gui::run_gui("client");
            exit(0);
        }
    }

    if args.len() >= 2 && args[1] == "--config" {
        let _ = shared::config::show_config("client");
        exit(0);
    }

    if args.len() < 2 {
        print_status();
        exit(0);
    }

    let cmd_or_url = &args[1];

    if cmd_or_url == "--install" {
        log::info!("Installing open-remote-url-client...");
        match shared::installer::install("client") {
            Ok(_) => {
                println!("Client installation completed successfully!");
                let _ = shared::config::show_config("client");
            }
            Err(e) => {
                println!("Installation failed: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--uninstall" {
        log::info!("Uninstalling open-remote-url-client...");
        match shared::installer::uninstall("client") {
            Ok(_) => {
                println!("Client uninstallation completed successfully!");
            }
            Err(e) => {
                println!("Uninstallation failed: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--daemon" {
        log::info!("Starting open-remote-url-client daemon...");
        daemon::run().await?;
    } else if cmd_or_url == "--start" {
        log::info!("Starting open-remote-url-client daemon...");
        match shared::installer::start_daemon("client") {
            Ok(_) => {
                println!("Client daemon started successfully!");
            }
            Err(e) => {
                println!("Failed to start client daemon: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--stop" {
        log::info!("Stopping open-remote-url-client daemon...");
        match shared::installer::stop_daemon("client") {
            Ok(_) => {
                println!("Client daemon stopped successfully!");
            }
            Err(e) => {
                println!("Failed to stop client daemon: {}", e);
                exit(1);
            }
        }
    } else if looks_like_url(cmd_or_url) {
        let config = shared::config::ClientConfig::load();
        let client_port = config.client_port;
        let client_host = config.client_host;

        let daemon_url = format!("http://{}:{}/open", client_host, client_port);

        log::info!("Sending URL to local daemon: {}", cmd_or_url);

        let client = reqwest::Client::new();
        let mut req = client
            .post(&daemon_url)
            .json(&serde_json::json!({ "url": cmd_or_url }));
        if let Some(phrase) = config.passphrase {
            req = req.header("Authorization", format!("Bearer {}", phrase));
        }

        match req.send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    log::info!("URL successfully forwarded to local daemon.");
                } else {
                    println!(
                        "=== Open Remote URL Error ===\nLocal daemon returned error status: {}",
                        resp.status()
                    );
                    exit(1);
                }
            }
            Err(e) => {
                println!("=== Open Remote URL Error ===\nFailed to connect to local daemon: {}\nMake sure the daemon is running ('open-remote-url-client --daemon')", e);
                exit(1);
            }
        }
    } else {
        // Unknown argument or not a URL: show help/status
        print_status();
        exit(0);
    }

    Ok(())
}
