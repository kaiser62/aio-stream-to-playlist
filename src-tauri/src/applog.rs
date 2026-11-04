//! In-memory app log (ring buffer) shown in the Logs tab. No file or network output; URLs are reduced to
//! their host so manifest URLs / tokens never end up in it.
use serde::Serialize;
use std::{collections::VecDeque, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};

const CAP: usize = 1500;

#[derive(Clone, Serialize)]
pub struct Line {
    pub id: u64,
    pub ts: u64,
    pub level: String,
    pub msg: String,
}

static LOGS: Mutex<VecDeque<Line>> = Mutex::new(VecDeque::new());
static SEQ: Mutex<u64> = Mutex::new(0);

/// Keep only scheme://host for anything that looks like a URL.
pub fn redact(s: &str) -> String {
    s.split(' ')
        .map(|t| {
            let lt = t.trim_start_matches(|c: char| "([\"'<".contains(c));
            if lt.starts_with("http://") || lt.starts_with("https://") || lt.starts_with("stremio://") {
                let pre = &t[..t.len() - lt.len()];
                let rest = lt.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
                format!("{pre}{rest}/…")
            } else {
                t.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn push(level: &str, msg: &str) {
    let msg = redact(msg);
    if level == "error" || level == "warn" {
        eprintln!("[{level}] {msg}");
    }
    let id = {
        let mut s = SEQ.lock().unwrap();
        *s += 1;
        *s
    };
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let mut q = LOGS.lock().unwrap();
    q.push_back(Line { id, ts, level: level.into(), msg });
    while q.len() > CAP {
        q.pop_front();
    }
}

pub fn since(after: u64) -> Vec<Line> {
    LOGS.lock().unwrap().iter().filter(|l| l.id > after).cloned().collect()
}

pub fn clear() {
    LOGS.lock().unwrap().clear();
}

#[macro_export]
macro_rules! alog {
    ($lvl:expr, $($arg:tt)*) => { $crate::applog::push($lvl, &format!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::redact;
    #[test]
    fn urls_are_reduced_to_host() {
        assert_eq!(redact("GET https://addon.example/abc123secret/manifest.json failed"), "GET https://addon.example/… failed");
        assert_eq!(redact("no url here"), "no url here");
    }
}
