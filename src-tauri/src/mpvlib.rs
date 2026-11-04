//! Minimal libmpv wrapper, loaded at run time with dlopen (no link-time dependency, so the app
//! still starts without libmpv and the LGPL library stays a separate, replaceable file).
//! Control and property observation go through mpv's JSON IPC socket, so this only needs to
//! create the instance, set options, start playback and drain events.
use libloading::Library;
use std::{
    ffi::{c_char, c_double, c_int, c_void, CStr, CString},
    path::{Path, PathBuf},
    thread,
};

type Create = unsafe extern "C" fn() -> *mut c_void;
type Init = unsafe extern "C" fn(*mut c_void) -> c_int;
type SetOpt = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type Command = unsafe extern "C" fn(*mut c_void, *mut *const c_char) -> c_int;
type WaitEvent = unsafe extern "C" fn(*mut c_void, c_double) -> *const Event;
type Destroy = unsafe extern "C" fn(*mut c_void);
type ErrStr = unsafe extern "C" fn(c_int) -> *const c_char;

#[repr(C)]
struct Event {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}
const EVENT_SHUTDOWN: c_int = 1;

pub fn find_libmpv() -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // macOS bundle: Contents/MacOS/<exe> -> Contents/Frameworks
            c.push(dir.join("../Frameworks/libmpv.2.dylib"));
            c.push(dir.join("libmpv.2.dylib"));
            c.push(dir.join("libmpv-2.dll"));
            c.push(dir.join("libmpv.so.2"));
        }
    }
    for d in ["/opt/homebrew/lib", "/usr/local/lib", "/usr/lib", "/usr/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu"] {
        for f in ["libmpv.2.dylib", "libmpv.dylib", "libmpv.so.2", "libmpv.so"] {
            c.push(Path::new(d).join(f));
        }
    }
    c.into_iter().find(|p| p.is_file())
}

struct Mpv {
    _lib: Library,
    h: *mut c_void,
    set_opt: SetOpt,
    init: Init,
    command: Command,
    wait: WaitEvent,
    destroy: Destroy,
    err: ErrStr,
}
// The handle is only ever used from the thread that owns the Mpv.
unsafe impl Send for Mpv {}

impl Mpv {
    fn load(path: &Path) -> Result<Self, String> {
        unsafe {
            let lib = Library::new(path).map_err(|e| format!("cannot load libmpv: {e}"))?;
            macro_rules! sym {
                ($n:literal, $t:ty) => {
                    *lib.get::<$t>(concat!($n, "\0").as_bytes()).map_err(|e| format!("libmpv is missing {}: {e}", $n))?
                };
            }
            let create: Create = sym!("mpv_create", Create);
            let init: Init = sym!("mpv_initialize", Init);
            let set_opt: SetOpt = sym!("mpv_set_option_string", SetOpt);
            let command: Command = sym!("mpv_command", Command);
            let wait: WaitEvent = sym!("mpv_wait_event", WaitEvent);
            let destroy: Destroy = sym!("mpv_terminate_destroy", Destroy);
            let err: ErrStr = sym!("mpv_error_string", ErrStr);
            let h = create();
            if h.is_null() {
                return Err("mpv_create failed".into());
            }
            Ok(Self { _lib: lib, h, set_opt, init, command, wait, destroy, err })
        }
    }

    fn msg(&self, code: c_int) -> String {
        unsafe { CStr::from_ptr((self.err)(code)).to_string_lossy().into_owned() }
    }

    fn option(&self, k: &str, v: &str) -> Result<(), String> {
        let (ck, cv) = (CString::new(k).map_err(|e| e.to_string())?, CString::new(v).map_err(|e| e.to_string())?);
        let r = unsafe { (self.set_opt)(self.h, ck.as_ptr(), cv.as_ptr()) };
        if r < 0 { Err(format!("option {k}={v}: {}", self.msg(r))) } else { Ok(()) }
    }

    fn initialize(&self) -> Result<(), String> {
        let r = unsafe { (self.init)(self.h) };
        if r < 0 { Err(format!("mpv_initialize: {}", self.msg(r))) } else { Ok(()) }
    }

    fn cmd(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| CString::new(*a).map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        let r = unsafe { (self.command)(self.h, ptrs.as_mut_ptr()) };
        if r < 0 { Err(format!("mpv command {args:?}: {}", self.msg(r))) } else { Ok(()) }
    }
}

/// Create an mpv instance in this process (its own video window; the caller re-parents it).
/// Returns once playback has been requested; a background thread owns the instance, drains its
/// events and destroys it after shutdown (send `quit` over the IPC socket to stop it).
pub fn start_embedded(lib: &Path, options: &[(&str, String)], playlist: &Path, start_index: usize) -> Result<(), String> {
    let mpv = Mpv::load(lib)?;
    for (k, v) in options {
        mpv.option(k, v)?;
    }
    mpv.option("playlist-start", &start_index.to_string())?;
    mpv.initialize()?;
    mpv.cmd(&["loadlist", &playlist.to_string_lossy(), "replace"])?;
    thread::Builder::new()
        .name("mpv-events".into())
        .spawn(move || {
            let mpv = mpv; // capture the whole (Send) struct, not just the raw handle
            loop {
                let ev = unsafe { (mpv.wait)(mpv.h, 1.0) };
                if !ev.is_null() && unsafe { (*ev).event_id } == EVENT_SHUTDOWN {
                    break;
                }
            }
            unsafe { (mpv.destroy)(mpv.h) };
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
