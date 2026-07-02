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

    let mut status_msg = format!(
        "Open Remote URL - Receiver Status\n\n\
        [Status]\n\
        - Installed:  {}\n\
        - Running:    {}\n\
        - Listen:     http://{}/\n\
        - Auth:       {}",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        config.listen,
        auth_status,
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
        - To install / start receiver:\n  Double-click the executable to open GUI Control Panel\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall"
    } else if cfg!(target_os = "macos") {
        "\n\n\
        [Usage]\n\
        - To install / start receiver:\n  Double-click the OpenRemoteURLReceiver.app bundle to open GUI Control Panel\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall"
    } else {
        "\n\n\
        [Usage]\n\
        - To install / start receiver:\n  Double-click the executable to open GUI Control Panel (GUI Desktop)\n  Or run ./install.sh in the release folder (CLI/Headless)\n\n\
        - To uninstall / clean registrations:\n  Open GUI Control Panel and click Uninstall (GUI)\n  Or run ./uninstall.sh in the release folder (CLI/Headless)"
    };

    status_msg.push_str(usage_msg);

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
