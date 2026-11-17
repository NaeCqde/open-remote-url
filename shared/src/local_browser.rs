//! The "real" default browser, for URLs the Sender decides to open locally (see `domain_filter`).
//!
//! The Sender registers itself as the OS handler of http/https, so the user's own browser is no longer the default. To
//! still open a URL in it, the Sender
//! 1. remembers the handler that was the default just before it replaced it (`remember_default_browser`, called by the
//!    scheme registration; never overwritten by the Sender itself), or uses `BROWSER` from the `.env` when set, and
//! 2. launches that application directly (macOS `open -b <bundle id>`, Linux `gtk-launch`, Windows the ProgID's command), never
//!    through the OS default, so the URL cannot come back to the Sender in a loop.
//!
//! `BROWSER` values: macOS a bundle id (`com.kagi.kagimacos`) or an app name / `.app` path (`Orion`); Linux a `.desktop`
//! id (`firefox.desktop`); Windows a ProgID (`ChromeHTML`) or the path of an `.exe`.

use crate::config::get_config_dir;
use std::path::PathBuf;

const OWN_MAC_BUNDLE_ID: &str = "quest.nae.open-remote-url.sender";
const OWN_LINUX_DESKTOP: &str = "open-remote-url-sender.desktop";
const OWN_WINDOWS_PROGID: &str = "OpenRemoteURLSender";

fn saved_path() -> PathBuf {
    get_config_dir("sender").join("local_browser")
}

fn is_own(id: &str) -> bool {
    let id = id.trim();
    id.eq_ignore_ascii_case(OWN_MAC_BUNDLE_ID)
        || id.eq_ignore_ascii_case(OWN_LINUX_DESKTOP)
        || id.starts_with(OWN_WINDOWS_PROGID)
}

/// `BROWSER` from the `.env`, else the browser remembered at registration. Never the Sender itself.
pub fn configured_browser() -> Option<String> {
    let from_env = std::env::var("BROWSER").ok().map(|s| s.trim().to_string());
    let value = match from_env.filter(|s| !s.is_empty()) {
        Some(v) => Some(v),
        None => std::fs::read_to_string(saved_path())
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    };
    value.filter(|v| !is_own(v))
}

/// Call right before the Sender takes over http/https. Remembers the current default browser unless it is already the
/// Sender (a later start must not forget the real one).
pub fn remember_default_browser() {
    let Some(current) = current_default_browser() else {
        return;
    };
    if is_own(&current) {
        return;
    }
    if std::fs::read_to_string(saved_path()).map(|s| s.trim() == current).unwrap_or(false) {
        return;
    }
    let path = saved_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, &current) {
        Ok(()) => log::info!("Remembered the default browser for local opening: {}", current),
        Err(e) => log::warn!("Cannot save the default browser ({}): {}", path.display(), e),
    }
}

/// Open `url` in the real default browser. `Err` when none is known or it cannot be launched (the caller falls back to
/// sending the URL to the Receiver, so a URL is never lost).
pub fn open_locally(url: &str) -> Result<(), String> {
    // Only ever hand an http(s) URL to a launcher: a local path or other scheme must not reach an arbitrary program.
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("not an http(s) URL".into());
    }
    let browser = configured_browser().ok_or_else(|| {
        "no local browser known: set BROWSER in .env, or re-register after setting your browser as the default".to_string()
    })?;
    launch(&browser, url)
}

// ─── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void, CString};

    #[link(name = "CoreFoundation", kind = "framework")]
    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, enc: u32) -> *mut c_void;
        fn CFStringGetCString(s: *const c_void, buf: *mut c_char, len: isize, enc: u32) -> bool;
        fn CFRelease(cf: *const c_void);
        fn LSCopyDefaultHandlerForURLScheme(scheme: *mut c_void) -> *mut c_void;
    }
    const UTF8: u32 = 0x08000100;

    pub fn default_handler(scheme: &str) -> Option<String> {
        let c = CString::new(scheme).ok()?;
        unsafe {
            let s = CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8);
            if s.is_null() {
                return None;
            }
            let h = LSCopyDefaultHandlerForURLScheme(s);
            CFRelease(s);
            if h.is_null() {
                return None;
            }
            let mut buf = vec![0 as c_char; 512];
            let ok = CFStringGetCString(h, buf.as_mut_ptr(), buf.len() as isize, UTF8);
            CFRelease(h);
            if !ok {
                return None;
            }
            let bytes: Vec<u8> = buf.iter().take_while(|&&b| b != 0).map(|&b| b as u8).collect();
            String::from_utf8(bytes).ok()
        }
    }
}

#[cfg(target_os = "macos")]
fn current_default_browser() -> Option<String> {
    mac::default_handler("https").or_else(|| mac::default_handler("http"))
}

#[cfg(target_os = "macos")]
fn launch(browser: &str, url: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new("open");
    // A bundle id has dots and no path separator or `.app` suffix; anything else is an app name or path.
    let is_bundle_id = browser.contains('.') && !browser.contains('/') && !browser.ends_with(".app");
    cmd.arg(if is_bundle_id { "-b" } else { "-a" }).arg(browser).arg(url);
    run(cmd)
}

// ─── Linux ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn current_default_browser() -> Option<String> {
    let out = std::process::Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

#[cfg(target_os = "linux")]
fn launch(browser: &str, url: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new("gtk-launch");
    cmd.arg(browser).arg(url);
    run(cmd)
}

