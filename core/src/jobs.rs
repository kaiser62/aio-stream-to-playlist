//! "Send to TorBox" jobs: add magnets, wait until ready, resolve links, write the playlist.
use crate::parse::{is_video, pick_file, TbFile};
use crate::playlist::{self, Entry};
use crate::torbox::{is_ready, TorBox};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::{Duration, Instant}};

#[derive(Serialize, Clone, Debug, Default)]
pub struct Row {
    pub title: String,
    pub state: String,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Job {
    pub rows: Vec<Row>,
    pub finished: bool,
    pub playlist: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SendItem {
    pub title: String,
    pub magnet: Option<String>,
    pub url: Option<String>,
    pub torrent_id: Option<i64>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    #[serde(default)]
    pub add_only: bool,
    #[serde(default)]
    pub all_files: bool,
}

pub struct Opts {
    pub poll: Duration,
    pub timeout: Duration,
}
impl Default for Opts {
    fn default() -> Self {
        Self { poll: Duration::from_secs(4), timeout: Duration::from_secs(600) }
    }
}

type Upd<'a> = &'a (dyn Fn(&Job) + Send + Sync);

fn set(job: &mut Job, i: usize, state: &str, detail: &str, upd: Upd) {
    job.rows[i].state = state.into();
    job.rows[i].detail = detail.into();
    upd(job);
}

async fn resolve(tb: &TorBox, it: &SendItem, job: &mut Job, i: usize, o: &Opts, upd: Upd<'_>) -> Result<Vec<Entry>, String> {
    if let (Some(u), None, None) = (&it.url, &it.magnet, it.torrent_id) {
        return Ok(vec![Entry { title: it.title.clone(), url: u.clone() }]);
    }
    set(job, i, "adding", "", upd);
    let tid = match it.torrent_id {
        Some(t) => t,
        None => tb.add_magnet(it.magnet.as_deref().ok_or("item has no magnet")?).await?,
    };
    if it.add_only {
        set(job, i, "done", "added to TorBox", upd);
        return Ok(vec![]);
    }
    set(job, i, "waiting", "", upd);
    let start = Instant::now();
    let info = loop {
        if let Some(inf) = tb.info(tid).await? {
            if is_ready(&inf) {
                break inf;
            }
            let pct = (inf.get("progress").and_then(|p| p.as_f64()).unwrap_or(0.0) * 100.0).round();
            let st = inf.get("download_state").and_then(|s| s.as_str()).unwrap_or("");
            set(job, i, "waiting", &format!("{st} {pct}%"), upd);
        }
        if start.elapsed() > o.timeout {
            return Err("timed out waiting for TorBox".into());
        }
        tokio::time::sleep(o.poll).await;
    };
    let files: Vec<TbFile> = serde_json::from_value(info.get("files").cloned().unwrap_or_default()).unwrap_or_default();
    if it.all_files {
        let mut vids: Vec<&TbFile> = files.iter().filter(|f| is_video(&f.name)).collect();
        vids.sort_by(|a, b| a.name.cmp(&b.name));
        if vids.is_empty() {
            return Err("no video files".into());
        }
        let mut out = Vec::new();
        for f in vids {
            let title = f.short_name.clone().unwrap_or_else(|| f.name.rsplit('/').next().unwrap_or(&f.name).to_string());
            out.push(Entry { title, url: tb.request_dl(tid, f.id).await? });
        }
        return Ok(out);
    }
    let f = pick_file(&files, it.season, it.episode).ok_or("no video file in torrent")?;
    Ok(vec![Entry { title: it.title.clone(), url: tb.request_dl(tid, f.id).await? }])
}

pub async fn run_send(tb: &TorBox, dir: &Path, items: Vec<SendItem>, name: &str, append: bool, o: Opts, upd: Upd<'_>) -> Job {
    let mut job = Job {
        rows: items.iter().map(|i| Row { title: i.title.clone(), state: "queued".into(), detail: String::new() }).collect(),
        ..Default::default()
    };
    upd(&job);
    let mut entries = if append { playlist::read(dir, name).unwrap_or_default() } else { Vec::new() };
    for (i, it) in items.iter().enumerate() {
        match resolve(tb, it, &mut job, i, &o, upd).await {
            Ok(mut e) => {
                entries.append(&mut e);
                if job.rows[i].state != "done" {
                    set(&mut job, i, "done", "", upd);
                }
            }
            Err(e) => set(&mut job, i, "error", &e, upd),
        }
    }
    if !entries.is_empty() {
        job.playlist = playlist::write(dir, name, &entries).ok();
    }
    job.finished = true;
    upd(&job);
    job
}
