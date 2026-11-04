use companion_core::{
    addons::{self, Addon},
    config::{self, Config},
    jobs::{self, Job, SendItem},
    playlist::{self, Entry, PlaylistInfo},
    torbox::{self, TorBox},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

struct AppState {
    dir: PathBuf,
    cfg: Mutex<Config>,
    jobs: Arc<Mutex<HashMap<String, Job>>>,
    client: reqwest::Client,
}

impl AppState {
    fn save(&self) -> Result<(), String> {
        let c = self.cfg.lock().unwrap();
        config::save(&self.dir.join("config.json"), &c).map_err(|e| e.to_string())
    }
    fn playlists(&self) -> PathBuf {
        self.dir.join("playlists")
    }
    fn tb(&self) -> TorBox {
        TorBox::new(torbox::DEFAULT_BASE, &self.cfg.lock().unwrap().torbox_key)
    }
    fn addons(&self) -> Vec<Addon> {
        self.cfg.lock().unwrap().addons.clone()
    }
}

#[derive(Serialize)]
struct AddonView {
    id: String,
    name: String,
    enabled: bool,
    timeout_secs: u64,
    version: String,
    description: String,
    logo: Option<String>,
    types: Vec<String>,
    resources: Vec<String>,
    catalogs: usize,
    configurable: bool,
    url_masked: String,
    last_ok: Option<bool>,
    last_error: Option<String>,
    latency_ms: Option<u64>,
}

fn view(a: &Addon) -> AddonView {
    let m = &a.manifest;
    AddonView {
        id: a.id.clone(),
        name: a.name.clone(),
        enabled: a.enabled,
        timeout_secs: a.timeout_secs,
        version: m.version.clone(),
        description: m.description.clone(),
        logo: m.logo.clone(),
        types: m.types.clone(),
        resources: m
            .resources
            .iter()
            .filter_map(|r| r.as_str().map(String::from).or_else(|| r.get("name").and_then(Value::as_str).map(String::from)))
            .collect(),
        catalogs: m.catalogs.len(),
        configurable: m.configurable,
        url_masked: addons::masked(&a.url),
        last_ok: a.last_ok,
        last_error: a.last_error.clone(),
        latency_ms: a.latency_ms,
    }
}

// ------------------------------------------------------------------ settings
#[tauri::command]
fn config_get(st: State<AppState>) -> Value {
    json!({ "has_key": !st.cfg.lock().unwrap().torbox_key.is_empty() })
}

#[tauri::command]
fn config_set(st: State<AppState>, torbox_key: Option<String>) -> Result<(), String> {
    if let Some(k) = torbox_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        st.cfg.lock().unwrap().torbox_key = k;
    }
    st.save()
}

// ------------------------------------------------------------------ addon manager
async fn add_one(st: &AppState, url: &str) -> Result<Addon, String> {
    let a = addons::add_addon(&st.client, url).await?;
    {
        let mut c = st.cfg.lock().unwrap();
        if c.addons.iter().any(|x| x.manifest.id == a.manifest.id && addons::base_of(&x.url) == addons::base_of(&a.url)) {
            return Err("This addon is already added".into());
        }
        c.addons.push(a.clone());
    }
    st.save()?;
    Ok(a)
}

#[tauri::command]
fn addons_list(st: State<AppState>) -> Vec<AddonView> {
    st.addons().iter().map(view).collect()
}

#[tauri::command]
async fn addons_add(st: State<'_, AppState>, url: String) -> Result<AddonView, String> {
    add_one(&st, &url).await.map(|a| view(&a))
}

#[tauri::command]
fn addons_update(st: State<AppState>, id: String, name: Option<String>, enabled: Option<bool>, timeout_secs: Option<u64>) -> Result<(), String> {
    {
        let mut c = st.cfg.lock().unwrap();
        let a = c.addons.iter_mut().find(|a| a.id == id).ok_or("unknown addon")?;
        if let Some(n) = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
            a.name = n;
        }
        if let Some(e) = enabled {
            a.enabled = e;
        }
        if let Some(t) = timeout_secs {
            a.timeout_secs = t.clamp(3, 120);
        }
    }
    st.save()
}

#[tauri::command]
fn addons_remove(st: State<AppState>, id: String) -> Result<(), String> {
    st.cfg.lock().unwrap().addons.retain(|a| a.id != id);
    st.save()
}

#[tauri::command]
fn addons_reorder(st: State<AppState>, ids: Vec<String>) -> Result<(), String> {
    {
        let mut c = st.cfg.lock().unwrap();
        c.addons.sort_by_key(|a| ids.iter().position(|i| *i == a.id).unwrap_or(usize::MAX));
    }
    st.save()
}

