//! Normalise Stremio stream objects (any addon) into one flat, filterable shape.
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub const VIDEO_EXT: [&str; 8] = [".mkv", ".mp4", ".avi", ".m4v", ".mov", ".ts", ".webm", ".wmv"];

fn re(p: &str) -> Regex {
    Regex::new(p).expect("valid regex")
}
type Table = Vec<(&'static str, Regex)>;
fn table(items: &[(&'static str, &str)]) -> Table {
    items.iter().map(|(l, p)| (*l, re(p))).collect()
}

static RES_RE: Lazy<Regex> = Lazy::new(|| re(r"(?i)(2160p|4k|uhd|1440p|1080p|720p|576p|480p|360p)"));
static SIZE_RE: Lazy<Regex> = Lazy::new(|| re(r"(?i)(\d+(?:[.,]\d+)?)\s?(GB|GiB|MB|MiB)"));
static SEED_RE: Lazy<Regex> = Lazy::new(|| re(r"(?i)(?:👤|seeders?:?)\s*(\d+)"));
static BAD_RE: Lazy<Regex> =
    Lazy::new(|| re(r"(?i)\b(cam|camrip|hdcam|ts|telesync|tc|hdts|scr|screener|r5)\b"));
static EP_RE: Lazy<Regex> = Lazy::new(|| re(r"[Ss](\d{1,2})[ ._-]?[Ee](\d{1,3})"));
static EP_ALT: Lazy<Regex> = Lazy::new(|| re(r"\b(\d{1,2})x(\d{2,3})\b"));
static EMOJI_RE: Lazy<Regex> = Lazy::new(|| re(r"[\x{1F000}-\x{1FFFF}\x{2600}-\x{27BF}\x{FE0F}]+"));
static CACHED_RE: Lazy<Regex> = Lazy::new(|| re(r"\[[A-Z]{2,4}\+\]|⚡"));
static FILE_EXT_RE: Lazy<Regex> = Lazy::new(|| re(r"(?i)\.(mkv|mp4|avi|m4v)$"));
static FILE_HINT_RE: Lazy<Regex> = Lazy::new(|| re(r"[Ss]\d+[Ee]\d+|\b(?:19|20)\d\d\b"));
static LANG_RE: Lazy<Regex> = Lazy::new(|| {
    re(r"(?i)\b(multi|dual|english|french|german|spanish|italian|japanese|korean|hindi|russian|portuguese)\b")
});
static CODEC: Lazy<Table> = Lazy::new(|| {
    table(&[
        ("AV1", r"(?i)\bav1\b"),
        ("HEVC", r"(?i)\b(hevc|x265|h\.?265)\b"),
        ("AVC", r"(?i)\b(avc|x264|h\.?264)\b"),
    ])
});
static HDR: Lazy<Table> = Lazy::new(|| {
    table(&[
        ("DV", r"(?i)\b(dv|dolby[ .-]?vision)\b"),
        ("HDR10+", r"(?i)hdr10\+"),
        ("HDR10", r"(?i)hdr10\b"),
        ("HDR", r"(?i)\bhdr\b"),
    ])
});
static AUDIO: Lazy<Table> = Lazy::new(|| {
    table(&[
        ("Atmos", r"(?i)atmos"),
        ("TrueHD", r"(?i)truehd"),
        ("DTS-HD", r"(?i)dts[ .-]?hd"),
        ("DTS", r"(?i)\bdts\b"),
        ("DD+", r"(?i)(ddp|dd\+|e-?ac-?3)"),
        ("DD", r"(?i)\b(dd|ac-?3)\b"),
        ("AAC", r"(?i)\baac\b"),
    ])
});
static QUALITY: Lazy<Table> = Lazy::new(|| {
    table(&[
        ("REMUX", r"(?i)remux"),
        ("BluRay", r"(?i)blu-?ray|bdrip|brrip"),
        ("WEB-DL", r"(?i)web[ .-]?dl"),
        ("WEBRip", r"(?i)web[ .-]?rip"),
        ("HDTV", r"(?i)hdtv"),
    ])
});

fn first(t: &Table, text: &str) -> String {
    t.iter().find(|(_, r)| r.is_match(text)).map(|(l, _)| l.to_string()).unwrap_or_default()
}
fn clean(text: &str) -> String {
    EMOJI_RE.replace_all(text, " ").into_owned()
}
fn res_rank(r: &str) -> u32 {
    match r {
        "2160p" => 2160,
        "1440p" => 1440,
        "1080p" => 1080,
        "720p" => 720,
        "576p" => 576,
        "480p" => 480,
        "360p" => 360,
        _ => 0,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Stream {
    pub title: String,
    pub source: String,
    pub resolution: String,
    pub res_rank: u32,
    pub size: u64,
    pub seeders: Option<u32>,
    pub cached: bool,
    pub bad: bool,
    pub codec: String,
    pub hdr: String,
    pub audio: String,
    pub quality: String,
    pub langs: Vec<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    #[serde(rename = "infoHash")]
    pub info_hash: Option<String>,
    pub magnet: Option<String>,
    pub url: Option<String>,
    pub file_idx: Option<i64>,
    #[serde(default)]
    pub addon_id: String,
    #[serde(default)]
    pub addon_name: String,
}

pub fn parse_stream(s: &Value) -> Stream {
    let hints = s.get("behaviorHints").unwrap_or(&Value::Null);
    let name = s.get("name").and_then(Value::as_str).unwrap_or("");
    let desc = s
        .get("description")
        .and_then(Value::as_str)
        .or_else(|| s.get("title").and_then(Value::as_str))
        .unwrap_or("");
    let hint_name = hints.get("filename").and_then(Value::as_str).unwrap_or("");
    let blob = format!("{name}\n{desc}\n{hint_name}");

    let mut filename = hint_name.to_string();
    if filename.is_empty() {
        for line in desc.lines() {
            let c = clean(line);
            let c = c.trim();
            if FILE_EXT_RE.is_match(c) || FILE_HINT_RE.is_match(c) {
                filename = c.to_string();
                break;
            }
        }
    }
    let mut resolution = RES_RE.captures(&blob).map(|c| c[1].to_lowercase()).unwrap_or_default();
    if resolution == "4k" || resolution == "uhd" {
        resolution = "2160p".into();
    }
    let size = hints
        .get("videoSize")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
        .filter(|n| *n > 0)
        .or_else(|| {
            SIZE_RE.captures(&blob).map(|c| {
                let n: f64 = c[1].replace(',', ".").parse().unwrap_or(0.0);
                let unit = if c[2].to_lowercase().starts_with('g') { 1_073_741_824.0 } else { 1_048_576.0 };
                (n * unit) as u64
            })
        })
        .unwrap_or(0);
    let seeders = SEED_RE.captures(&blob).and_then(|c| c[1].parse().ok());
    let url = s.get("url").and_then(Value::as_str).map(str::to_string);
    let info_hash =
        s.get("infoHash").and_then(Value::as_str).map(str::to_lowercase).filter(|h| !h.is_empty());
    let cached = CACHED_RE.is_match(name) || (url.is_some() && info_hash.is_none());

    let ep_src = if filename.is_empty() { blob.as_str() } else { filename.as_str() };
    let ep = [&*EP_RE, &*EP_ALT].iter().find_map(|r| {
        r.captures(ep_src)
            .and_then(|c| Some((c[1].parse::<u32>().ok()?, c[2].parse::<u32>().ok()?)))
    });

    let magnet = info_hash.as_ref().map(|h| {
        let mut m = format!(
            "magnet:?xt=urn:btih:{h}&dn={}",
            urlencoding::encode(if filename.is_empty() { "stream" } else { &filename })
        );
        let trackers = s
            .get("sources")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str()).filter_map(|x| x.strip_prefix("tracker:")).take(12).collect::<Vec<_>>())
            .unwrap_or_default();
        for t in trackers {
            m.push_str("&tr=");
            m.push_str(&urlencoding::encode(t));
        }
        m
    });

    let title = if !filename.is_empty() {
        filename.clone()
    } else {
        let n = clean(name).trim().to_string();
        if n.is_empty() { "unknown".into() } else { n }
    };
    let mut langs: Vec<String> = LANG_RE.find_iter(&blob).map(|m| m.as_str().to_lowercase()).collect();
    langs.sort();
    langs.dedup();

    Stream {
        title,
        source: clean(name).trim().to_string(),
        res_rank: res_rank(&resolution),
        resolution,
        size,
        seeders,
        cached,
        bad: BAD_RE.is_match(&blob),
        codec: first(&CODEC, &blob),
        hdr: first(&HDR, &blob),
        audio: first(&AUDIO, &blob),
        quality: first(&QUALITY, &blob),
        langs,
        season: ep.map(|e| e.0),
        episode: ep.map(|e| e.1),
        info_hash,
        magnet,
        url,
        file_idx: s.get("fileIdx").and_then(Value::as_i64),
        addon_id: String::new(),
        addon_name: String::new(),
    }
}

/// Drop rows with neither hash nor url; dedupe on hash/url keeping the best copy
/// (cached first, then most seeders, then largest).
pub fn dedupe(streams: Vec<Stream>) -> Vec<Stream> {
    let mut idx: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<Stream> = Vec::new();
    for s in streams {
        let Some(key) = s.info_hash.clone().or_else(|| s.url.clone()) else { continue };
        let score = |x: &Stream| (x.cached, x.seeders.unwrap_or(0), x.size);
        match idx.get(&key) {
            Some(&i) => {
                if score(&s) > score(&out[i]) {
                    out[i] = s;
                }
            }
            None => {
                idx.insert(key, out.len());
                out.push(s);
            }
        }
    }
    out
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TbFile {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub short_name: Option<String>,
}

pub fn is_video(name: &str) -> bool {
    let l = name.to_lowercase();
    VIDEO_EXT.iter().any(|e| l.ends_with(e))
}

/// Choose the file matching S/E when given, else the largest video file.
pub fn pick_file(files: &[TbFile], season: Option<u32>, episode: Option<u32>) -> Option<&TbFile> {
    let vids: Vec<&TbFile> = files.iter().filter(|f| is_video(&f.name)).collect();
    if let (Some(s), Some(e)) = (season, episode) {
        for f in &vids {
            for r in [&*EP_RE, &*EP_ALT] {
                if let Some(c) = r.captures(&f.name) {
                    if c[1].parse::<u32>().ok() == Some(s) && c[2].parse::<u32>().ok() == Some(e) {
                        return Some(f);
                    }
                }
            }
        }
    }
    vids.into_iter().max_by_key(|f| f.size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_rich_fields() {
        let s = parse_stream(&json!({"name":"[TB+] X 4K",
            "description":"Show.S01E02.2160p.WEB-DL.HEVC.HDR10.Atmos.mkv\n💾 8.4 GB 👤 120","infoHash":"ABC"}));
        assert_eq!((s.resolution.as_str(), s.codec.as_str(), s.hdr.as_str()), ("2160p", "HEVC", "HDR10"));
        assert_eq!((s.audio.as_str(), s.quality.as_str()), ("Atmos", "WEB-DL"));
        assert_eq!((s.season, s.episode, s.seeders, s.cached), (Some(1), Some(2), Some(120), true));
        assert!(s.magnet.unwrap().starts_with("magnet:?xt=urn:btih:abc"));
    }

    #[test]
    fn size_from_text_and_hint() {
        assert_eq!(parse_stream(&json!({"name":"a","description":"900 MB","url":"http://x"})).size, 943_718_400);
        assert_eq!(parse_stream(&json!({"name":"a","url":"http://x","behaviorHints":{"videoSize":5}})).size, 5);
    }

    #[test]
    fn direct_url_is_cached_and_bad_detected() {
        let s = parse_stream(&json!({"name":"[RD] x","description":"Movie.1080p.CAM.mkv","url":"http://x"}));
        assert!(s.cached && s.bad && s.info_hash.is_none());
    }

    #[test]
    fn dedupe_keeps_best_and_drops_unusable() {
        let raw: Vec<Stream> = [json!({"name":"a","infoHash":"H"}),
            json!({"name":"b","infoHash":"h","description":"👤 9"}), json!({"name":"none"})]
            .iter().map(parse_stream).collect();
        let out = dedupe(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seeders, Some(9));
    }

    #[test]
    fn picks_episode_file() {
        let f = |id, name: &str, size| TbFile { id, name: name.into(), size, short_name: None };
        let files = vec![f(1, "a/S01E01.mkv", 9), f(2, "a/S01E02.mkv", 1), f(3, "a/x.nfo", 99)];
        assert_eq!(pick_file(&files, Some(1), Some(2)).unwrap().id, 2);
        assert_eq!(pick_file(&files, None, None).unwrap().id, 1);
    }
}
