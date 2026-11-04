//! Stremio addon protocol: manifests, add/manage, stream + catalog queries across many addons.
use crate::parse::{dedupe, parse_stream, Stream};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub const CINEMETA: &str = "https://v3-cinemeta.strem.io/manifest.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub resources: Vec<Value>,
    #[serde(default, rename = "idPrefixes")]
    pub id_prefixes: Vec<String>,
    #[serde(default)]
    pub catalogs: Vec<Value>,
    #[serde(default)]
    pub configurable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Addon {
    pub id: String,
    /// Full manifest URL. May embed secrets (user config), so it is never logged.
    pub url: String,
    pub name: String,
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    pub manifest: Manifest,
    #[serde(default)]
    pub last_ok: Option<bool>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
}
fn default_timeout() -> u64 {
    20
}

#[derive(Serialize, Clone, Debug)]
pub struct AddonError {
    pub addon_id: String,
    pub addon_name: String,
    pub error: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct FetchResult {
    pub streams: Vec<Stream>,
    pub errors: Vec<AddonError>,
    /// (addon id, ok, error, latency ms) so the caller can store health.
    pub health: Vec<(String, bool, Option<String>, u64)>,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder().user_agent("companion/0.1").build().expect("http client")
}

/// Accepts stremio://, https://…/manifest.json, or a bare addon base URL.
pub fn normalize_manifest_url(input: &str) -> Result<String, String> {
    let mut s = input.trim().to_string();
    if let Some(r) = s.strip_prefix("stremio://") {
        s = format!("https://{r}");
    }
    if !(s.starts_with("https://") || s.starts_with("http://")) {
        return Err("Addon URL must start with https:// or stremio://".into());
    }
    let mut u = url::Url::parse(&s).map_err(|e| format!("Invalid URL: {e}"))?;
    let path = u.path().trim_end_matches('/').to_string();
    if !path.ends_with("/manifest.json") {
        u.set_path(&format!("{path}/manifest.json"));
    }
    Ok(u.to_string())
}

/// Base used for resource requests (manifest URL minus /manifest.json, query and fragment).
pub fn base_of(manifest_url: &str) -> String {
    let mut u = match url::Url::parse(manifest_url) {
        Ok(u) => u,
        Err(_) => return manifest_url.trim_end_matches("/manifest.json").to_string(),
    };
    u.set_query(None);
    u.set_fragment(None);
    let p = u.path().trim_end_matches("/manifest.json").trim_end_matches('/').to_string();
    u.set_path(&p);
    u.to_string().trim_end_matches('/').to_string()
}

pub fn masked(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => format!("{}://{}/…", u.scheme(), u.host_str().unwrap_or("?")),
        Err(_) => "…".into(),
    }
}

fn strings(v: Option<&Value>) -> Option<Vec<String>> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
}

pub fn manifest_from_value(v: &Value) -> Result<Manifest, String> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let m = Manifest {
        id: s("id"),
        name: s("name"),
        version: s("version"),
        description: s("description"),
        logo: v.get("logo").and_then(Value::as_str).map(str::to_string),
        types: strings(v.get("types")).unwrap_or_default(),
        resources: v.get("resources").and_then(Value::as_array).cloned().unwrap_or_default(),
        id_prefixes: strings(v.get("idPrefixes")).unwrap_or_default(),
        catalogs: v.get("catalogs").and_then(Value::as_array).cloned().unwrap_or_default(),
        configurable: v.pointer("/behaviorHints/configurable") == Some(&Value::Bool(true)),
    };
    if m.id.is_empty() || m.name.is_empty() {
        return Err("Not a Stremio addon manifest (missing id or name)".into());
    }
    if m.resources.is_empty() {
        return Err("Addon declares no resources".into());
    }
    Ok(m)
}

pub async fn fetch_manifest(client: &reqwest::Client, manifest_url: &str) -> Result<Manifest, String> {
    let r = client
        .get(manifest_url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("Could not reach addon: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Addon returned HTTP {}", r.status().as_u16()));
    }
    let v: Value = r.json().await.map_err(|e| format!("Manifest is not valid JSON: {e}"))?;
    manifest_from_value(&v)
}

