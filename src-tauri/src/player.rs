//! Desktop mpv backend: spawns mpv in its own window and drives it over JSON IPC.
//! The key map comes from `companion_core::player` and is handed to mpv as an `input.conf`.
use companion_core::{
    player::{self, Event, PlayerCmd, RecentStore, ResumeStore},
    playlist::{self, Entry},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

struct Session {
    writer: Box<dyn Write + Send>,
    /// `None` for an embedded libmpv instance (stopped with the `quit` command).
    child: Option<Child>,
    id: u64,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
/// Latest value of every observed property, so a UI that opens late can load the current state.
static PROPS: Mutex<Option<HashMap<String, Value>>> = Mutex::new(None);

pub fn props_snapshot() -> Value {
    json!(PROPS.lock().unwrap().clone().unwrap_or_default())
}

fn set_prop(name: &str, v: Value) {
    PROPS.lock().unwrap().get_or_insert_with(HashMap::new).insert(name.to_string(), v);
}

/// Run a validated mpv command from the UI.
pub fn exec(args: &[Value]) -> Result<(), String> {
    player::validate_exec(args)?;
    send_raw(&player::ipc_line(args, REQ.fetch_add(1, Ordering::Relaxed)))
}

/// Set a whitelisted property from the UI.
pub fn set_property(name: &str, value: Value) -> Result<(), String> {
    if !player::SETTABLE.contains(&name) {
        return Err(format!("property not allowed: {name}"));
    }
    send_raw(&player::ipc_line(&[json!("set_property"), json!(name), value], REQ.fetch_add(1, Ordering::Relaxed)))
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static REQ: AtomicU64 = AtomicU64::new(1);

pub fn find_mpv() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "mpv.exe" } else { "mpv" };
    let mut c: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("PATH") {
        c.extend(std::env::split_paths(&p).map(|d| d.join(exe)));
    }
    // GUI apps on macOS do not inherit the shell PATH, so check the usual install locations.
    for d in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/snap/bin",
        "/Applications/mpv.app/Contents/MacOS",
        "C:\\Program Files\\mpv",
        "C:\\Program Files (x86)\\mpv",
    ] {
        c.push(Path::new(d).join(exe));
    }
    c.into_iter().find(|p| p.is_file())
}

fn ipc_path(id: u64) -> String {
    let pid = std::process::id();
    if cfg!(windows) {
        format!(r"\\.\pipe\companion-mpv-{pid}-{id}")
    } else {
        // Unix socket paths are limited to ~104 bytes, so use the short temp dir.
        std::env::temp_dir().join(format!("companion-mpv-{pid}-{id}.sock")).to_string_lossy().into_owned()
    }
}

type Conn = (Box<dyn Read + Send>, Box<dyn Write + Send>);

#[cfg(unix)]
fn connect(path: &str) -> io::Result<Conn> {
    let s = std::os::unix::net::UnixStream::connect(path)?;
    let r = s.try_clone()?;
    Ok((Box::new(r), Box::new(s)))
}

#[cfg(windows)]
fn connect(path: &str) -> io::Result<Conn> {
    let f = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    let r = f.try_clone()?;
    Ok((Box::new(r), Box::new(f)))
}

fn send_raw(line: &str) -> Result<(), String> {
    let mut g = SESSION.lock().unwrap();
    let s = g.as_mut().ok_or("the player is not running")?;
    s.writer.write_all(line.as_bytes()).and_then(|_| s.writer.flush()).map_err(|e| e.to_string())
}

pub fn command(c: &PlayerCmd) -> Result<(), String> {
    send_raw(&player::ipc_line(&c.to_mpv(), REQ.fetch_add(1, Ordering::Relaxed)))
}

pub fn close() {
    if let Some(mut s) = SESSION.lock().unwrap().take() {
        match s.child.as_mut() {
            Some(c) => {
                let _ = c.kill();
                let _ = c.wait();
            }
            None => {
                let _ = s.writer.write_all(player::ipc_line(&PlayerCmd::Stop.to_mpv(), 0).as_bytes());
            }
        }
    }
}

