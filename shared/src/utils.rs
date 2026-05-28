pub fn is_double_clicked() -> bool {
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn GetConsoleProcessList(lpdwProcessList: *mut u32, dwProcessCount: u32) -> u32;
        }
        unsafe {
            let mut buffer = [0u32; 2];
            let count = GetConsoleProcessList(buffer.as_mut_ptr(), 2);
            return count == 1;
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::io::IsTerminal;
        return !std::io::stdout().is_terminal();
    }

    #[cfg(target_os = "linux")]
    {
        use std::io::IsTerminal;
        let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
        return has_display && !std::io::stdout().is_terminal();
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    false
}

pub fn detach_console() {
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn FreeConsole() -> i32;
        }
        unsafe {
            let _ = FreeConsole();
        }
    }
}
