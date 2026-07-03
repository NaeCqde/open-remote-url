#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod daemon;

#[cfg(target_os = "macos")]
mod url_event;

use std::env;
use std::error::Error;
use std::process::exit;

fn print_status() {
    let (is_installed, is_running, exe_path, config_path) =
        shared::installer::check_status("sender");
    let config = shared::config::SenderConfig::load();

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

    let schemes_display = if config.schemes.is_empty() {
        "(none)".to_string()
    } else {
        config.schemes.join(", ")
    };

    let http_display = if config.register_http { "Registered" } else { "Not registered" };

    let status_msg = format!(
        "Open Remote URL - Sender Status\n\n\
        [Status]\n\
        - Installed:        {}\n\
        - Running:          {}\n\
        - Executable:       {}\n\
        - Configuration:    {}\n\
        - Sender URL:       http://{}/\n\
        - Self URL:         {}/\n\
        - Host URL:         {}/\n\
        - Auth:             {}\n\
        - Custom Schemes:   {}\n\
        - HTTP/HTTPS:       {}",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        exe_display,
        config_path.to_string_lossy(),
        config.listen,
        config.self_url.trim_end_matches('/'),
        config.host_url.trim_end_matches('/'),
        auth_status,
        schemes_display,
        http_display,
    );

    println!("{}", status_msg);
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.contains("://")
}

/// POST `url` to the local daemon's `/open` endpoint. Prints an error and
/// exits(1) if the daemon can't be reached; on success, just logs.
async fn forward_url_to_daemon(url: &str) {
    let config = shared::config::SenderConfig::load();
    let sender_port = config.sender_port;
    let daemon_url = format!("http://localhost:{}/open", sender_port);

    log::info!("Sending URL to local daemon: {}", url);

    let client = reqwest::Client::new();
    let mut req = client
        .post(&daemon_url)
        .json(&serde_json::json!({ "url": url }));
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
            println!("=== Open Remote URL Error ===\nFailed to connect to local daemon: {}\nMake sure the daemon is running ('open-remote-url-sender --daemon')", e);
            exit(1);
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    // macOS: this binary is the registered URL scheme handler (CFBundleExecutable).
    // Whenever macOS launches/targets it for `open scheme://...`, the URL arrives
    // as a `kAEGetURL` Apple Event on the main thread's run loop -- regardless of
    // whether we were invoked with `--daemon` or with no args at all (e.g. because
    // the persistent daemon wasn't running yet and Launch Services spawned a fresh
    // instance). So install the handler unconditionally, before any GUI/CLI branch
    // decision runs, or a pending event would be silently dropped.
    #[cfg(target_os = "macos")]
    {
        url_event::listen();
        url_event::install_handler();
    }

    // macOS daemon: this process is the registered URL scheme handler.
    // When macOS launches it as a handler (e.g. `open steam://...`), the URL
    // arrives as a `kAEGetURL` Apple Event, which is delivered to the main
    // thread's CFRunLoop.  So the daemon runs the CFRunLoop on the MAIN thread
    // (to receive Apple Events) and the tokio HTTP server on a BACKGROUND
    // thread.  `url_event::listen()` sets up the channel bridging the two.
    #[cfg(target_os = "macos")]
    if args.iter().any(|a| a == "--daemon") {
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
        shared::config::load_env("sender");
        log::info!("Starting open-remote-url-sender daemon...");

        // Required for Launch Services to treat this long-lived process as a
        // properly checked-in running instance of the bundle -- otherwise a
        // later `open scheme://...` finds the PID but times out delivering
        // the Apple Event to it (errAETimeout / -1712).
        shared::gui::init_nsapplication_accessory();

        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new()
                .expect("failed to build tokio runtime");
            if let Err(e) = rt.block_on(daemon::run()) {
                eprintln!("daemon error: {}", e);
                std::process::exit(1);
            }
        });

        // Blocks forever on the main thread, driving NSApp's real run loop
        // (not a bare CFRunLoopRun) so Launch Services completes its
        // check-in and can deliver Apple Events to this running instance
        // (which the handler installed above forwards to the tokio daemon).
        shared::gui::run_nsapplication_forever();
        return Ok(());
    }

    // Everything else runs inside a standard tokio runtime.
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_main(args))
}

async fn async_main(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    shared::config::load_env("sender");

    // macOS: if we were launched to handle a URL scheme (e.g. `open steam://...`)
    // and the persistent `--daemon` process didn't catch the Apple Event itself,
    // this fresh instance will have received it here. Pump the run loop briefly
    // to let it arrive, forward it to the local daemon, and exit -- before ever
    // considering the GUI/status branches below.
    #[cfg(target_os = "macos")]
    if let Some(rx) = url_event::take_receiver() {
        url_event::pump_for(0.3);
        if let Ok(url) = rx.try_recv() {
            forward_url_to_daemon(&url).await;
            exit(0);
        }
    }

    shared::cli::setup_gui_or_console("sender", &args);

    if args.len() < 2 {
        print_status();
        exit(0);
    }

    let cmd_or_url = &args[1];

    if shared::cli::handle_common_command(cmd_or_url, "sender") {
        return Ok(());
    }

    if cmd_or_url == "--daemon" {
        log::info!("Starting open-remote-url-sender daemon...");
        daemon::run().await?;
    } else if looks_like_url(cmd_or_url) {
        forward_url_to_daemon(cmd_or_url).await;
    } else {
        print_status();
        exit(0);
    }

    Ok(())
}

/// Single-machine test: send a synthetic Apple Event to this process and
/// verify the URL arrives on the channel receiver.
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::url_event;

    /// Single-machine verification: set up the channel and install the Apple
    /// Event handler, then send a synthetic 'GURL'/'GURL' event with
    /// kAEWaitReply.  For same-process targets kAEWaitReply causes the Apple
    /// Event Manager to call the handler synchronously on the current call
    /// stack (inside AESendMessage), so this works from any thread without a
    /// run loop.  The handler sends the URL to the channel; we read it back
    /// via the receiver returned by `take_receiver()`.
    #[test]
    fn apple_event_url_roundtrip() {
        const TEST_URL: &str = "steam://install/4084250";

        // Create the channel and register the handler BEFORE sending.
        url_event::listen();
        url_event::install_handler();
        // kAEWaitReply: handler is invoked synchronously → URL sent to channel.
        url_event::send_to_self(TEST_URL);
        // The receiver now holds the forwarded URL.
        let rx = url_event::take_receiver().expect("receiver available");
        let got = rx.recv_timeout(std::time::Duration::from_secs(1)).ok();
        assert_eq!(got.as_deref(), Some(TEST_URL));
    }
}
