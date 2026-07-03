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

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    // macOS daemon: register the "open application" handler unconditionally,
    // before any GUI/CLI branch decision, the same way the sender does --
    // see sender/src/main.rs for the full rationale. When this process
    // becomes the long-lived --daemon (the instance Launch Services checks
    // in), a plain double-click on the .app is routed to it as
    // kAEOpenApplication/kAEReopenApplication rather than spawning a fresh
    // process; without a handler for that, the daemon can't respond and
    // Finder reports "Application Not Responding". Spawn a new instance
    // instead, which takes the normal GUI branch and shows a window.
    #[cfg(target_os = "macos")]
    if args.iter().any(|a| a == "--daemon") {
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
        shared::config::load_env("receiver");
        log::info!("Starting open-remote-url-receiver daemon...");

        shared::gui::init_nsapplication_accessory();
        shared::mac_apple_events::install_open_application_handler();

        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new()
                .expect("failed to build tokio runtime");
            if let Err(e) = rt.block_on(daemon::run()) {
                eprintln!("daemon error: {}", e);
                std::process::exit(1);
            }
        });

        // `-[NSApplication run]` below re-installs AppKit's own default
        // kAEOpenApplication/kAEReopenApplication handlers during
        // `-finishLaunching`, silently overwriting the one installed above
        // (last AEInstallEventHandler call for a given event wins). Left
        // alone, a later double-click hits AppKit's default handler, which
        // tries to bring this windowless Accessory process to the front and
        // gets denied by WindowServer -- so nothing visibly happens. Re-assert
        // our handler shortly after `-run()` starts, once `-finishLaunching`
        // has had time to complete.
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(500));
            shared::mac_apple_events::install_open_application_handler();
        });

        // Blocks forever on the main thread, driving NSApp's real run loop so
        // Apple Events (e.g. kAEReopenApplication above) are delivered.
        shared::gui::run_nsapplication_forever();
        return Ok(());
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_main(args))
}

async fn async_main(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    shared::config::load_env("receiver");

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