pub async fn add_addon(client: &reqwest::Client, input: &str) -> Result<Addon, String> {
    let url = normalize_manifest_url(input)?;
    let t0 = Instant::now();
    let manifest = fetch_manifest(client, &url).await?;
    Ok(Addon {
        id: uuid::Uuid::new_v4().to_string(),
        name: manifest.name.clone(),
        url,
        enabled: true,
        timeout_secs: default_timeout(),
        manifest,
        last_ok: Some(true),
        last_error: None,
        latency_ms: Some(t0.elapsed().as_millis() as u64),
    })
}

/// Does this addon serve `resource` for this type/id? Honors per-resource and manifest-level types/idPrefixes.
pub fn supports(m: &Manifest, resource: &str, kind: &str, id: &str) -> bool {
    for r in &m.resources {
        let (types, prefixes) = match r {
            Value::String(n) if n == resource => (m.types.clone(), m.id_prefixes.clone()),
            Value::Object(o) if o.get("name").and_then(Value::as_str) == Some(resource) => (
                strings(o.get("types")).unwrap_or_else(|| m.types.clone()),
                strings(o.get("idPrefixes")).unwrap_or_else(|| m.id_prefixes.clone()),
            ),
            _ => continue,
        };
        if !types.is_empty() && !types.iter().any(|t| t == kind) {
            return false;
        }
        if !prefixes.is_empty() && !prefixes.iter().any(|p| id.starts_with(p.as_str())) {
            return false;
        }
        return true;
    }
    false
}

fn path_id(id: &str) -> String {
    urlencoding::encode(id).replace("%3A", ":")
}

