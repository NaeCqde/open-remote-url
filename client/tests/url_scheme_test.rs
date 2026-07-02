/// End-to-end URL scheme test for macOS.
///
/// Verifies the full pipeline:
///   open steam://install/4084250
///     → macOS sends kAEGetURL Apple Event to OpenRemoteURLClient.app
///     → app captures URL, POSTs to client daemon  /open
///     → client daemon forwards to host daemon      /open
///     → host daemon (mock) records the URL
///
/// Prerequisites (checked at runtime; test is skipped if not met):
///   - ~/Applications/OpenRemoteURLClient.app installed
///   - Port 30000 free (service daemon must not be running)
///   - `steam` in SCHEME or HTTP=true in the installed app config
#[cfg(target_os = "macos")]
mod url_scheme {
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    // ── helpers ─────────────────────────────────────────────────────────────

    /// Poll until a TCP connection to `addr` succeeds or `timeout` elapses.
    fn wait_for_port(addr: SocketAddr, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    /// Find the client binary next to the current test binary.
    fn find_client_bin() -> Option<std::path::PathBuf> {
        // test binary: target/{debug,release}/deps/<name>-<hash>
        // client bin:  target/{debug,release}/open-remote-url-client
        let exe = std::env::current_exe().ok()?;
        let bin = exe.parent()?.parent()?.join("open-remote-url-client");
        bin.exists().then_some(bin)
    }

    // ── test ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn open_command_url_forwarded_end_to_end() {
        // ── prerequisite: app bundle installed ──────────────────────────────
        let home = std::env::var("HOME").unwrap_or_default();
        let app_bundle = std::path::PathBuf::from(&home)
            .join("Applications")
            .join("OpenRemoteURLClient.app");
        if !app_bundle.exists() {
            eprintln!(
                "SKIP open_command_url_forwarded_end_to_end: \
                 ~/Applications/OpenRemoteURLClient.app not installed"
            );
            return;
        }

        // ── prerequisite: client binary built ───────────────────────────────
        let client_bin = match find_client_bin() {
            Some(b) => b,
            None => {
                eprintln!("SKIP: open-remote-url-client binary not found");
                return;
            }
        };

        // ── prerequisite: port 30000 free ───────────────────────────────────
        // (the installed app's Apple Event handler POSTs to whatever port is
        //  in its installed config — typically 30000)
        if std::net::TcpStream::connect_timeout(
            &"127.0.0.1:30000".parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            eprintln!(
                "SKIP open_command_url_forwarded_end_to_end: \
                 port 30000 already in use (service daemon running?)"
            );
            return;
        }

        // ── 1. Start a mock host server (captures /open POSTs) ───────────────
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap = captured.clone();

        let mock_app = axum::Router::new()
            .route("/", axum::routing::get(|| async { "alive" }))
            .route(
                "/open",
                axum::routing::post(
                    move |axum::Json(body): axum::Json<serde_json::Value>| {
                        let cap = cap.clone();
                        async move {
                            if let Some(url) =
                                body.get("url").and_then(|v| v.as_str())
                            {
                                *cap.lock().unwrap() = Some(url.to_string());
                            }
                            "OK"
                        }
                    },
                ),
            );

        let mock_listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mock_port = mock_listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(mock_listener, mock_app).await.ok();
        });

        // ── 2. Write temp .env → daemon uses this HOST_URL ───────────────────
        let tmpdir = tempfile::tempdir().unwrap();
        let env_content = format!(
            "LISTEN=0.0.0.0:30000\n\
             HOST_URL=http://127.0.0.1:{mock_port}\n\
             RELAY_URL=http://127.0.0.1:30000\n\
             PASSPHRASE=\n\
             SCHEME=steam\n\
             HTTP=false\n"
        );
        std::fs::write(tmpdir.path().join(".env"), env_content).unwrap();

        // ── 3. Spawn client daemon with tempdir as cwd ───────────────────────
        let mut daemon = std::process::Command::new(&client_bin)
            .arg("--daemon")
            .current_dir(tmpdir.path())
            .spawn()
            .expect("failed to spawn client daemon");

        // ── 4. Wait for daemon to be ready on port 30000 ────────────────────
        let daemon_addr: SocketAddr = "127.0.0.1:30000".parse().unwrap();
        assert!(
            wait_for_port(daemon_addr, Duration::from_secs(5)),
            "client daemon did not start within 5 seconds"
        );

        // ── 5. Invoke the installed app with the URL via `open -b` ──────────
        // macOS sends a kAEGetURL Apple Event to the bundle.
        // The bundle runs our url_event::capture() code which POSTs to port 30000.
        let open_status = std::process::Command::new("open")
            .args(["-b", "quest.nae.open-remote-url.client", "steam://install/4084250"])
            .status()
            .expect("failed to run `open`");
        assert!(open_status.success(), "`open` command failed");

        // ── 6. Give the chain time to complete ───────────────────────────────
        tokio::time::sleep(Duration::from_millis(800)).await;

        // ── 7. Assert ────────────────────────────────────────────────────────
        let got = captured.lock().unwrap().clone();
        assert_eq!(
            got.as_deref(),
            Some("steam://install/4084250"),
            "mock host did not receive the expected URL (got: {:?})",
            got
        );

        // ── cleanup ─────────────────────────────────────────────────────────
        daemon.kill().ok();
    }
}
