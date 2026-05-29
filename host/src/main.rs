#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod daemon;

use std::env;
use std::error::Error;
use std::process::exit;

fn print_status() {
    let (is_installed, is_running, exe_path, config_path) =
        shared::installer::check_status("host");
    let config = shared::config::HostConfig::load();

    let mut status_msg = format!(
        "Open Remote URL - Host Status\n\n\
        [Status]\n\
        - Installed:  {}\n\
        - Running:    {}\n\
        - HOST:       http://{}:{}/",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        config.host,
        config.port,
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
        - To install / start host:\n  Double-click the executable to open GUI Control Panel\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall"
    } else if cfg!(target_os = "macos") {
        "\n\n\
        [Usage]\n\
        - To install / start host:\n  Double-click the OpenRemoteURLHost.app bundle to open GUI Control Panel\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall"
    } else {
        "\n\n\
        [Usage]\n\
        - To install / start host:\n  Double-click the executable to open GUI Control Panel (GUI Desktop)\n  Or run ./install.sh in the release folder (CLI/Headless)\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall (GUI)\n  Or run ./uninstall.sh in the release folder (CLI/Headless)"
    };

    status_msg.push_str(usage_msg);

    println!("{}", status_msg);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    shared::config::load_env("host");

    let args: Vec<String> = env::args().collect();

    #[cfg(target_os = "windows")]
    {
        if args.len() < 2 {
            shared::gui::run_gui("host");
            exit(0);
        } else {
            shared::utils::attach_console();
        }
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if args.len() < 2 && shared::utils::is_double_clicked() {
            shared::gui::run_gui("host");
            exit(0);
        }
    }

    if args.len() >= 2 && args[1] == "--config" {
        let _ = shared::config::show_config("host");
        exit(0);
    }

    if args.len() < 2 {
        print_status();
        exit(0);
    }

    let cmd_or_url = &args[1];

    if cmd_or_url == "--install" {
        log::info!("Installing open-remote-url-host...");
        match shared::installer::install("host") {
            Ok(_) => {
                println!("Host installation completed successfully!");
                let _ = shared::config::show_config("host");
            }
            Err(e) => {
                println!("Installation failed: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--uninstall" {
        log::info!("Uninstalling open-remote-url-host...");
        match shared::installer::uninstall("host") {
            Ok(_) => {
                println!("Host uninstallation completed successfully!");
            }
            Err(e) => {
                println!("Uninstallation failed: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--daemon" {
        log::info!("Starting open-remote-url-host daemon...");
        daemon::run().await?;
    } else if cmd_or_url == "--start" {
        log::info!("Starting open-remote-url-host daemon...");
        match shared::installer::start_daemon("host") {
            Ok(_) => {
                println!("Host daemon started successfully!");
            }
            Err(e) => {
                println!("Failed to start host daemon: {}", e);
                exit(1);
            }
        }
    } else if cmd_or_url == "--stop" {
        log::info!("Stopping open-remote-url-host daemon...");
        match shared::installer::stop_daemon("host") {
            Ok(_) => {
                println!("Host daemon stopped successfully!");
            }
            Err(e) => {
                println!("Failed to stop host daemon: {}", e);
                exit(1);
            }
        }
    } else {
        // Unknown argument: show help/status
        print_status();
        exit(0);
    }

    Ok(())
}
