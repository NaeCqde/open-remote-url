pub fn is_double_clicked() -> bool {
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn GetConsoleProcessList(lpdwProcessList: *mut u32, dwProcessCount: u32) -> u32;
            fn AttachConsole(dwProcessId: u32) -> i32;
            fn FreeConsole() -> i32;
        }
        unsafe {
            let mut buffer = [0u32; 2];
            let count = GetConsoleProcessList(buffer.as_mut_ptr(), 2);
            if count > 0 {
                return count == 1;
            } else {
                if AttachConsole(0xFFFFFFFF) != 0 {
                    let _ = FreeConsole();
                    return false;
                }
                return true;
            }
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

pub fn attach_console() {
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
            fn SetStdHandle(nStdHandle: u32, hHandle: *mut std::ffi::c_void) -> i32;
            fn CreateFileA(
                lpFileName: *const u8,
                dwDesiredAccess: u32,
                dwShareMode: u32,
                lpSecurityAttributes: *mut std::ffi::c_void,
                dwCreationDisposition: u32,
                dwFlagsAndAttributes: u32,
                hTemplateFile: *mut std::ffi::c_void,
            ) -> *mut std::ffi::c_void;
        }
        unsafe {
            if AttachConsole(0xFFFFFFFF) != 0 {
                const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // -11
                const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;  // -12
                const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;  // -10
                
                const GENERIC_READ: u32 = 0x80000000;
                const GENERIC_WRITE: u32 = 0x40000000;
                const FILE_SHARE_WRITE: u32 = 2;
                const FILE_SHARE_READ: u32 = 1;
                const OPEN_EXISTING: u32 = 3;
                
                let h_out = CreateFileA(
                    "CONOUT$\0".as_ptr(),
                    GENERIC_WRITE,
                    FILE_SHARE_WRITE,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                );
                if h_out as isize != -1 {
                    SetStdHandle(STD_OUTPUT_HANDLE, h_out);
                    SetStdHandle(STD_ERROR_HANDLE, h_out);
                }
                
                let h_in = CreateFileA(
                    "CONIN$\0".as_ptr(),
                    GENERIC_READ,
                    FILE_SHARE_READ,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                );
                if h_in as isize != -1 {
                    SetStdHandle(STD_INPUT_HANDLE, h_in);
                }
            }
        }
    }
}

pub fn create_no_window(mut cmd: std::process::Command) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let _ = &mut cmd;
    cmd
}
