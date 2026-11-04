mod applog;
mod shaders;
#[cfg(desktop)]
mod mpvlib;
#[cfg(desktop)]
mod player;
#[cfg(target_os = "macos")]
mod embed;

use companion_core::{
    addons::{self, Addon},
    config::{self, Config},
    jobs::{self, Job, SendItem},
    player::{KeyInfo, PlayerCmd},
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

/// Poster/summary/episode info for the Watching tab. Uses the user's enabled metadata addons (Cinemeta by default).
#[tauri::command]
async fn title_meta(st: State<'_, AppState>, name: String, series: bool, season: Option<u32>, episode: Option<u32>) -> Result<Value, String> {
    let list = st.addons();
    let kind = if series { "series" } else { "movie" };
    let hit = addons::search(&st.client, &list, kind, &name).await;
    let Some(h) = hit.first() else { return Ok(Value::Null) };
    let id = h["id"].as_str().unwrap_or("").to_string();
    let m = addons::meta(&st.client, &list, kind, &id).await.unwrap_or(Value::Null);
    let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let ep = m.get("videos").and_then(Value::as_array).and_then(|v| {
        v.iter().find(|x| x["season"].as_u64() == season.map(u64::from) && x["episode"].as_u64() == episode.map(u64::from))
    });
    let es = |k: &str| ep.and_then(|e| e.get(k)).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(json!({
        "id": id, "name": if s("name").is_empty() { h["name"].as_str().unwrap_or("").to_string() } else { s("name") },
        "year": s("releaseInfo"), "poster": h["poster"].as_str().map(String::from).filter(|x| !x.is_empty()).unwrap_or_else(|| s("poster")),
        "background": s("background"), "description": s("description"), "rating": s("imdbRating"), "runtime": s("runtime"),
        "genres": m.get("genres").cloned().unwrap_or(json!([])),
        "ep_title": if es("name").is_empty() { es("title") } else { es("name") }, "ep_thumb": es("thumbnail"), "ep_overview": es("overview"),
    }))
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

// ------------------------------------------------------------------ built-in player (mpv)
/// Opens the playlist in the built-in player. Returns "embedded" (libmpv inside our own window,
/// macOS) or "external" (separate mpv window).
#[cfg(desktop)]
#[tauri::command]
async fn player_open(app: tauri::AppHandle, st: State<'_, AppState>, name: Option<String>, file: Option<String>, index: Option<usize>) -> Result<String, String> {
    let f = name.or(file).ok_or("no playlist given")?;
    let entries = playlist::read(&st.playlists(), &f).map_err(|e| e.to_string())?;
    start_player(&app, &st.dir, entries, index.unwrap_or(0))
}

/// Plays a file path or URL (single item or a list) without needing a saved playlist.
#[cfg(desktop)]
#[tauri::command]
async fn player_play(app: tauri::AppHandle, st: State<'_, AppState>, entries: Vec<Entry>, index: Option<usize>) -> Result<String, String> {
    crate::alog!("info", "play {} item(s) from the player", entries.len());
    start_player(&app, &st.dir, entries, index.unwrap_or(0))
}

#[cfg(desktop)]
fn start_player(app: &tauri::AppHandle, dir: &std::path::Path, entries: Vec<Entry>, index: usize) -> Result<String, String> {
    if entries.is_empty() {
        return Err("nothing to play".into());
    }
    let index = index.min(entries.len() - 1);
    #[cfg(target_os = "macos")]
    if mpvlib::find_libmpv().is_some() {
        match open_embedded_macos(app, dir, &entries, index) {
            Ok(()) => return Ok("embedded".into()),
            Err(e) => crate::alog!("error", "embedded player failed, falling back to an external mpv window: {e}"),
        }
    }
    player::open(app, dir, &entries, index)?;
    Ok("external".into())
}

#[cfg(desktop)]
#[tauri::command]
fn player_props() -> Value {
    player::props_snapshot()
}

#[cfg(desktop)]
#[tauri::command]
fn player_set(prop: String, value: Value) -> Result<(), String> {
    player::set_property(&prop, value)
}

/// Apply an Anime4K preset ("off", "a-hq", "b-hq", "c-hq", "a-fast", "b-fast", "c-fast").
#[cfg(desktop)]
#[tauri::command]
fn player_filter(st: State<AppState>, preset: String) -> Result<(), String> {
    let v = shaders::value(&st.dir, &preset).inspect_err(|e| crate::alog!("error", "shader preset {preset}: {e}"))?;
    crate::alog!("info", "upscaling preset: {preset}");
    player::set_raw_property("glsl-shaders", Value::String(v))
}

#[cfg(mobile)]
#[tauri::command]
fn player_filter(_preset: String) -> Result<(), String> {
    Err("The built-in player is not available on mobile yet".into())
}

/// Frame-rate target (0 = off, else 60/90/120) via FFmpeg `minterpolate`; `mode` is "blend" (cheap) or "mci" (motion-compensated, heavy CPU).
#[cfg(desktop)]
#[tauri::command]
fn player_fps(target: u32, mode: String) -> Result<(), String> {
    crate::alog!("info", "frame-rate target: {target} ({mode})");
    if !matches!(target, 0 | 30 | 48 | 60 | 90 | 120) {
        return Err("unsupported frame rate".into());
    }
    let vf = if target == 0 {
        String::new()
    } else {
        let mi = match mode.as_str() {
            "mci" => "mi_mode=mci:mc_mode=aobmc:me_mode=bidir:vsbmc=1",
            "blend" => "mi_mode=blend",
            _ => return Err("unknown mode".into()),
        };
        format!("lavfi=[minterpolate=fps={target}:{mi}]")
    };
    // Software filters need frames in system memory: use a copy-back decoder while one is active.
    player::set_raw_property("hwdec", Value::String(if target == 0 { "auto-safe" } else { "auto-copy-safe" }.into()))?;
    player::set_raw_property("vf", Value::String(vf))
}

#[cfg(mobile)]
#[tauri::command]
fn player_fps(_target: u32, _mode: String) -> Result<(), String> {
    Err("The built-in player is not available on mobile yet".into())
}

#[cfg(desktop)]
#[tauri::command]
fn player_exec(args: Vec<Value>) -> Result<(), String> {
    player::exec(&args)
}

#[tauri::command]
fn player_recent(st: State<AppState>) -> Vec<companion_core::player::RecentItem> {
    companion_core::player::RecentStore::load(&st.dir.join("recent.json")).items
}

#[tauri::command]
fn player_recent_remove(st: State<AppState>, url: String) {
    let p = st.dir.join("recent.json");
    let mut r = companion_core::player::RecentStore::load(&p);
    r.remove(&url);
    let _ = r.save(&p);
}

/// Address of the mpv window currently glued under the main window (0 = none).
#[cfg(target_os = "macos")]
static MPV_WINDOW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Where the UI wants the video (CSS px in the main webview); None/zero-size = hidden.
#[cfg(target_os = "macos")]
static VIDEO_RECT: std::sync::Mutex<embed::Rect> = std::sync::Mutex::new(None);
#[cfg(target_os = "macos")]
static HOOKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "macos")]
fn on_main<T: Send + 'static>(app: &tauri::AppHandle, f: impl FnOnce(objc2::MainThreadMarker) -> T + Send + 'static) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let _ = tx.send(f(mtm));
        }
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(5)).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn refit(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    let child = MPV_WINDOW.load(Ordering::SeqCst);
    if child == 0 {
        return;
    }
    let Some(win) = app.get_webview_window("main") else { return };
    let Ok(parent) = win.ns_window() else { return };
    let parent = parent as usize;
    let rect = *VIDEO_RECT.lock().unwrap();
    let app2 = app.clone();
    let _ = app2.run_on_main_thread(move || {
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            embed::resync(mtm, parent, child, rect);
        }
    });
}

