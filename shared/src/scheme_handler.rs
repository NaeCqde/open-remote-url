use crate::config::ClientConfig;

/// Called at client daemon startup to register URL scheme handlers based on config.
/// SCHEME=vcc,unityhub  → registers those schemes so the OS invokes this exe
/// HTTP=false           → unregisters http/https handlers; default is true (register them)
pub fn register_url_schemes() {
    let config = ClientConfig::load();

    #[cfg(target_os = "windows")]
    register_windows(&config);

    #[cfg(target_os = "macos")]
    register_macos(&config);

    #[cfg(target_os = "linux")]
    register_linux(&config);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let _ = config;
}

// ─── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn register_windows(config: &ClientConfig) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log::error!("scheme_handler: cannot get exe path: {}", e);
            return;
        }
    };
    let exe_str = exe.to_string_lossy();
    let open_cmd = format!("\"{}\" \"%1\"", exe_str);

    for scheme in &config.schemes {
        reg_add_url_scheme(scheme, &open_cmd);
    }

    if config.register_http {
        register_windows_http_https(&exe_str, &open_cmd);
    } else {
        unregister_windows_http_https();
    }
}

#[cfg(target_os = "windows")]
fn reg_add_url_scheme(scheme: &str, open_cmd: &str) {
    let base = format!("HKCU\\Software\\Classes\\{}", scheme);
    let cmd_key = format!("{}\\shell\\open\\command", base);

    let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
        .args(&["add", &base, "/t", "REG_SZ", "/d", &format!("URL:{} Protocol", scheme), "/f"])
        .status();
    let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
        .args(&["add", &base, "/v", "URL Protocol", "/t", "REG_SZ", "/d", "", "/f"])
        .status();
    let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
        .args(&["add", &cmd_key, "/t", "REG_SZ", "/d", open_cmd, "/f"])
        .status();

    log::info!("Registered URL scheme handler: {}://", scheme);
}

#[cfg(target_os = "windows")]
fn register_windows_http_https(exe_str: &str, open_cmd: &str) {
    let bin_escaped = format!("\"{}\"", exe_str);

    let entries: &[(&str, &str, &str)] = &[
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient", "", ""),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities", "ApplicationName", "Open Remote URL Client"),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities", "ApplicationDescription", "Redirect URLs to remote Host"),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities\\URLAssociations", "http", "OpenRemoteURLClient"),
        ("HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities\\URLAssociations", "https", "OpenRemoteURLClient"),
        ("HKCU\\Software\\Classes\\OpenRemoteURLClient", "", "Open Remote URL Client HTML Document"),
        ("HKCU\\Software\\RegisteredApplications", "OpenRemoteURLClient", "Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient\\Capabilities"),
    ];

    for &(key, value_name, value_data) in entries {
        let data = if key.ends_with("OpenRemoteURLClient") && value_name.is_empty() {
            bin_escaped.as_str()
        } else {
            value_data
        };
        let mut args = vec!["add", key, "/t", "REG_SZ", "/d", data, "/f"];
        if !value_name.is_empty() {
            args.extend_from_slice(&["/v", value_name]);
        }
        let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
            .args(&args)
            .status();
    }

    // shell\open\command for the ProgID
    let cmd_key = "HKCU\\Software\\Classes\\OpenRemoteURLClient\\shell\\open\\command";
    let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
        .args(&["add", cmd_key, "/t", "REG_SZ", "/d", open_cmd, "/f"])
        .status();

    log::info!("Registered http/https URL scheme handlers");
}

#[cfg(target_os = "windows")]
fn unregister_windows_http_https() {
    for key in &[
        "HKCU\\Software\\Classes\\OpenRemoteURLClient",
        "HKCU\\Software\\Clients\\StartMenuInternet\\OpenRemoteURLClient",
    ] {
        let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
            .args(&["delete", key, "/f"])
            .status();
    }
    let _ = crate::utils::create_no_window(std::process::Command::new("reg"))
        .args(&["delete", "HKCU\\Software\\RegisteredApplications", "/v", "OpenRemoteURLClient", "/f"])
        .status();

    log::info!("Unregistered http/https URL scheme handlers");
}

// ─── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn register_macos(config: &ClientConfig) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log::error!("scheme_handler: cannot get exe path: {}", e);
            return;
        }
    };

    // Find the .app bundle that contains this executable.
    let bundle_path = find_macos_app_bundle(&exe);
    let Some(bundle) = bundle_path else {
        log::warn!("scheme_handler: not running from a .app bundle; skipping URL scheme registration");
        return;
    };

    let mut all_schemes: Vec<&str> = config.schemes.iter().map(|s| s.as_str()).collect();
    if config.register_http {
        all_schemes.push("http");
        all_schemes.push("https");
    }

    if let Err(e) = rewrite_info_plist(&bundle, &all_schemes) {
        log::error!("scheme_handler: failed to rewrite Info.plist: {}", e);
        return;
    }

    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";
    let _ = std::process::Command::new(lsregister)
        .arg("-f")
        .arg(&bundle)
        .status();

    let bundle_id = format!("quest.nae.open-remote-url.client");
    for scheme in &all_schemes {
        set_default_handler_macos(scheme, &bundle_id);
    }

    log::info!("Registered macOS URL scheme handlers: {:?}", all_schemes);
}

