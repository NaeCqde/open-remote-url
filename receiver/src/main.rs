#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod daemon;

use std::env;
use std::error::Error;
use std::process::exit;

fn print_status() {
    let (is_installed, is_running, exe_path, config_path) =
        shared::installer::check_status("receiver");
    let config = shared::config::ReceiverConfig::load();

    let auth_status = if config.passphrase.is_some() {
        "Enabled".to_string()
    } else {
        "DISABLED  *** Set PASSPHRASE if exposing to the internet ***".to_string()
    };

    let exe_display = if is_installed {
        exe_path.to_string_lossy().to_string()
    } else {
        "(Will be set upon installation)".to_string()
    };

    let status_msg = format!(
        "Open Remote URL - Receiver Status\n\n\
        [Status]\n\
        - Installed:      {}\n\
        - Running:        {}\n\
        - Executable:     {}\n\
        - Configuration:  {}\n\
        - Receiver URL:   http://{}/\n\
        - Auth:           {}",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        exe_display,
        config_path.to_string_lossy(),
        config.listen,
        auth_status,
    );

    println!("{}", status_msg);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    shared::config::load_env("receiver");

    let args: Vec<String> = env::args().collect();

    shared::cli::setup_gui_or_console("receiver", &args);

    if args.len() < 2 {
        print_status();
        exit(0);
    }

    let cmd = &args[1];

    if shared::cli::handle_common_command(cmd, "receiver") {
        return Ok(());
    }

    if cmd == "--daemon" {
        log::info!("Starting open-remote-url-receiver daemon...");
        daemon::run().await?;
    } else {
        print_status();
        exit(0);
    }

    Ok(())
}
