/// macOS Apple Event handler for URL scheme invocations.
///
/// Architecture (macOS daemon only):
///   Main thread  → NSApp + CFRunLoopRun  (receives kAEGetURL Apple Events)
///   Tokio thread → axum HTTP server      (URL forwarding to host)
///
/// `listen()` sets up a std::sync::mpsc channel whose sender is stored in a
/// global and called by the C Apple Event handler.  The receiver is retrieved
/// by `take_receiver()` inside `daemon::run()` and bridged into the tokio
/// async world via `spawn_blocking`.

use std::ffi::c_void;
use std::sync::{mpsc, Mutex, OnceLock};

// ── Channel storage ──────────────────────────────────────────────────────────

static SENDER: OnceLock<Mutex<mpsc::Sender<String>>> = OnceLock::new();
static RECEIVER: OnceLock<Mutex<Option<mpsc::Receiver<String>>>> = OnceLock::new();

// ── Apple Event constants ────────────────────────────────────────────────────

const K_INTERNET_EVENT_CLASS: u32 = 0x4755_524C; // 'GURL'
const K_AE_GET_URL: u32           = 0x4755_524C; // 'GURL'
const K_KEY_DIRECT_OBJECT: u32    = 0x2D2D_2D2D; // '----'
const K_TYPE_WILDCARD: u32        = 0x2A2A_2A2A; // '****'
#[cfg(test)]
const K_TYPE_CHAR: u32            = 0x5445_5854; // 'TEXT'
#[cfg(test)]
const K_TYPE_PROCESS_SERIAL_NUM: u32 = 0x7073_6E20; // 'psn '
#[cfg(test)]
const K_CURRENT_PROCESS_LO: u32   = 2;

/// AEDesc layout on 64-bit macOS (4-byte type + 4-byte pad + 8-byte ptr = 16 B).
#[repr(C)]
pub struct AEDesc {
    pub descriptor_type: u32,
    pub data_handle: *mut c_void,
}

impl AEDesc {
    pub const fn null() -> Self {
        Self { descriptor_type: 0, data_handle: std::ptr::null_mut() }
    }
}

// Safety: AEDesc is only used while holding the Apple Event stack frame.
unsafe impl Send for AEDesc {}
unsafe impl Sync for AEDesc {}

#[cfg(test)]
#[repr(C)]
struct ProcessSerialNumber { hi: u32, lo: u32 }

type HandlerUPP = unsafe extern "C" fn(*const AEDesc, *mut AEDesc, isize) -> i32;

// ── FFI ─────────────────────────────────────────────────────────────────────

#[link(name = "CoreServices",   kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn AEInstallEventHandler(ec: u32, ei: u32, h: HandlerUPP, r: isize, sys: bool) -> i32;
    fn AEGetParamDesc(ev: *const AEDesc, kw: u32, dt: u32, out: *mut AEDesc) -> i32;
    fn AEGetDescDataSize(d: *const AEDesc) -> isize;
    fn AEGetDescData(d: *const AEDesc, buf: *mut c_void, max: isize) -> i32;
    fn AEDisposeDesc(d: *mut AEDesc) -> i32;
    fn CFRunLoopRun();
}

// For tests: synthesize an Apple Event and dispatch it to the current process.
#[cfg(test)]
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn AECreateDesc(ty: u32, ptr: *const c_void, sz: isize, out: *mut AEDesc) -> i32;
    fn AECreateAppleEvent(
        ec: u32, ei: u32, target: *const AEDesc,
        return_id: i16, transaction_id: i32, out: *mut AEDesc,
    ) -> i32;
    fn AEPutParamPtr(ev: *mut AEDesc, kw: u32, ty: u32, data: *const c_void, sz: isize) -> i32;
    fn AESendMessage(ev: *const AEDesc, reply: *mut AEDesc, mode: i32, timeout: i32) -> i32;
}

// ── Apple Event handler ──────────────────────────────────────────────────────

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
                    if let Some(sender) = SENDER.get() {
                        let _ = sender.lock().map(|s| s.send(url));
                    }
                }
            }
        }
        AEDisposeDesc(&mut desc);
    }
    0
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Create the mpsc channel and store both ends in globals.
/// Call this on the main thread before starting the tokio daemon thread.
pub fn listen() {
    let (tx, rx) = mpsc::channel::<String>();
    SENDER.get_or_init(|| Mutex::new(tx));
    RECEIVER.get_or_init(|| Mutex::new(Some(rx)));
}

/// Take the receiver out of the global (can only succeed once).
/// Called inside `daemon::run()` on the tokio thread.
pub fn take_receiver() -> Option<mpsc::Receiver<String>> {
    RECEIVER.get()?.lock().ok()?.take()
}

/// Install the kAEGetURL handler.  Call on the main thread after NSApp setup.
pub fn install_handler() {
    unsafe {
        AEInstallEventHandler(K_INTERNET_EVENT_CLASS, K_AE_GET_URL, on_get_url, 0, false);
    }
}

/// Block the calling thread in CFRunLoopRun forever (returns only on process exit).
/// Must be called on the main thread so that AppKit delivers Apple Events here.
pub fn run_forever() {
    unsafe { CFRunLoopRun(); }
}

/// Send a synthetic kAEGetURL Apple Event to the current process.
/// Used only in tests (requires the handler to already be installed).
#[cfg(test)]
pub fn send_to_self(url: &str) {
    const K_AE_WAIT_REPLY: i32  = 3;
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
        AECreateAppleEvent(K_INTERNET_EVENT_CLASS, K_AE_GET_URL, &target, -1, 0, &mut event);
        AEPutParamPtr(&mut event, K_KEY_DIRECT_OBJECT, K_TYPE_CHAR,
                      url.as_ptr() as *const c_void, url.len() as isize);
        let mut reply = AEDesc::null();
        // kAEWaitReply: same-process → handler invoked synchronously on this call stack.
        AESendMessage(&event, &mut reply, K_AE_WAIT_REPLY, K_AE_DEFAULT_TIMEOUT);
        AEDisposeDesc(&mut reply);
        AEDisposeDesc(&mut event);
        AEDisposeDesc(&mut target);
    }
}
