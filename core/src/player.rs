//! Player model: command set, IINA-style key map, mpv JSON-IPC encoding and resume store.
//!
//! Pure logic with no mpv dependency, so it is unit-testable everywhere. The key map is a
//! clean-room table of IINA's documented default bindings, written from the spec.
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::HashMap, fs, io, path::Path};

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerCmd {
    TogglePause,
    Seek(f64),
    SeekAbs(f64),
    VolumeAdd(f64),
    SetVolume(f64),
    ToggleMute,
    SpeedMul(f64),
    SpeedReset,
    Fullscreen,
    FrameStep,
    FrameBack,
    AbLoop,
    CycleSub,
    CycleAudio,
    SubDelay(f64),
    AudioDelay(f64),
    Screenshot,
    Next,
    Prev,
    ChapterNext,
    ChapterPrev,
    Stats,
    Stop,
}

impl PlayerCmd {
    /// Build a command from the names the UI sends over `player_cmd`.
    pub fn from_name(name: &str, arg: Option<f64>) -> Option<Self> {
        use PlayerCmd::*;
        let a = arg.unwrap_or(0.0);
        Some(match name {
            "toggle_pause" => TogglePause,
            "seek" => Seek(a),
            "seek_abs" => SeekAbs(a),
            "volume_add" => VolumeAdd(a),
            "set_volume" => SetVolume(a),
            "toggle_mute" => ToggleMute,
            "speed_mul" => SpeedMul(if arg.is_some() { a } else { 1.0 }),
            "speed_reset" => SpeedReset,
            "fullscreen" => Fullscreen,
            "frame_step" => FrameStep,
            "frame_back" => FrameBack,
            "ab_loop" => AbLoop,
            "cycle_sub" => CycleSub,
            "cycle_audio" => CycleAudio,
            "sub_delay" => SubDelay(a),
            "audio_delay" => AudioDelay(a),
            "screenshot" => Screenshot,
            "next" => Next,
            "prev" => Prev,
            "chapter_next" => ChapterNext,
            "chapter_prev" => ChapterPrev,
            "stats" => Stats,
            "stop" => Stop,
            _ => return None,
        })
    }

    /// The mpv command as a JSON-IPC array (numbers stay numbers).
    pub fn to_mpv(&self) -> Vec<Value> {
        use PlayerCmd::*;
        match self {
            TogglePause => vec![json!("cycle"), json!("pause")],
            Seek(s) => vec![json!("seek"), json!(s)],
            SeekAbs(s) => vec![json!("seek"), json!(s), json!("absolute")],
            VolumeAdd(v) => vec![json!("add"), json!("volume"), json!(v)],
            SetVolume(v) => vec![json!("set"), json!("volume"), json!(v)],
            ToggleMute => vec![json!("cycle"), json!("mute")],
            SpeedMul(f) => vec![json!("multiply"), json!("speed"), json!(f)],
            SpeedReset => vec![json!("set"), json!("speed"), json!(1.0)],
            Fullscreen => vec![json!("cycle"), json!("fullscreen")],
            FrameStep => vec![json!("frame-step")],
            FrameBack => vec![json!("frame-back-step")],
            AbLoop => vec![json!("ab-loop")],
            CycleSub => vec![json!("cycle"), json!("sub")],
            CycleAudio => vec![json!("cycle"), json!("audio")],
            SubDelay(d) => vec![json!("add"), json!("sub-delay"), json!(d)],
            AudioDelay(d) => vec![json!("add"), json!("audio-delay"), json!(d)],
            Screenshot => vec![json!("screenshot")],
            Next => vec![json!("playlist-next")],
            Prev => vec![json!("playlist-prev")],
            ChapterNext => vec![json!("add"), json!("chapter"), json!(1)],
            ChapterPrev => vec![json!("add"), json!("chapter"), json!(-1)],
            Stats => vec![json!("script-binding"), json!("stats/display-stats-toggle")],
            Stop => vec![json!("quit")],
        }
    }