#[cfg(target_os = "macos")]
fn find_macos_app_bundle(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let parent = exe.parent()?;
    if parent.file_name()?.to_str()? == "MacOS" {
        let contents = parent.parent()?;
        if contents.file_name()?.to_str()? == "Contents" {
            let app = contents.parent()?;
            if app.extension()?.to_str()? == "app" {
                return Some(app.to_path_buf());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn rewrite_info_plist(bundle: &std::path::Path, schemes: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let info_plist = bundle.join("Contents").join("Info.plist");

    let app_type = "client";
    let name = "OpenRemoteURLClient";
    let bin = "open-remote-url-client";
    let bundle_id = format!("quest.nae.open-remote-url.{}", app_type);

    let scheme_entries: String = schemes
        .iter()
        .map(|s| format!("                <string>{}</string>\n", s))
        .collect();

    let url_types_block = if schemes.is_empty() {
        String::new()
    } else {
        format!(
            r#"    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>URL Schemes</string>
            <key>CFBundleURLSchemes</key>
            <array>
{scheme_entries}            </array>
        </dict>
    </array>
"#
        )
    };

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>{bin}</string>
    <key>LSUIElement</key>
    <true/>
{url_types_block}</dict>
</plist>
"#
    );

    std::fs::write(&info_plist, plist)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        alloc: *const std::ffi::c_void,
        c_str: *const std::ffi::c_char,
        encoding: u32,
    ) -> *mut std::ffi::c_void;
    fn CFRelease(cf: *const std::ffi::c_void);
    fn LSSetDefaultHandlerForURLScheme(
        scheme: *mut std::ffi::c_void,
        handler: *mut std::ffi::c_void,
    ) -> i32;
}

#[cfg(target_os = "macos")]
fn set_default_handler_macos(scheme: &str, bundle_id: &str) {
    use std::ffi::CString;
    // kCFStringEncodingUTF8
    const UTF8: u32 = 0x08000100;

    let Ok(scheme_c) = CString::new(scheme) else { return };
    let Ok(handler_c) = CString::new(bundle_id) else { return };

    unsafe {
        let scheme_cf = CFStringCreateWithCString(std::ptr::null(), scheme_c.as_ptr(), UTF8);
        let handler_cf = CFStringCreateWithCString(std::ptr::null(), handler_c.as_ptr(), UTF8);

        if !scheme_cf.is_null() && !handler_cf.is_null() {
            let result = LSSetDefaultHandlerForURLScheme(scheme_cf, handler_cf);
            if result != 0 {
                log::warn!("LSSetDefaultHandlerForURLScheme('{}') returned {}", scheme, result);
            }
        }

        if !scheme_cf.is_null() {
            CFRelease(scheme_cf);
        }
        if !handler_cf.is_null() {
            CFRelease(handler_cf);
        }
    }
}

// ─── Linux ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn register_linux(config: &ClientConfig) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log::error!("scheme_handler: cannot get exe path: {}", e);
            return;
        }
    };

    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };

    let app_dir = std::path::PathBuf::from(&home)
        .join(".local")
        .join("share")
        .join("applications");
    let _ = std::fs::create_dir_all(&app_dir);
    let desktop_path = app_dir.join("open-remote-url-client.desktop");

    let mut mime_types: Vec<String> = config
        .schemes
        .iter()
        .map(|s| format!("x-scheme-handler/{}", s))
        .collect();
    if config.register_http {
        mime_types.push("x-scheme-handler/http".to_string());
        mime_types.push("x-scheme-handler/https".to_string());
    }

    let mime_line = mime_types.join(";");
    let desktop_content = format!(
        "[Desktop Entry]\nType=Application\nName=Open Remote URL\nExec={} %u\nIcon=html\nTerminal=false\nNoDisplay=true\nMimeType={};\n",
        exe.to_string_lossy(),
        mime_line,
    );

    if let Err(e) = std::fs::write(&desktop_path, &desktop_content) {
        log::error!("scheme_handler: failed to write .desktop file: {}", e);
        return;
    }

    let _ = std::process::Command::new("update-desktop-database")
        .arg(&app_dir)
        .status();

    for mime in &mime_types {
        let _ = std::process::Command::new("xdg-mime")
            .args(&["default", "open-remote-url-client.desktop", mime])
            .status();
    }

    log::info!("Registered Linux URL scheme handlers: {:?}", mime_types);
}
