mod daemon;

use std::env;
use std::process::exit;

fn print_status() {
    let (is_installed, is_running, exe_path, config_path) = shared::installer::check_status("host");
    let config = shared::config::HostConfig::load();

    let ext = if cfg!(target_os = "windows") {
        "bat"
    } else if cfg!(target_os = "macos") {
        "command"
    } else {
        "sh"
    };

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

    status_msg.push_str(&format!(
        "\n\n\
        [Usage]\n\
        - To install / start host:\n  Run install.{} in the release folder\n\n\
        - To uninstall / clean registrations:\n  Run uninstall.{} in the release folder",
        ext, ext
    ));

    println!("{}", status_msg);
}

#[cfg(target_os = "windows")]
fn detach_console() {
    extern "system" {
        fn FreeConsole() -> i32;
    }
    unsafe {
        FreeConsole();
    }
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
        detach_console();

        log::info!("Starting Host Daemon in background...");
        daemon::run().await?;
    } else {
        // Unknown argument: show help/status
        print_status();
        exit(0);
    }

    Ok(())
}