fn prepare(dir: &Path, entries: &[Entry]) -> Result<(PathBuf, PathBuf), String> {
    let pdir = dir.join("player");
    std::fs::create_dir_all(&pdir).map_err(|e| e.to_string())?;
    let conf = pdir.join("input.conf");
    let queue = pdir.join("queue.m3u8");
    std::fs::write(&conf, player::input_conf()).map_err(|e| e.to_string())?;
    std::fs::write(&queue, playlist::build_m3u8(entries)).map_err(|e| e.to_string())?;
    Ok((conf, queue))
}

/// Connect to mpv's socket, start observing properties and spawn the reader thread.
fn attach(app: &AppHandle, dir: &Path, sock: &str, id: u64, mut child: Option<Child>) -> Result<(), String> {
    let mut conn = None;
    for _ in 0..50 {
        if let Some(c) = child.as_mut() {
            if let Ok(Some(st)) = c.try_wait() {
                return Err(format!("mpv exited immediately ({st})"));
            }
        }
        if let Ok(c) = connect(sock) {
            conn = Some(c);
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let Some((reader, mut writer)) = conn else {
        if let Some(c) = child.as_mut() {
            let _ = c.kill();
        }
        return Err("could not connect to mpv's control socket".into());
    };
    for (oid, prop) in [
        (player::OBS_TIME, "time-pos"),
        (player::OBS_PATH, "path"),
        (player::OBS_DURATION, "duration"),
        (player::OBS_PAUSE, "pause"),
    ] {
        writer.write_all(player::observe_line(oid, prop).as_bytes()).map_err(|e| e.to_string())?;
    }
    for i in 0..player::OBSERVED.len() {
        if let Some(l) = player::observe_generic_line(i) {
            writer.write_all(l.as_bytes()).map_err(|e| e.to_string())?;
        }
    }
    *PROPS.lock().unwrap() = Some(HashMap::new());
    *SESSION.lock().unwrap() = Some(Session { writer, child, id });

    let app = app.clone();
    let dir = dir.to_path_buf();
    thread::spawn(move || run_reader(app, reader, dir, id));
    Ok(())
}

/// External mpv window (fallback when libmpv or an embedding surface is unavailable).
pub fn open(app: &AppHandle, dir: &Path, entries: &[Entry], index: usize) -> Result<(), String> {
    close();
    let mpv = find_mpv().ok_or("mpv was not found. Install it (macOS: brew install mpv) and try again")?;
    let (conf, queue) = prepare(dir, entries)?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let sock = ipc_path(id);
    #[cfg(unix)]
    let _ = std::fs::remove_file(&sock);

    let child = Command::new(mpv)
        .args([
            "--no-terminal",
            "--force-window=yes",
            "--idle=no",
            "--keep-open=no",
            "--cache=yes",
            "--demuxer-max-bytes=256MiB",
            "--demuxer-readahead-secs=90",
            "--stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_delay_max=5",
        ])
        .arg(format!("--input-ipc-server={sock}"))
        .arg(format!("--input-conf={}", conf.display()))
        .arg(format!("--playlist-start={index}"))
        .arg(format!("--playlist={}", queue.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start mpv: {e}"))?;
    attach(app, dir, &sock, id, Some(child))
}

/// libmpv in this process (its video window is re-parented under our player window by
/// `embed`), controlled from the webview overlay. The mpv OSC, default key bindings and keyboard
/// input are disabled because the overlay supplies all of them.
pub fn open_embedded(app: &AppHandle, dir: &Path, entries: &[Entry], index: usize) -> Result<(), String> {
    close();
    let lib = crate::mpvlib::find_libmpv().ok_or("libmpv was not found (macOS: brew install mpv)")?;
    let (_conf, queue) = prepare(dir, entries)?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let sock = ipc_path(id);
    #[cfg(unix)]
    let _ = std::fs::remove_file(&sock);
    let opts: Vec<(&str, String)> = vec![
        ("input-ipc-server", sock.clone()),
        ("force-window", "yes".into()),
        ("border", "no".into()),
        ("window-dragging", "no".into()),
        ("auto-window-resize", "no".into()),
        ("keepaspect-window", "no".into()),
        ("idle", "yes".into()),
        ("keep-open", "no".into()),
        ("osc", "no".into()),
        ("osd-level", "0".into()),
        ("input-default-bindings", "no".into()),
        ("input-vo-keyboard", "no".into()),
        ("input-cursor", "no".into()),
        ("cursor-autohide", "no".into()),
        ("hwdec", "auto-safe".into()),
        ("cache", "yes".into()),
        ("demuxer-max-bytes", "256MiB".into()),
        ("demuxer-readahead-secs", "90".into()),
        ("stream-lavf-o", "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5".into()),
    ];
    crate::mpvlib::start_embedded(&lib, &opts, &queue, index)?;
    attach(app, dir, &sock, id, None)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn run_reader(app: AppHandle, reader: Box<dyn Read + Send>, dir: PathBuf, id: u64) {
    let resume_file = dir.join("resume.json");
    let recent_file = dir.join("recent.json");
    let mut store = ResumeStore::load(&resume_file);
    let mut recent = RecentStore::load(&recent_file);
    let (mut path, mut pos, mut dur, mut paused) = (String::new(), 0.0_f64, None::<f64>, false);
    let (mut last_save, mut last_emit, mut last_state) = (Instant::now(), Instant::now(), Instant::now());
    let title_of = |path: &str| -> String {
        let t = PROPS.lock().unwrap().as_ref().and_then(|m| m.get("media-title").and_then(Value::as_str).map(String::from));
        t.filter(|s| !s.is_empty()).unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_string())
    };
    let emit = |name: &str, v: &Value| {
        let _ = app.emit("player://prop", json!({"name": name, "value": v}));
    };

    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else { break };
        match player::parse_event(&line) {
            Event::Time(t) => {
                pos = t;
                if last_save.elapsed() >= Duration::from_secs(5) && !path.is_empty() {
                    store.set(&path, pos, dur);
                    let _ = store.save(&resume_file);
                    recent.touch(&title_of(&path), &path, pos, dur.unwrap_or(0.0), unix_now());
                    let _ = recent.save(&recent_file);
                    last_save = Instant::now();
                }
                if last_emit.elapsed() >= Duration::from_millis(100) {
                    set_prop("time-pos", json!(pos));
                    emit("time-pos", &json!(pos));
                    last_emit = Instant::now();
                }
                if last_state.elapsed() >= Duration::from_millis(1000) {
                    let _ = app.emit("player://state", json!({"pos": pos, "duration": dur, "path": path, "paused": paused}));
                    last_state = Instant::now();
                }
            }
            Event::Duration(d) => {
                dur = Some(d);
                set_prop("duration", json!(d));
                emit("duration", &json!(d));
            }
            Event::Pause(p) => {
                paused = p;
                set_prop("pause", json!(p));
                emit("pause", &json!(p));
            }
            Event::Path(p) => {
                // The previous file is done: store where it stopped before switching.
                if !path.is_empty() {
                    store.set(&path, pos, dur);
                    let _ = store.save(&resume_file);
                    recent.touch(&title_of(&path), &path, pos, dur.unwrap_or(0.0), unix_now());
                    let _ = recent.save(&recent_file);
                }
                path = p;
                pos = 0.0;
                dur = None;
                set_prop("path", json!(path));
                set_prop("time-pos", json!(0.0));
                emit("path", &json!(path));
            }
            Event::FileLoaded => {
                if let Some(p) = store.get(&path) {
                    let _ = command(&PlayerCmd::SeekAbs((p - 2.0).max(0.0)));
                }
                let p0 = store.get(&path).unwrap_or(0.0);
                recent.touch(&title_of(&path), &path, p0, 0.0, unix_now());
                let _ = recent.save(&recent_file);
                emit("file-loaded", &Value::Null);
            }
            Event::Prop(name, v) => {
                set_prop(&name, v.clone());
                emit(&name, &v);
            }
            Event::EndFile(_) | Event::Other => {}
        }
    }

    if !path.is_empty() {
        store.set(&path, pos, dur);
        let _ = store.save(&resume_file);
        recent.touch(&title_of(&path), &path, pos, dur.unwrap_or(0.0), unix_now());
        let _ = recent.save(&recent_file);
    }
    // Clear the session only if it is still ours (a newer one may have replaced it).
    let mut g = SESSION.lock().unwrap();
    let ours = g.as_ref().map(|s| s.id) == Some(id) || g.is_none();
    if g.as_ref().map(|s| s.id) == Some(id) {
        if let Some(mut s) = g.take() {
            if let Some(c) = s.child.as_mut() {
                let _ = c.wait();
            }
        }
    }
    drop(g);
    // A newer session may already be playing: only the latest one reports "closed".
    if ours {
        let _ = app.emit("player://closed", Value::Null);
    }
}
