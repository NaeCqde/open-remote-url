#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod daemon;
mod installer;

use std::env;
use std::error::Error;
use std::process::exit;

fn print_status() {
    let (is_installed, is_running, _) = installer::check_status();

    let ext = if cfg!(target_os = "windows") {
        "bat"
    } else if cfg!(target_os = "macos") {
        "command"
    } else {
        "sh"
    };

    let status_msg = format!(
        "Open Remote URL - Client Status\n\n\
        [Status]\n\
        - Installed: {}\n\
        - Running: {}\n\n\
        [Usage]\n\
        - To install / start client:\n  Run install.{} in the release folder\n\n\
        - To uninstall / clean registrations:\n  Run uninstall.{} in the release folder",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        ext,
        ext
    );

    println!("{}", status_msg);
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.contains("://")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let _ = env::set_current_dir(parent);
        }
    }

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let _ = dotenvy::dotenv();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_status();
        exit(0);
    }

    let cmd_or_url = &args[1];

    if cmd_or_url == "--install" {
        log::info!("Installing open-remote-url...");
        match installer::install() {
            Ok(_) => {
                println!("Client installation completed successfully!");
            }
            Err(e) => {
                println!("Installation failed: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--uninstall" {
        log::info!("Uninstalling open-remote-url...");
        match installer::uninstall() {
            Ok(_) => {
                println!("Client uninstallation completed successfully!");
            }
            Err(e) => {
                println!("Uninstallation failed: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--daemon" {
        log::info!("Starting open-remote-url client daemon...");
        daemon::run().await?;
    } else if looks_like_url(cmd_or_url) {
        let config = shared::config::ClientConfig::load();
        let relay_port = config.relay_port;
        let relay_host = config.relay_host;

        let daemon_url = format!("http://{}:{}/open", relay_host, relay_port);

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
                println!("=== Open Remote URL Error ===\nFailed to connect to local daemon: {}\nMake sure the daemon is running ('open-remote-url --daemon')", e);
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