/// Query every enabled addon that serves streams for this id, in parallel; one failure never fails the batch.
pub async fn fetch_streams(
    client: &reqwest::Client,
    addons: &[Addon],
    kind: &str,
    video_id: &str,
    season: Option<u32>,
    episode: Option<u32>,
) -> FetchResult {
    let jobs = addons.iter().filter(|a| a.enabled && supports(&a.manifest, "stream", kind, video_id)).map(|a| {
        let url = format!("{}/stream/{}/{}.json", base_of(&a.url), kind, path_id(video_id));
        let client = client.clone();
        let (id, name, timeout) = (a.id.clone(), a.name.clone(), a.timeout_secs);
        async move {
            let t0 = Instant::now();
            let res: Result<Vec<Stream>, String> = async {
                let r = client.get(&url).timeout(Duration::from_secs(timeout)).send().await.map_err(|e| format!("{}", e.without_url()))?;
                if !r.status().is_success() {
                    return Err(format!("HTTP {}", r.status().as_u16()));
                }
                let v: Value = r.json().await.map_err(|_| "invalid JSON".to_string())?;
                Ok(v.get("streams")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|s| {
                                let mut p = parse_stream(s);
                                p.addon_id = id.clone();
                                p.addon_name = name.clone();
                                p
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            .await;
            (id, name, res, t0.elapsed().as_millis() as u64)
        }
    });
    let mut out = FetchResult::default();
    let mut all = Vec::new();
    for (id, name, res, ms) in futures::future::join_all(jobs).await {
        match res {
            Ok(mut s) => {
                out.health.push((id, true, None, ms));
                all.append(&mut s);
            }
            Err(e) => {
                out.health.push((id.clone(), false, Some(e.clone()), ms));
                out.errors.push(AddonError { addon_id: id, addon_name: name, error: e });
            }
        }
    }
    let mut streams = dedupe(all);
    if season.is_some() || episode.is_some() {
        for s in &mut streams {
            s.season = season;
            s.episode = episode;
        }
    }
    out.streams = streams;
    out
}

fn has_search(cat: &Value) -> bool {
    let named = |k: &str| {
        cat.get(k).and_then(Value::as_array).map(|a| a.iter().any(|x| x.get("name").and_then(Value::as_str) == Some("search") || x.as_str() == Some("search"))).unwrap_or(false)
    };
    named("extra") || named("extraSupported")
}

/// Search the catalogs of all enabled addons that support it; merge by id (first addon wins).
pub async fn search(client: &reqwest::Client, addons: &[Addon], kind: &str, query: &str) -> Vec<Value> {
    for q in query_variants(query) {
        let r = search_one(client, addons, kind, &q).await;
        if !r.is_empty() {
            return r;
        }
    }
    Vec::new()
}

/// Catalog search matches from the start of a title, so "beekeeper 2" misses "The Beekeeper 2".
/// Try the query as typed, then with a leading "the ", then without a leading "the ".
fn query_variants(query: &str) -> Vec<String> {
    let q = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut v = vec![q.clone()];
    let lower = q.to_lowercase();
    if let Some(rest) = lower.strip_prefix("the ") {
        v.push(rest.to_string());
    } else if !q.is_empty() {
        v.push(format!("the {q}"));
    }
    v
}

async fn search_one(client: &reqwest::Client, addons: &[Addon], kind: &str, query: &str) -> Vec<Value> {
    let q = urlencoding::encode(query).into_owned();
    let jobs = addons.iter().filter(|a| a.enabled).filter_map(|a| {
        let cat = a.manifest.catalogs.iter().find(|c| c.get("type").and_then(Value::as_str) == Some(kind) && has_search(c))?;
        let cid = cat.get("id").and_then(Value::as_str)?;
        let url = format!("{}/catalog/{}/{}/search={}.json", base_of(&a.url), kind, cid, q);
        let (client, t) = (client.clone(), a.timeout_secs);
        Some(async move {
            let r = client.get(&url).timeout(Duration::from_secs(t)).send().await.ok()?;
            r.json::<Value>().await.ok()
        })
    });
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for v in futures::future::join_all(jobs).await.into_iter().flatten() {
        for m in v.get("metas").and_then(Value::as_array).cloned().unwrap_or_default() {
            let id = m.get("imdb_id").or_else(|| m.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
            if id.is_empty() || !seen.insert(id.clone()) {
                continue;
            }
            out.push(hit_of(&m, id));
        }
    }
    out.truncate(12);
    out
}

fn hit_of(m: &Value, id: String) -> Value {
    json!({
        "id": id,
        "name": m.get("name").and_then(Value::as_str).unwrap_or(""),
        "year": m.get("releaseInfo").or_else(|| m.get("year")).and_then(Value::as_str).unwrap_or(""),
        "poster": m.get("poster").and_then(Value::as_str),
    })
}

/// Browsable catalogs (no required extra such as a mandatory search/genre) of enabled addons for a type.
pub fn catalogs(addons: &[Addon], kind: &str) -> Vec<Value> {
    let required = |c: &Value| {
        ["extra", "extraSupported"].iter().any(|k| {
            c.get(*k).and_then(Value::as_array).map(|a| a.iter().any(|x| x.get("isRequired").and_then(Value::as_bool) == Some(true))).unwrap_or(false)
        })
    };
    let mut out = Vec::new();
    for a in addons.iter().filter(|a| a.enabled) {
        for c in a.manifest.catalogs.iter().filter(|c| c.get("type").and_then(Value::as_str) == Some(kind) && !required(c)) {
            let Some(cid) = c.get("id").and_then(Value::as_str) else { continue };
            let name = c.get("name").and_then(Value::as_str).unwrap_or(cid);
            out.push(json!({"addon_id": a.id, "addon_name": a.name, "id": cid, "name": name}));
        }
    }
    out
}

/// One page of a catalog (`/catalog/{kind}/{id}[/skip=N].json`). Only IMDb-id items are kept, since streams are fetched by IMDb id.
pub async fn browse(client: &reqwest::Client, addons: &[Addon], addon_id: &str, kind: &str, cat_id: &str, skip: u32) -> Result<Vec<Value>, String> {
    let a = addons.iter().find(|a| a.id == addon_id && a.enabled).ok_or("Addon not found or disabled")?;
    let base = base_of(&a.url);
    let url = if skip > 0 {
        format!("{}/catalog/{}/{}/skip={}.json", base, kind, urlencoding::encode(cat_id), skip)
    } else {
        format!("{}/catalog/{}/{}.json", base, kind, urlencoding::encode(cat_id))
    };
    let r = client.get(&url).timeout(Duration::from_secs(a.timeout_secs)).send().await.map_err(|_| "Could not reach the addon".to_string())?;
    let v = r.json::<Value>().await.map_err(|_| "The addon returned an unreadable catalog".to_string())?;
    Ok(v.get("metas").and_then(Value::as_array).cloned().unwrap_or_default().iter().filter_map(|m| {
        let id = m.get("imdb_id").or_else(|| m.get("id")).and_then(Value::as_str)?.to_string();
        id.starts_with("tt").then(|| hit_of(m, id))
    }).collect())
}

/// Fetch full metadata (`/meta/{kind}/{id}.json`) from the first enabled addon that serves it.
pub async fn meta(client: &reqwest::Client, addons: &[Addon], kind: &str, id: &str) -> Option<Value> {
    for a in addons.iter().filter(|a| a.enabled && supports(&a.manifest, "meta", kind, id)) {
        let url = format!("{}/meta/{}/{}.json", base_of(&a.url), kind, urlencoding::encode(id));
        if let Ok(r) = client.get(&url).timeout(Duration::from_secs(a.timeout_secs)).send().await {
            if let Ok(v) = r.json::<Value>().await {
                if let Some(m) = v.get("meta").filter(|m| m.is_object()) {
                    return Some(m.clone());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn query_variants_add_and_strip_leading_the() {
        assert_eq!(super::query_variants("beekeeper  2"), vec!["beekeeper 2", "the beekeeper 2"]);
        assert_eq!(super::query_variants("The Matrix"), vec!["The Matrix", "matrix"]);
    }

    use super::*;

    #[test]
    fn normalizes_urls() {
        assert_eq!(normalize_manifest_url("stremio://h.com/a/b/manifest.json").unwrap(), "https://h.com/a/b/manifest.json");
        assert_eq!(normalize_manifest_url("https://h.com/a/").unwrap(), "https://h.com/a/manifest.json");
        assert!(normalize_manifest_url("ftp://x").is_err());
        assert_eq!(base_of("https://h.com/a/b/manifest.json?x=1"), "https://h.com/a/b");
        assert_eq!(masked("https://h.com/secret/manifest.json"), "https://h.com/…");
    }

    #[test]
    fn catalogs_skip_required_extras_and_disabled_addons() {
        let m = manifest_from_value(&json!({"id":"a","name":"A","resources":["catalog"],"types":["movie"],"catalogs":[
            {"type":"movie","id":"top","name":"Top"},
            {"type":"movie","id":"genre","name":"By genre","extra":[{"name":"genre","isRequired":true}]},
            {"type":"series","id":"s","name":"Shows"}]})).unwrap();
        let mk = |enabled| Addon { id: "x".into(), url: "https://h/manifest.json".into(), name: "A".into(), enabled, timeout_secs: 5, manifest: m.clone(), last_ok: None, last_error: None, latency_ms: None };
        let c = catalogs(&[mk(true)], "movie");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0]["id"], "top");
        assert!(catalogs(&[mk(false)], "movie").is_empty());
    }

    #[test]
    fn supports_respects_types_and_prefixes() {
        let m = manifest_from_value(&json!({"id":"a","name":"A","types":["movie","series"],"idPrefixes":["tt"],"resources":["stream"]})).unwrap();
        assert!(supports(&m, "stream", "series", "tt1:1:2"));
        assert!(!supports(&m, "stream", "anime", "tt1"));
        assert!(!supports(&m, "stream", "movie", "kitsu:1"));
        assert!(!supports(&m, "catalog", "movie", "tt1"));
        let m2 = manifest_from_value(&json!({"id":"b","name":"B","resources":[{"name":"stream","types":["movie"],"idPrefixes":["kitsu"]}]})).unwrap();
        assert!(supports(&m2, "stream", "movie", "kitsu:5"));
        assert!(!supports(&m2, "stream", "movie", "tt5"));
    }

    #[test]
    fn rejects_bad_manifests() {
        assert!(manifest_from_value(&json!({"name":"x"})).is_err());
        assert!(manifest_from_value(&json!({"id":"x","name":"x","resources":[]})).is_err());
    }
}
