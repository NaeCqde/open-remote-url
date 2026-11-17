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

/// Called at every start of the Sender, right before it takes over http/https.
/// - The default is another browser: that is the real one. Remember it and write it to `BROWSER` in the installed `.env`
///   (overwriting, so `BROWSER` follows the browser the user actually uses), then the Sender replaces the default.
/// - The default already is the Sender: nothing to learn from the OS; `BROWSER` (else the remembered browser) is used as is.
///   Only when neither exists, and on Windows, a browser the system has registered is taken as a start.
pub fn remember_default_browser() {
    let current = current_default_browser();
    let real = match current {
        Some(c) if !is_own(&c) => Some(c),
        _ if configured_browser().is_none() => registered_browser_candidate(),
        _ => None,
    };
    let Some(browser) = real else {
        return;
    };
    let path = saved_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::read_to_string(&path).map(|s| s.trim() != browser).unwrap_or(true) {
        let _ = std::fs::write(&path, &browser);
    }
    let env_path = crate::config::get_config_path("sender");
    let same = std::env::var("BROWSER").map(|b| b.trim() == browser).unwrap_or(false);
    // Only into an existing installed .env: creating one here would hide the template .env of a not-yet-installed Sender.
    if !same && env_path.exists() {
        match crate::config::set_env_value(&env_path, "BROWSER", &browser) {
            Ok(()) => log::info!("BROWSER={} written to {}", browser, env_path.display()),
            Err(e) => log::warn!("Cannot write BROWSER to {}: {}", env_path.display(), e),
        }
    }
    if !same {
        // This process keeps using the value it just learned.
        std::env::set_var("BROWSER", &browser);
    }
}

/// Uninstall time: put the real browser back as the OS default for http/https, if the Sender still is the default. The
/// browser is `BROWSER` from the `.env` (kept on purpose), else the remembered one. Returns a line to show the user.
pub fn restore_default_browser() -> String {
    crate::config::load_env("sender");
    restore_platform()
}

fn restore_candidates() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(b) = std::env::var("BROWSER") {
        v.push(b.trim().to_string());
    }
    if let Ok(b) = std::fs::read_to_string(saved_path()) {
        v.push(b.trim().to_string());
    }
    v.retain(|b| !b.is_empty() && !is_own(b));
    v
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
fn registered_browser_candidate() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn restore_platform() -> String {
    match current_default_browser() {
        Some(c) if is_own(&c) => {}
        _ => return "Default browser: not changed (the Sender is not the default).".into(),
    }
    // A bundle id is needed to set the default; an app name in BROWSER cannot be used for it.
    let Some(target) = restore_candidates()
        .into_iter()
        .find(|b| b.contains('.') && !b.contains('/') && !b.ends_with(".app"))
    else {
        return "Default browser: not restored (no browser bundle id in BROWSER or remembered). Choose one in System Settings.".into();
    };
    crate::scheme_handler::set_default_handler_macos("http", &target);
    crate::scheme_handler::set_default_handler_macos("https", &target);
    format!("Default browser restored to {}.", target)
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
fn registered_browser_candidate() -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn restore_platform() -> String {
    let own_desktop = std::env::var("HOME").ok().map(|h| {
        PathBuf::from(h)
            .join(".local/share/applications")
            .join(OWN_LINUX_DESKTOP)
    });
    let was_default = current_default_browser().map(|c| is_own(&c)).unwrap_or(false);
    let target = restore_candidates().into_iter().find(|b| b.ends_with(".desktop"));
    let msg = match (was_default, target) {
        (false, _) => "Default browser: not changed (the Sender is not the default).".to_string(),
        (true, None) => "Default browser: not restored (no .desktop id in BROWSER or remembered).".to_string(),
        (true, Some(t)) => {
            for mime in ["x-scheme-handler/http", "x-scheme-handler/https"] {
                let _ = std::process::Command::new("xdg-mime").args(["default", &t, mime]).status();
            }
            let _ = std::process::Command::new("xdg-settings")
                .args(["set", "default-web-browser", &t])
                .status();
            format!("Default browser restored to {}.", t)
        }
    };
    if let Some(p) = own_desktop {
        if p.exists() {
            let _ = std::fs::remove_file(&p);
            if let Some(dir) = p.parent() {
                let _ = std::process::Command::new("update-desktop-database").arg(dir).status();
            }
        }
    }
    msg
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

/// A browser the system has registered (StartMenuInternet), other than the Sender and without an https association
/// (Internet Explorer). The first one found; all are logged.
#[cfg(target_os = "windows")]
fn registered_browser_candidate() -> Option<String> {
    let mut found: Vec<String> = Vec::new();
    for root in ["HKLM\\SOFTWARE\\Clients\\StartMenuInternet", "HKCU\\Software\\Clients\\StartMenuInternet"] {
        let Some(out) = reg_query(&[root]) else { continue };
        for key in out.lines().map(str::trim).filter(|l| l.starts_with("HKEY_")) {
            if key.eq_ignore_ascii_case(root) || key.to_ascii_lowercase().contains("openremoteurlsender") {
                continue;
            }
            let assoc = format!("{}\\Capabilities\\URLAssociations", key);
            if let Some(progid) = reg_query(&[&assoc, "/v", "https"]).and_then(|o| reg_sz_data(&o)) {
                if !is_own(&progid) && !found.contains(&progid) {
                    found.push(progid);
                }
            }
        }
    }
    if found.len() > 1 {
        log::info!("Several browsers are registered: {:?}; using the first. Set BROWSER to choose another.", found);
    }
    found.into_iter().next()
}

/// Windows cannot set the default browser from a program (the choice is hash-protected). The registrations of the
/// Sender are removed by the uninstall itself; the user picks the browser in Settings.
#[cfg(target_os = "windows")]
fn restore_platform() -> String {
    "Default browser: Windows does not let a program set it. The Sender's registration is removed; choose your browser in Settings > Apps > Default apps.".into()
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
fn registered_browser_candidate() -> Option<String> {
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn restore_platform() -> String {
    String::new()
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