    /// The same command in `input.conf` text form, e.g. `add volume 5`.
    pub fn to_text(&self) -> String {
        self.to_mpv()
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// One keyboard binding. `key` uses mpv key names (`SPACE`, `Shift+LEFT`, `Meta+s`...).
pub struct Binding {
    pub key: &'static str,
    pub cmd: PlayerCmd,
    pub desc: &'static str,
}

/// IINA-style default key map. `Meta` is Cmd on macOS.
pub fn bindings() -> Vec<Binding> {
    use PlayerCmd::*;
    let b = |key, cmd, desc| Binding { key, cmd, desc };
    vec![
        b("SPACE", TogglePause, "Play / pause"),
        b("LEFT", Seek(-5.0), "Back 5 s"),
        b("RIGHT", Seek(5.0), "Forward 5 s"),
        b("Shift+LEFT", Seek(-60.0), "Back 60 s"),
        b("Shift+RIGHT", Seek(60.0), "Forward 60 s"),
        b("UP", VolumeAdd(5.0), "Volume up"),
        b("DOWN", VolumeAdd(-5.0), "Volume down"),
        b("m", ToggleMute, "Mute"),
        b("[", SpeedMul(0.9091), "Slower"),
        b("]", SpeedMul(1.1), "Faster"),
        b("BS", SpeedReset, "Reset speed"),
        b("f", Fullscreen, "Fullscreen"),
        b("ENTER", Fullscreen, "Fullscreen"),
        b(".", FrameStep, "Next frame"),
        b(",", FrameBack, "Previous frame"),
        b("l", AbLoop, "A-B loop"),
        b("s", CycleSub, "Next subtitle track"),
        b("A", CycleAudio, "Next audio track"),
        b("z", SubDelay(-0.1), "Subtitle earlier"),
        b("x", SubDelay(0.1), "Subtitle later"),
        b("Ctrl+=", AudioDelay(0.1), "Audio later"),
        b("Ctrl+-", AudioDelay(-0.1), "Audio earlier"),
        b("Ctrl+s", Screenshot, "Screenshot"),
        b("Meta+s", Screenshot, "Screenshot"),
        b(">", Next, "Next in playlist"),
        b("<", Prev, "Previous in playlist"),
        b("PGDWN", Next, "Next in playlist"),
        b("PGUP", Prev, "Previous in playlist"),
        b("Meta+RIGHT", ChapterNext, "Next chapter"),
        b("Meta+LEFT", ChapterPrev, "Previous chapter"),
        b("i", Stats, "Playback stats"),
        b("Meta+w", Stop, "Close player"),
        b("q", Stop, "Close player"),
    ]
}

/// `input.conf` text generated from the key map, passed to mpv with `--input-conf`.
pub fn input_conf() -> String {
    let mut s = String::from("# generated from companion-core player::bindings()\n");
    for b in bindings() {
        s.push_str(&format!("{} {}\n", b.key, b.cmd.to_text()));
    }
    s
}

#[derive(Serialize)]
pub struct KeyInfo {
    pub key: String,
    pub desc: String,
}

/// Key list for a help overlay in the UI.
pub fn key_list() -> Vec<KeyInfo> {
    bindings().into_iter().map(|b| KeyInfo { key: b.key.to_string(), desc: b.desc.to_string() }).collect()
}

// ---------------------------------------------------------------- mpv JSON IPC
/// One newline-terminated IPC request.
pub fn ipc_line(cmd: &[Value], request_id: u64) -> String {
    format!("{}\n", json!({ "command": cmd, "request_id": request_id }))
}

pub fn observe_line(id: u64, prop: &str) -> String {
    ipc_line(&[json!("observe_property"), json!(id), json!(prop)], 1000 + id)
}

pub const OBS_TIME: u64 = 1;
pub const OBS_PATH: u64 = 2;
pub const OBS_DURATION: u64 = 3;
pub const OBS_PAUSE: u64 = 4;

#[derive(Debug, PartialEq)]
pub enum Event {
    Time(f64),
    Duration(f64),
    Path(String),
    Pause(bool),
    FileLoaded,
    EndFile(String),
    Other,
}

pub fn parse_event(line: &str) -> Event {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Event::Other };
    match v.get("event").and_then(Value::as_str) {
        Some("file-loaded") => Event::FileLoaded,
        Some("end-file") => Event::EndFile(v.get("reason").and_then(Value::as_str).unwrap_or("").to_string()),
        Some("property-change") => {
            let data = v.get("data");
            match v.get("id").and_then(Value::as_u64) {
                Some(OBS_TIME) => data.and_then(Value::as_f64).map(Event::Time).unwrap_or(Event::Other),
                Some(OBS_DURATION) => data.and_then(Value::as_f64).map(Event::Duration).unwrap_or(Event::Other),
                Some(OBS_PATH) => data.and_then(Value::as_str).map(|s| Event::Path(s.to_string())).unwrap_or(Event::Other),
                Some(OBS_PAUSE) => data.and_then(Value::as_bool).map(Event::Pause).unwrap_or(Event::Other),
                _ => Event::Other,
            }
        }
        _ => Event::Other,
    }
}

// ---------------------------------------------------------------- resume positions
/// Stable key for a URL: query string and fragment are dropped because TorBox links carry
/// expiring tokens. FNV-1a, hex.
pub fn resume_key(url: &str) -> String {
    let base = url.split(['?', '#']).next().unwrap_or(url);
    let mut h: u64 = 0xcbf29ce484222325;
    for b in base.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[derive(Default)]
pub struct ResumeStore {
    map: HashMap<String, f64>,
}

impl ResumeStore {
    pub fn load(path: &Path) -> Self {
        let map = fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Self { map }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(d) = path.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(path, serde_json::to_string(&self.map).unwrap_or_default())
    }

    pub fn get(&self, url: &str) -> Option<f64> {
        self.map.get(&resume_key(url)).copied()
    }

    /// Remember a position. Near the start or the end (>= 95%) the entry is cleared instead.
    pub fn set(&mut self, url: &str, pos: f64, duration: Option<f64>) {
        let k = resume_key(url);
        let finished = duration.map(|d| d > 0.0 && pos >= d * 0.95).unwrap_or(false);
        if pos < 10.0 || finished {
            self.map.remove(&k);
        } else {
            self.map.insert(k, pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn keys_are_unique_and_commands_nonempty() {
        let mut seen = HashSet::new();
        for b in bindings() {
            assert!(seen.insert(b.key), "duplicate key {}", b.key);
            assert!(!b.cmd.to_mpv().is_empty());
        }
    }

    #[test]
    fn text_and_json_forms() {
        assert_eq!(PlayerCmd::VolumeAdd(5.0).to_text(), "add volume 5.0");
        assert_eq!(PlayerCmd::Seek(-5.0).to_text(), "seek -5.0");
        assert_eq!(PlayerCmd::TogglePause.to_text(), "cycle pause");
        assert_eq!(PlayerCmd::ChapterPrev.to_text(), "add chapter -1");
        assert_eq!(PlayerCmd::SeekAbs(30.0).to_mpv(), vec![json!("seek"), json!(30.0), json!("absolute")]);
    }

    #[test]
    fn input_conf_has_one_line_per_binding() {
        let c = input_conf();
        assert_eq!(c.lines().filter(|l| !l.starts_with('#')).count(), bindings().len());
        assert!(c.lines().any(|l| l == "SPACE cycle pause"));
        assert!(c.lines().any(|l| l == "Meta+RIGHT add chapter 1"));
    }

    #[test]
    fn from_name_round_trip() {
        assert_eq!(PlayerCmd::from_name("seek", Some(10.0)), Some(PlayerCmd::Seek(10.0)));
        assert_eq!(PlayerCmd::from_name("stop", None), Some(PlayerCmd::Stop));
        assert_eq!(PlayerCmd::from_name("rm_rf", None), None);
    }

    #[test]
    fn ipc_encoding() {
        let l = ipc_line(&PlayerCmd::TogglePause.to_mpv(), 7);
        assert!(l.ends_with('\n'));
        let v: Value = serde_json::from_str(l.trim()).unwrap();
        assert_eq!(v["command"], json!(["cycle", "pause"]));
        assert_eq!(v["request_id"], 7);
    }

    #[test]
    fn event_parsing() {
        assert_eq!(parse_event(r#"{"event":"property-change","id":1,"name":"time-pos","data":12.5}"#), Event::Time(12.5));
        assert_eq!(parse_event(r#"{"event":"property-change","id":2,"name":"path","data":"http://x/a.mkv"}"#), Event::Path("http://x/a.mkv".into()));
        assert_eq!(parse_event(r#"{"event":"property-change","id":4,"name":"pause","data":true}"#), Event::Pause(true));
        assert_eq!(parse_event(r#"{"event":"property-change","id":1,"name":"time-pos"}"#), Event::Other);
        assert_eq!(parse_event(r#"{"event":"file-loaded"}"#), Event::FileLoaded);
        assert_eq!(parse_event(r#"{"event":"end-file","reason":"eof"}"#), Event::EndFile("eof".into()));
        assert_eq!(parse_event("garbage"), Event::Other);
    }

    #[test]
    fn resume_ignores_query_and_clears_near_end() {
        let mut r = ResumeStore::default();
        r.set("https://s/v/a.mkv?token=1", 300.0, Some(3600.0));
        assert_eq!(r.get("https://s/v/a.mkv?token=2"), Some(300.0));
        r.set("https://s/v/a.mkv", 3500.0, Some(3600.0));
        assert_eq!(r.get("https://s/v/a.mkv"), None);
        r.set("https://s/v/b.mkv", 4.0, None);
        assert_eq!(r.get("https://s/v/b.mkv"), None);
    }

    #[test]
    fn resume_persists() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub/resume.json");
        let mut r = ResumeStore::default();
        r.set("https://s/c.mkv", 120.0, Some(1000.0));
        r.save(&p).unwrap();
        assert_eq!(ResumeStore::load(&p).get("https://s/c.mkv"), Some(120.0));
        assert_eq!(ResumeStore::load(&d.path().join("missing.json")).get("x"), None);
    }
}