/// Re-fetch the manifest and record health (also serves as the "test" button).
#[tauri::command]
async fn addons_refresh(st: State<'_, AppState>, id: String) -> Result<AddonView, String> {
    let url = st.addons().into_iter().find(|a| a.id == id).ok_or("unknown addon")?.url;
    let t0 = std::time::Instant::now();
    let res = addons::fetch_manifest(&st.client, &url).await;
    let ms = t0.elapsed().as_millis() as u64;
    let out = {
        let mut c = st.cfg.lock().unwrap();
        let a = c.addons.iter_mut().find(|a| a.id == id).ok_or("unknown addon")?;
        a.latency_ms = Some(ms);
        match res {
            Ok(m) => {
                a.manifest = m;
                a.last_ok = Some(true);
                a.last_error = None;
            }
            Err(e) => {
                a.last_ok = Some(false);
                a.last_error = Some(e);
            }
        }
        view(a)
    };
    st.save()?;
    Ok(out)
}

#[tauri::command]
fn addon_url(st: State<AppState>, id: String) -> Result<String, String> {
    st.addons().into_iter().find(|a| a.id == id).map(|a| a.url).ok_or_else(|| "unknown addon".into())
}

#[tauri::command]
fn addons_export(st: State<AppState>) -> Vec<Value> {
    st.addons().iter().map(|a| json!({"name": a.name, "url": a.url})).collect()
}

#[tauri::command]
async fn addons_import(st: State<'_, AppState>, urls: Vec<String>) -> Result<Value, String> {
    let (mut added, mut errors) = (0, Vec::new());
    for u in urls.iter().map(|u| u.trim()).filter(|u| !u.is_empty()) {
        match add_one(&st, u).await {
            Ok(_) => added += 1,
            Err(e) => errors.push(format!("{}: {e}", addons::masked(u))),
        }
    }
    Ok(json!({ "added": added, "errors": errors }))
}

/// First run: add Cinemeta (metadata only) so title search works. No stream sources are preinstalled.
#[tauri::command]
async fn addons_seed(st: State<'_, AppState>) -> Result<(), String> {
    {
        let c = st.cfg.lock().unwrap();
        if c.seeded || !c.addons.is_empty() {
            return Ok(());
        }
    }
    add_one(&st, addons::CINEMETA).await?;
    st.cfg.lock().unwrap().seeded = true;
    st.save()
}

fn http_url(url: &str) -> Result<(), String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(())
    } else {
        Err("only http(s) links can be opened".into())
    }
}

