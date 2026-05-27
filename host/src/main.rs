mod daemon;
mod installer;

use std::env;
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
        "Open Remote URL - Host Status\n\n\
        [Status]\n\
        - Installed: {}\n\
        - Running: {}\n\n\
        [Usage]\n\
        - To install / start host:\n  Run install-host.{} in the release folder\n\n\
        - To uninstall / clean registrations:\n  Run uninstall-host.{} in the release folder",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        ext,
        ext
    );

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

    let arg = &args[1];

    if arg == "--install" {
        log::info!("Installing open-remote-url-host...");
        match installer::install() {
            Ok(_) => {
                println!("Host installation completed successfully!");
            }
            Err(e) => {
                println!("Installation failed: {}", e);
                exit(1);
            }
        }
        return Ok(());
    } else if arg == "--uninstall" {
        log::info!("Uninstalling open-remote-url-host...");
        match installer::uninstall() {
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