// ─── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn reg_query(args: &[&str]) -> Option<String> {
    let out = crate::utils::create_no_window(std::process::Command::new("reg"))
        .arg("query")
        .args(args)
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// The data of the first `REG_SZ` value in `reg query` output.
#[cfg(target_os = "windows")]
fn reg_sz_data(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let (_, data) = l.split_once("REG_SZ")?;
        Some(data.trim().to_string()).filter(|s| !s.is_empty())
    })
}

#[cfg(target_os = "windows")]
fn current_default_browser() -> Option<String> {
    let key = "HKCU\\Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\https\\UserChoice";
    reg_sz_data(&reg_query(&[key, "/v", "ProgId"])?)
}

#[cfg(target_os = "windows")]
fn launch(browser: &str, url: &str) -> Result<(), String> {
    // An .exe path is used directly; otherwise the value is a ProgID whose open command is looked up.
    if browser.to_ascii_lowercase().ends_with(".exe") {
        let mut cmd = crate::utils::create_no_window(std::process::Command::new(browser));
        cmd.arg(url);
        return run(cmd);
    }
    let key = format!("HKCR\\{}\\shell\\open\\command", browser);
    let template = reg_sz_data(&reg_query(&[&key, "/ve"]).ok_or("browser command not found in the registry")?)
        .ok_or("empty browser command")?;
    // `"C:\path\browser.exe" --flag %1` → program + arguments with the URL substituted for %1 / %L.
    let template = template.trim();
    let (program, rest) = if let Some(stripped) = template.strip_prefix('"') {
        let end = stripped.find('"').ok_or("malformed browser command")?;
        (stripped[..end].to_string(), stripped[end + 1..].to_string())
    } else {
        match template.split_once(' ') {
            Some((p, r)) => (p.to_string(), r.to_string()),
            None => (template.to_string(), String::new()),
        }
    };
    let mut cmd = crate::utils::create_no_window(std::process::Command::new(program));
    let mut had_url = false;
    for arg in rest.split_whitespace() {
        let arg = arg.trim_matches('"');
        if arg == "%1" || arg == "%L" || arg == "%l" {
            cmd.arg(url);
            had_url = true;
        } else {
            cmd.arg(arg);
        }
    }
    if !had_url {
        cmd.arg(url);
    }
    run(cmd)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn current_default_browser() -> Option<String> {
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn launch(_browser: &str, _url: &str) -> Result<(), String> {
    Err("unsupported platform".into())
}

/// Start the launcher without waiting for the browser. Some launchers (a browser that was not running, on Windows) stay
/// alive until the browser is closed, so waiting for exit would block for minutes. A launcher that fails does so at
/// once; give it a short moment to, then treat a still-running one as success and reap it in the background.
fn run(mut cmd: std::process::Command) -> Result<(), String> {
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot start the browser launcher: {}", e))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    loop {
        match child.try_wait() {
            Ok(Some(s)) if s.success() => return Ok(()),
            Ok(Some(s)) => return Err(format!("browser launcher exited with {}", s)),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            Ok(None) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
            Err(e) => return Err(format!("cannot check the browser launcher: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_handler_is_never_a_fallback() {
        assert!(is_own("quest.nae.open-remote-url.sender"));
        assert!(is_own("Quest.Nae.Open-Remote-URL.Sender"));
        assert!(is_own("open-remote-url-sender.desktop"));
        assert!(is_own("OpenRemoteURLSender"));
        assert!(!is_own("com.kagi.kagimacos"));
        // the Receiver's id is a different program and is a legitimate (if odd) choice
        assert!(!is_own("quest.nae.open-remote-url.receiver"));
    }

    #[test]
    fn run_does_not_wait_for_a_long_lived_launcher() {
        #[cfg(unix)]
        let (prog, args) = ("sleep", vec!["20"]);
        #[cfg(windows)]
        let (prog, args) = ("ping", vec!["-n", "20", "127.0.0.1"]);
        let mut c = std::process::Command::new(prog);
        c.args(args);
        let t = std::time::Instant::now();
        assert!(run(c).is_ok());
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn run_reports_a_failing_launcher() {
        #[cfg(unix)]
        let c = std::process::Command::new("false");
        #[cfg(windows)]
        let c = {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "exit 1"]);
            c
        };
        assert!(run(c).is_err());
        assert!(run(std::process::Command::new("/definitely/not/a/program")).is_err());
    }

    #[test]
    fn non_http_urls_are_refused() {
        assert!(open_locally("file:///etc/passwd").is_err());
        assert!(open_locally("/Applications/Calculator.app").is_err());
        assert!(open_locally("steam://run/1").is_err());
    }

    /// Manual probe for a real machine (`cargo test -p shared --lib probe_real_launch -- --ignored --nocapture`).
    /// Needs `ORU_PROBE_BROWSER` (what `BROWSER` would hold) and optionally `ORU_PROBE_URL`.
    #[test]
    #[ignore]
    fn probe_real_launch() {
        eprintln!("DETECTED={:?}", current_default_browser());
        let browser = std::env::var("ORU_PROBE_BROWSER").expect("set ORU_PROBE_BROWSER");
        let url = std::env::var("ORU_PROBE_URL").unwrap_or_else(|_| "https://example.com/open-remote-url-probe".into());
        eprintln!("LAUNCH={:?}", launch(&browser, &url));
    }
}