#[tauri::command]
fn addon_configure(app: tauri::AppHandle, st: State<AppState>, id: String) -> Result<(), String> {
    let a = st.addons().into_iter().find(|a| a.id == id).ok_or("unknown addon")?;
    let url = format!("{}/configure", addons::base_of(&a.url));
    http_url(&url)?;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ search / fetch
#[tauri::command]
async fn search(st: State<'_, AppState>, kind: String, query: String) -> Result<Vec<Value>, String> {
    let list = st.addons();
    if list.iter().all(|a| !a.enabled) {
        return Err("No addons enabled. Add one in the Addons tab.".into());
    }
    Ok(addons::search(&st.client, &list, &kind, &query).await)
}

#[tauri::command]
async fn fetch_streams(st: State<'_, AppState>, kind: String, imdb: String, season: Option<u32>, ep1: Option<u32>, ep2: Option<u32>) -> Result<Value, String> {
    let list = st.addons();
    if list.iter().all(|a| !a.enabled) {
        return Err("No addons enabled. Add one in the Addons tab.".into());
    }
    let (mut streams, mut errors, mut health) = (Vec::new(), Vec::new(), Vec::new());
    if kind == "movie" {
        let r = addons::fetch_streams(&st.client, &list, "movie", &imdb, None, None).await;
        streams = r.streams;
        errors.extend(r.errors.iter().map(|e| format!("{}: {}", e.addon_name, e.error)));
        health = r.health;
    } else {
        let (s, a) = (season.unwrap_or(1), ep1.unwrap_or(1));
        for ep in a..=ep2.unwrap_or(a).max(a) {
            let r = addons::fetch_streams(&st.client, &list, "series", &format!("{imdb}:{s}:{ep}"), Some(s), Some(ep)).await;
            streams.extend(r.streams);
            errors.extend(r.errors.iter().map(|e| format!("E{ep} {}: {}", e.addon_name, e.error)));
            health.extend(r.health);
        }
    }
    {
        let mut c = st.cfg.lock().unwrap();
        for (id, ok, err, ms) in health {
            if let Some(a) = c.addons.iter_mut().find(|a| a.id == id) {
                a.last_ok = Some(ok);
                a.last_error = err;
                a.latency_ms = Some(ms);
            }
        }
    }
    let _ = st.save();
    Ok(json!({ "streams": streams, "errors": errors }))
}

// ------------------------------------------------------------------ TorBox + jobs
#[tauri::command]
fn send(st: State<AppState>, items: Vec<SendItem>, name: String, append: bool) -> Result<Value, String> {
    let id = format!("{:x}", SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let (tb, dir, store, jid) = (st.tb(), st.playlists(), st.jobs.clone(), id.clone());
    tauri::async_runtime::spawn(async move {
        let upd = {
            let (store, jid) = (store.clone(), jid.clone());
            move |j: &Job| {
                store.lock().unwrap().insert(jid.clone(), j.clone());
            }
        };
        jobs::run_send(&tb, &dir, items, &name, append, jobs::Opts::default(), &upd).await;
    });
    Ok(json!({ "job": id }))
}

#[tauri::command]
fn job_get(st: State<AppState>, id: String) -> Job {
    st.jobs.lock().unwrap().get(&id).cloned().unwrap_or(Job { rows: vec![], finished: false, playlist: None })
}

#[tauri::command]
async fn library(st: State<'_, AppState>) -> Result<Value, String> {
    let tb = st.tb();
    Ok(json!({ "items": tb.library().await? }))
}

// ------------------------------------------------------------------ playlists
#[tauri::command]
fn playlists_list(st: State<AppState>) -> Vec<PlaylistInfo> {
    playlist::list(&st.playlists())
}

#[tauri::command]
fn playlist_read(st: State<AppState>, file: String) -> Result<Vec<Entry>, String> {
    playlist::read(&st.playlists(), &file).map_err(|e| e.to_string())
}

#[tauri::command]
fn playlist_text(st: State<AppState>, file: String) -> Result<String, String> {
    playlist::read_text(&st.playlists(), &file).map_err(|e| e.to_string())
}

#[tauri::command]
fn playlist_save(st: State<AppState>, name: String, old: Option<String>, entries: Vec<Entry>) -> Result<String, String> {
    playlist::save_as(&st.playlists(), &name, old.as_deref(), &entries).map_err(|e| e.to_string())
}

#[tauri::command]
fn playlist_delete(st: State<AppState>, file: String) -> Result<(), String> {
    playlist::delete(&st.playlists(), &file).map_err(|e| e.to_string())
}

/// Copy the playlist to the user's Downloads folder (desktop). Returns the destination path.
#[tauri::command]
fn playlist_export(app: tauri::AppHandle, st: State<AppState>, file: String) -> Result<String, String> {
    let src = playlist::full_path(&st.playlists(), &file);
    let dl = app.path().download_dir().map_err(|_| "No Downloads folder on this device; use Copy instead".to_string())?;
    let dest = dl.join(src.file_name().ok_or("bad file")?);
    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

#[cfg(desktop)]
#[tauri::command]
fn open_playlist(st: State<AppState>, file: String, player: Option<String>) -> Result<(), String> {
    use std::process::Command;
    let p = playlist::full_path(&st.playlists(), &file);
    if !p.exists() {
        return Err("playlist not found".into());
    }
    let allowed = ["IINA", "VLC", "mpv"];
    let r = if cfg!(target_os = "macos") {
        match player.as_deref().filter(|x| allowed.contains(x)) {
            Some(app) => Command::new("open").arg("-a").arg(app).arg(&p).spawn(),
            None => Command::new("open").arg(&p).spawn(),
        }
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", ""]).arg(&p).spawn()
    } else {
        Command::new("xdg-open").arg(&p).spawn()
    };
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(mobile)]
#[tauri::command]
fn open_playlist(_file: String, _player: Option<String>) -> Result<(), String> {
    Err("Opening a player directly is not available on mobile yet; use Copy or Export".into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let cfg = config::load(&dir.join("config.json"));
            app.manage(AppState { dir, cfg: Mutex::new(cfg), jobs: Arc::new(Mutex::new(HashMap::new())), client: addons::http_client() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config_get, config_set, addons_list, addons_add, addons_update, addons_remove, addons_reorder,
            addons_refresh, addon_url, addons_export, addons_import, addons_seed, addon_configure,
            search, fetch_streams, send, job_get, library, playlists_list, playlist_read, playlist_text,
            playlist_save, playlist_delete, playlist_export, open_playlist
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
