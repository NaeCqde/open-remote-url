#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod daemon;

use std::env;
use std::error::Error;
use std::process::exit;

/// macOS Apple Event handler for URL scheme invocations.
///
/// When macOS launches this app as a URL scheme handler (via `open scheme://...`
/// or by clicking a link), the URL is delivered as an Apple Event — NOT as a
/// command-line argument.  This module registers a 'GURL'/'GURL' handler,
/// briefly spins the run loop to receive any pending event, and returns the URL.
#[cfg(target_os = "macos")]
mod url_event {
    use std::ffi::c_void;
    use std::sync::{Mutex, OnceLock};

    static CAPTURED: OnceLock<Mutex<Option<String>>> = OnceLock::new();

    fn store() -> &'static Mutex<Option<String>> {
        CAPTURED.get_or_init(|| Mutex::new(None))
    }

    // Apple Event four-char-code constants
    const K_INTERNET_EVENT_CLASS: u32 = 0x4755_524C; // 'GURL'
    const K_AE_GET_URL: u32           = 0x4755_524C; // 'GURL'
    const K_KEY_DIRECT_OBJECT: u32    = 0x2D2D_2D2D; // '----'
    const K_TYPE_WILDCARD: u32        = 0x2A2A_2A2A; // '****'
    const K_TYPE_CHAR: u32            = 0x5445_5854; // 'TEXT'
    // AESendMessage send-mode flags
    // ProcessSerialNumber.lo for the current process
    const K_CURRENT_PROCESS_LO: u32   = 2;
    const K_TYPE_PROCESS_SERIAL_NUM: u32 = 0x7073_6E20; // 'psn '

    /// AEDesc as laid out in memory on 64-bit macOS (16 bytes).
    #[repr(C)]
    pub struct AEDesc {
        pub descriptor_type: u32,
        pub data_handle: *mut c_void, // 4-byte pad + 8-byte ptr via repr(C) alignment
    }

    impl AEDesc {
        pub const fn null() -> Self {
            Self { descriptor_type: 0, data_handle: std::ptr::null_mut() }
        }
    }

    // Safety: AEDesc is only used on the main thread in the Apple Event handler.
    unsafe impl Send for AEDesc {}
    unsafe impl Sync for AEDesc {}

    #[repr(C)]
    struct ProcessSerialNumber { hi: u32, lo: u32 }

    type HandlerUPP = unsafe extern "C" fn(*const AEDesc, *mut AEDesc, isize) -> i32;

    #[link(name = "CoreServices", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn AEInstallEventHandler(ec: u32, ei: u32, h: HandlerUPP, r: isize, sys: bool) -> i32;
        fn AEGetParamDesc(ev: *const AEDesc, kw: u32, dt: u32, out: *mut AEDesc) -> i32;
        fn AEGetDescDataSize(d: *const AEDesc) -> isize;
        fn AEGetDescData(d: *const AEDesc, buf: *mut c_void, max: isize) -> i32;
        fn AEDisposeDesc(d: *mut AEDesc) -> i32;
        fn AECreateDesc(ty: u32, ptr: *const c_void, sz: isize, out: *mut AEDesc) -> i32;
        fn AECreateAppleEvent(
            ec: u32, ei: u32, target: *const AEDesc,
            return_id: i16, transaction_id: i32, out: *mut AEDesc,
        ) -> i32;
        fn AEPutParamPtr(
            event: *mut AEDesc, kw: u32, ty: u32,
            data: *const c_void, sz: isize,
        ) -> i32;
        fn AESendMessage(
            event: *const AEDesc, reply: *mut AEDesc,
            mode: i32, timeout: i32,
        ) -> i32;
        fn CFRunLoopRunInMode(mode: *const c_void, secs: f64, stop_after_one: bool) -> i32;
        static kCFRunLoopDefaultMode: *const c_void;
    }

    unsafe extern "C" fn on_get_url(
        event: *const AEDesc,
        _reply: *mut AEDesc,
        _refcon: isize,
    ) -> i32 {
        let mut desc = AEDesc::null();
        if AEGetParamDesc(event, K_KEY_DIRECT_OBJECT, K_TYPE_WILDCARD, &mut desc) == 0 {
            let sz = AEGetDescDataSize(&desc) as usize;
            if sz > 0 && sz < 8192 {
                let mut buf = vec![0u8; sz];
                if AEGetDescData(&desc, buf.as_mut_ptr() as *mut c_void, sz as isize) == 0 {
                    let url = String::from_utf8_lossy(&buf)
                        .trim_matches('\0')
                        .to_string();
                    if !url.is_empty() {
                        if let Ok(mut g) = store().lock() {
                            *g = Some(url);
                        }
                    }
                }
            }
            AEDisposeDesc(&mut desc);
        }
        0
    }

    /// Register the 'open URL' Apple Event handler.  Safe to call multiple times.
    pub fn install_handler() {
        let _ = store();
        unsafe {
            AEInstallEventHandler(K_INTERNET_EVENT_CLASS, K_AE_GET_URL, on_get_url, 0, false);
        }
    }

    /// Register the handler and run the main CFRunLoop for up to 200 ms to
    /// receive any 'open URL' Apple Event that macOS delivered at launch.
    /// Returns the URL if one arrived, or `None` for a normal launch.
    pub fn capture() -> Option<String> {
        install_handler();
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.2, false);
        }
        store().lock().ok()?.take()
    }

    /// Send a synthetic 'open URL' Apple Event to the current process using
    /// `kAEWaitReply`.  When the target is the same process, the Apple Event
    /// Manager dispatches the event synchronously on the *current call stack*
    /// (i.e. inside `AESendMessage`), so no run-loop spin is needed.
    /// This makes it usable from test threads.
    pub fn send_to_self(url: &str) {
        const K_AE_WAIT_REPLY: i32 = 3;
        const K_AE_DEFAULT_TIMEOUT: i32 = -1;

        unsafe {
            let psn = ProcessSerialNumber { hi: 0, lo: K_CURRENT_PROCESS_LO };
            let mut target = AEDesc::null();
            AECreateDesc(
                K_TYPE_PROCESS_SERIAL_NUM,
                &psn as *const _ as *const c_void,
                std::mem::size_of::<ProcessSerialNumber>() as isize,
                &mut target,
            );

            let mut event = AEDesc::null();
            AECreateAppleEvent(
                K_INTERNET_EVENT_CLASS, K_AE_GET_URL,
                &target, -1, 0, &mut event,
            );

            AEPutParamPtr(
                &mut event,
                K_KEY_DIRECT_OBJECT, K_TYPE_CHAR,
                url.as_ptr() as *const c_void, url.len() as isize,
            );

            let mut reply = AEDesc::null();
            // kAEWaitReply: for same-process targets, the handler is invoked
            // synchronously on this call stack before AESendMessage returns.
            AESendMessage(&event, &mut reply, K_AE_WAIT_REPLY, K_AE_DEFAULT_TIMEOUT);
            AEDisposeDesc(&mut reply);
            AEDisposeDesc(&mut event);
            AEDisposeDesc(&mut target);
        }
    }
}

