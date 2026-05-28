mod daemon;

use std::env;
use std::process::exit;

fn print_status() {
    let (is_installed, is_running, exe_path, config_path) = shared::installer::check_status("host");
    let config = shared::config::HostConfig::load();

    let mut status_msg = format!(
        "Open Remote URL - Host Status\n\n\
        [Status]\n\
        - Installed:  {}\n\
        - Running:    {}\n\
        - HOST:       http://{}:{}",
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    shared::config::load_env("host");

    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 && args[1] == "--config" {
        let _ = shared::config::show_config("host");
        exit(0);
    }

    if args.len() < 2 {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            if shared::utils::is_double_clicked() {
                #[cfg(target_os = "windows")]
                shared::utils::detach_console();

                shared::gui::run_gui("host");
                exit(0);
            }
        }
        print_status();
        exit(0);
    }

    let arg = &args[1];

    if arg == "--install" {
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
        return Ok(());
    } else if arg == "--uninstall" {
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
        return Ok(());
    } else if arg == "--daemon" {
        // Hide/detach console on Windows if running in background daemon mode
        #[cfg(target_os = "windows")]
        shared::utils::detach_console();

        log::info!("Starting Host Daemon in background...");
        daemon::run().await?;
    } else {
        // Unknown argument: show help/status
        print_status();
        exit(0);
    }

    Ok(())
}
