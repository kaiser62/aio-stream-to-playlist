//! Integration test against a real mpv (headless). Skipped when mpv is not installed or on non-unix.
#![cfg(unix)]
use companion_core::player::{self, parse_event, Event, PlayerCmd};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn find_mpv() -> Option<PathBuf> {
    ["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv", "/usr/bin/mpv"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .or_else(|| {
            std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join("mpv")).find(|f| f.is_file()))
        })
}

#[test]
fn generated_keymap_drives_real_mpv() {
    let Some(mpv) = find_mpv() else {
        eprintln!("mpv not installed, skipping");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let conf = dir.path().join("input.conf");
    std::fs::write(&conf, player::input_conf()).unwrap();
    let sock = std::env::temp_dir().join(format!("companion-test-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock);

    let mut child = Command::new(mpv)
        .args(["--idle=yes", "--vo=null", "--ao=null", "--no-terminal", "--force-window=no", "--no-config"])
        .arg(format!("--input-ipc-server={}", sock.display()))
        .arg(format!("--input-conf={}", conf.display()))
        .arg("av://lavfi:testsrc=duration=30:size=64x64:rate=10")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut stream = None;
    for _ in 0..50 {
        if let Ok(s) = UnixStream::connect(&sock) {
            stream = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut w = stream.expect("could not connect to mpv socket");
    w.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    for (id, prop) in [(player::OBS_PAUSE, "pause"), (player::OBS_TIME, "time-pos")] {
        w.write_all(player::observe_line(id, prop).as_bytes()).unwrap();
    }
    let mut lines = BufReader::new(w.try_clone().unwrap()).lines();

    // Wait for playback to start (time-pos event), then press SPACE through mpv's key bindings.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut saw_time = false;
    while Instant::now() < deadline {
        let Some(Ok(l)) = lines.next() else { break };
        if matches!(parse_event(&l), Event::Time(_)) {
            saw_time = true;
            break;
        }
    }
    assert!(saw_time, "no time-pos event from mpv");

    w.write_all(format!("{}\n", json!({"command": ["keypress", "SPACE"]})).as_bytes()).unwrap();
    let mut paused = false;
    while Instant::now() < deadline {
        let Some(Ok(l)) = lines.next() else { break };
        if parse_event(&l) == Event::Pause(true) {
            paused = true;
            break;
        }
    }
    assert!(paused, "SPACE from generated input.conf did not pause mpv");

    // Direct IPC command also works.
    w.write_all(player::ipc_line(&PlayerCmd::TogglePause.to_mpv(), 9).as_bytes()).unwrap();
    let mut resumed = false;
    while Instant::now() < deadline {
        let Some(Ok(l)) = lines.next() else { break };
        if parse_event(&l) == Event::Pause(false) {
            resumed = true;
            break;
        }
    }
    assert!(resumed, "TogglePause over IPC did not resume");

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&sock);
}