fn print_status() {
    let (is_installed, is_running, exe_path, config_path) =
        shared::installer::check_status("client");
    let config = shared::config::ClientConfig::load();

    let auth_status = if config.passphrase.is_some() {
        "Enabled".to_string()
    } else {
        "DISABLED  *** Set PASSPHRASE if exposing to the internet ***".to_string()
    };

    let mut status_msg = format!(
        "Open Remote URL - Client Status\n\n\
        [Status]\n\
        - Installed:  {}\n\
        - Running:    {}\n\
        - Listen:     http://{}/\n\
        - RELAY:      {}/\n\
        - HOST:       {}/\n\
        - Auth:       {}",
        if is_installed { "Yes" } else { "No" },
        if is_running { "Yes" } else { "No" },
        config.listen,
        config.relay_url.trim_end_matches('/'),
        config.host_url.trim_end_matches('/'),
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
    let args: Vec<String> = env::args().collect();

    // macOS: when this app is launched as a URL scheme handler (e.g. via
    // `open steam://...`), the URL arrives as an Apple Event — not as argv.
    // Check for it BEFORE setup_gui_or_console so we never open the GUI.
    #[cfg(target_os = "macos")]
    if !args.iter().any(|a| a == "--daemon") {
        if let Some(url) = url_event::capture() {
            shared::config::load_env("client");
            let config = shared::config::ClientConfig::load();
            let client = reqwest::Client::new();
            let mut req = client
                .post(format!("http://localhost:{}/open", config.client_port))
                .json(&serde_json::json!({ "url": url }));
            if let Some(ref phrase) = config.passphrase {
                req = req.header("Authorization", format!("Bearer {}", phrase));
            }
            match req.send().await {
                Ok(r) if r.status().is_success() => {}
                Ok(r) => eprintln!("Daemon returned error: {}", r.status()),
                Err(e) => eprintln!("Failed to reach daemon: {}", e),
            }
            return Ok(());
        }
    }

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    shared::config::load_env("client");

    shared::cli::setup_gui_or_console("client", &args);

    if args.len() < 2 {
        print_status();
        exit(0);
    }

    let cmd_or_url = &args[1];

    if shared::cli::handle_common_command(cmd_or_url, "client") {
        return Ok(());
    }

    if cmd_or_url == "--daemon" {
        log::info!("Starting open-remote-url-client daemon...");
        daemon::run().await?;
    } else if looks_like_url(cmd_or_url) {
        let config = shared::config::ClientConfig::load();
        let client_port = config.client_port;
        let daemon_url = format!("http://localhost:{}/open", client_port);

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
        print_status();
        exit(0);
    }

    Ok(())
}

/// Single-machine test: send a synthetic Apple Event to this process and
/// verify that `url_event::capture()` returns the URL.
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::url_event;

    /// Single-machine verification: install the Apple Event handler first,
    /// then send a synthetic 'GURL'/'GURL' event with kAEWaitReply.
    /// For same-process targets kAEWaitReply causes the Apple Event Manager
    /// to call the handler synchronously on the current call stack (inside
    /// AESendMessage), so this works from any thread without a run loop.
    #[test]
    fn apple_event_url_roundtrip() {
        const TEST_URL: &str = "steam://install/4084250";

        // Register the handler BEFORE sending the event.
        url_event::install_handler();
        // kAEWaitReply: handler is invoked synchronously → URL stored.
        url_event::send_to_self(TEST_URL);
        // capture() returns whatever the handler stored (no extra run-loop needed).
        let got = url_event::capture();
        assert_eq!(got.as_deref(), Some(TEST_URL));
    }
}