/// The UI reports where the video area is (CSS px relative to the webview's top-left).
#[tauri::command]
fn player_set_rect(app: tauri::AppHandle, x: f64, y: f64, w: f64, h: f64) {
    #[cfg(target_os = "macos")]
    {
        *VIDEO_RECT.lock().unwrap() = Some((x, y, w, h));
        refit(&app);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, x, y, w, h);
}

/// Fullscreen for the player. macOS uses "simple" fullscreen (same Space, no animation) because native
/// fullscreen moves the window to its own Space and mpv's child window then covers the HTML controls.
#[tauri::command]
fn logs_get(since: u64) -> Vec<applog::Line> { applog::since(since) }
#[tauri::command]
fn logs_clear() { applog::clear() }
#[tauri::command]
fn log_push(level: String, msg: String) { applog::push(if ["error", "warn", "info", "debug"].contains(&level.as_str()) { &level } else { "info" }, &msg) }

#[tauri::command]
fn window_fullscreen(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    let w = app.get_webview_window("main").ok_or("no window")?;
    crate::alog!("info", "fullscreen: {on}");
    #[cfg(target_os = "macos")]
    w.set_simple_fullscreen(on).map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "macos"))]
    w.set_fullscreen(on).map_err(|e| e.to_string())?;
    Ok(on)
}

