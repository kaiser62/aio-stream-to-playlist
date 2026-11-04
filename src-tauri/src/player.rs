//! Desktop mpv backend: spawns mpv in its own window and drives it over JSON IPC.
//! The key map comes from `companion_core::player` and is handed to mpv as an `input.conf`.
use companion_core::{
    player::{self, Event, PlayerCmd, ResumeStore},
    playlist::{self, Entry},
};
use serde_json::{json, Value};
use std::{
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
    child: Child,
    id: u64,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
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
        let _ = s.child.kill();
        let _ = s.child.wait();
    }
}

pub fn open(app: &AppHandle, dir: &Path, entries: &[Entry], index: usize) -> Result<(), String> {
    close();
    let mpv = find_mpv().ok_or("mpv was not found. Install it (macOS: brew install mpv) and try again")?;
    let pdir = dir.join("player");
    std::fs::create_dir_all(&pdir).map_err(|e| e.to_string())?;
    let conf = pdir.join("input.conf");
    let queue = pdir.join("queue.m3u8");
    std::fs::write(&conf, player::input_conf()).map_err(|e| e.to_string())?;
    std::fs::write(&queue, playlist::build_m3u8(entries)).map_err(|e| e.to_string())?;

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let sock = ipc_path(id);
    #[cfg(unix)]
    let _ = std::fs::remove_file(&sock);

    let mut child = Command::new(mpv)
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

    let mut conn = None;
    for _ in 0..50 {
        if let Ok(Some(st)) = child.try_wait() {
            return Err(format!("mpv exited immediately ({st})"));
        }
        if let Ok(c) = connect(&sock) {
            conn = Some(c);
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let Some((reader, mut writer)) = conn else {
        let _ = child.kill();
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
    *SESSION.lock().unwrap() = Some(Session { writer, child, id });

    let app = app.clone();
    let resume_file = dir.join("resume.json");
    thread::spawn(move || run_reader(app, reader, resume_file, id));
    Ok(())
}

fn run_reader(app: AppHandle, reader: Box<dyn Read + Send>, resume_file: PathBuf, id: u64) {
    let mut store = ResumeStore::load(&resume_file);
    let (mut path, mut pos, mut dur, mut paused) = (String::new(), 0.0_f64, None::<f64>, false);
    let (mut last_save, mut last_emit) = (Instant::now(), Instant::now());
    let flush = |store: &ResumeStore| {
        let _ = store.save(&resume_file);
    };

    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else { break };
        match player::parse_event(&line) {
            Event::Time(t) => {
                pos = t;
                if last_save.elapsed() >= Duration::from_secs(5) && !path.is_empty() {
                    store.set(&path, pos, dur);
                    flush(&store);
                    last_save = Instant::now();
                }
                if last_emit.elapsed() >= Duration::from_millis(1000) {
                    let _ = app.emit("player://state", json!({"pos": pos, "duration": dur, "path": path, "paused": paused}));
                    last_emit = Instant::now();
                }
            }
            Event::Duration(d) => dur = Some(d),
            Event::Pause(p) => paused = p,
            Event::Path(p) => {
                // The previous file is done: store where it stopped before switching.
                if !path.is_empty() {
                    store.set(&path, pos, dur);
                    flush(&store);
                }
                path = p;
                pos = 0.0;
                dur = None;
            }
            Event::FileLoaded => {
                if let Some(p) = store.get(&path) {
                    let _ = command(&PlayerCmd::SeekAbs((p - 2.0).max(0.0)));
                }
            }
            Event::EndFile(_) | Event::Other => {}
        }
    }

    if !path.is_empty() {
        store.set(&path, pos, dur);
        flush(&store);
    }
    // Clear the session only if it is still ours (a newer one may have replaced it).
    let mut g = SESSION.lock().unwrap();
    if g.as_ref().map(|s| s.id) == Some(id) {
        if let Some(mut s) = g.take() {
            let _ = s.child.wait();
        }
    }
    drop(g);
    let _ = app.emit("player://closed", Value::Null);
}