#[cfg(target_os = "macos")]
fn open_embedded_macos(app: &tauri::AppHandle, dir: &std::path::Path, entries: &[Entry], index: usize) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use tauri::WindowEvent;
    let win = app.get_webview_window("main").ok_or("main window not found")?;
    let parent = win.ns_window().map_err(|e| e.to_string())? as usize;
    if !HOOKED.swap(true, Ordering::SeqCst) {
        let app2 = app.clone();
        win.on_window_event(move |e| match e {
            WindowEvent::Destroyed => {
                MPV_WINDOW.store(0, Ordering::SeqCst);
                player::close();
            }
            WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => refit(&app2),
            _ => {}
        });
    }
    MPV_WINDOW.store(0, Ordering::SeqCst);
    let known = on_main(app, embed::snapshot)?;
    player::open_embedded(app, dir, entries, index)?;
    // mpv creates its window asynchronously; adopt it as soon as it exists.
    let app2 = app.clone();
    std::thread::spawn(move || {
        for _ in 0..100 {
            let k = known.clone();
            let rect = *VIDEO_RECT.lock().unwrap();
            if let Ok(Some(child)) = on_main(&app2, move |mtm| embed::adopt(mtm, parent, &k, rect)) {
                MPV_WINDOW.store(child, Ordering::SeqCst);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        crate::alog!("error", "embedded player: mpv window never appeared");
    });
    Ok(())
}

#[cfg(desktop)]
#[tauri::command]
fn player_cmd(cmd: String, arg: Option<f64>) -> Result<(), String> {
    let c = PlayerCmd::from_name(&cmd, arg).ok_or_else(|| format!("unknown player command: {cmd}"))?;
    player::command(&c)
}

#[cfg(desktop)]
#[tauri::command]
fn player_close() {
    player::close()
}

#[cfg(mobile)]
#[tauri::command]
fn player_open(_name: Option<String>, _file: Option<String>, _index: Option<usize>) -> Result<(), String> {
    Err("The built-in player is not available on mobile yet".into())
}

#[cfg(mobile)]
#[tauri::command]
fn player_play(_entries: Vec<Entry>, _index: Option<usize>) -> Result<String, String> {
    Err("The built-in player is not available on mobile yet".into())
}

#[cfg(mobile)]
#[tauri::command]
fn player_props() -> Value {
    json!({})
}

#[cfg(mobile)]
#[tauri::command]
fn player_set(_prop: String, _value: Value) -> Result<(), String> {
    Err("The built-in player is not available on mobile yet".into())
}

#[cfg(mobile)]
#[tauri::command]
fn player_exec(_args: Vec<Value>) -> Result<(), String> {
    Err("The built-in player is not available on mobile yet".into())
}

#[cfg(mobile)]
#[tauri::command]
fn player_cmd(_cmd: String, _arg: Option<f64>) -> Result<(), String> {
    Err("The built-in player is not available on mobile yet".into())
}

#[cfg(mobile)]
#[tauri::command]
fn player_close() {}

/// Keyboard shortcuts for a help overlay.
#[tauri::command]
fn player_keys() -> Vec<KeyInfo> {
    companion_core::player::key_list()
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
            search, title_meta, fetch_streams, send, job_get, library, playlists_list, playlist_read, playlist_text,
            playlist_save, playlist_delete, playlist_export, open_playlist,
            player_open, player_play, player_cmd, player_close, player_keys,
            player_props, player_set, player_exec, player_recent, player_recent_remove, player_set_rect, player_filter, player_fps, window_fullscreen, logs_get, logs_clear, log_push
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
