//! Minimal TorBox API client (add magnet, poll, list library, get download links).
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone)]
pub struct TorBox {
    base: String,
    key: String,
    client: reqwest::Client,
}

#[derive(Serialize, Clone, Debug)]
pub struct LibItem {
    pub id: i64,
    pub name: String,
    pub size: u64,
    pub progress: f64,
    pub state: String,
    pub ready: bool,
    pub videos: usize,
}

pub const DEFAULT_BASE: &str = "https://api.torbox.app/v1/api";

impl TorBox {
    pub fn new(base: &str, key: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .user_agent("companion/0.1")
            .build()
            .expect("http client");
        Self { base: base.trim_end_matches('/').into(), key: key.into(), client }
    }

    async fn call(&self, req: reqwest::RequestBuilder) -> Result<Value, String> {
        if self.key.is_empty() {
            return Err("Set your TorBox API key in Settings first.".into());
        }
        let resp = req.bearer_auth(&self.key).send().await.map_err(|e| format!("Network error: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("Network error: {e}"))?;
        if !status.is_success() {
            return Err(format!("TorBox HTTP {}: {}", status.as_u16(), text.chars().take(300).collect::<String>()));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("Bad response: {e}"))?;
        if v.get("success") == Some(&Value::Bool(false)) {
            let d = v.get("detail").or_else(|| v.get("error")).and_then(Value::as_str).unwrap_or("request failed");
            return Err(format!("TorBox: {d}"));
        }
        Ok(v.get("data").cloned().unwrap_or(Value::Null))
    }

    pub async fn add_magnet(&self, magnet: &str) -> Result<i64, String> {
        let form = reqwest::multipart::Form::new()
            .text("magnet", magnet.to_string())
            .text("seed", "1")
            .text("allow_zip", "false");
        let d = self.call(self.client.post(format!("{}/torrents/createtorrent", self.base)).multipart(form)).await?;
        d.get("torrent_id").or_else(|| d.get("queued_id")).and_then(Value::as_i64).ok_or_else(|| "TorBox returned no torrent id".to_string())
    }

    pub async fn info(&self, id: i64) -> Result<Option<Value>, String> {
        let d = self
            .call(self.client.get(format!("{}/torrents/mylist", self.base)).query(&[("id", id.to_string()), ("bypass_cache", "true".into())]))
            .await?;
        Ok(match d {
            Value::Array(a) => a.into_iter().next(),
            Value::Null => None,
            v => Some(v),
        })
    }

    pub async fn library(&self) -> Result<Vec<LibItem>, String> {
        let d = self.call(self.client.get(format!("{}/torrents/mylist", self.base)).query(&[("bypass_cache", "true")])).await?;
        Ok(d.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| {
                        let videos = t.get("files").and_then(Value::as_array).map(|f| {
                            f.iter().filter(|x| x.get("name").and_then(Value::as_str).map(crate::parse::is_video).unwrap_or(false)).count()
                        });
                        Some(LibItem {
                            id: t.get("id")?.as_i64()?,
                            name: t.get("name").and_then(Value::as_str).unwrap_or("").into(),
                            size: t.get("size").and_then(Value::as_u64).unwrap_or(0),
                            progress: t.get("progress").and_then(Value::as_f64).unwrap_or(0.0),
                            state: t.get("download_state").and_then(Value::as_str).unwrap_or("").into(),
                            ready: is_ready(t),
                            videos: videos.unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn request_dl(&self, torrent_id: i64, file_id: i64) -> Result<String, String> {
        let d = self
            .call(self.client.get(format!("{}/torrents/requestdl", self.base)).query(&[
                ("token", self.key.clone()),
                ("torrent_id", torrent_id.to_string()),
                ("file_id", file_id.to_string()),
                ("redirect", "false".into()),
            ]))
            .await?;
        d.as_str().map(str::to_string).ok_or_else(|| "TorBox returned no download link".to_string())
    }
}

pub fn is_ready(t: &Value) -> bool {
    ["download_finished", "download_present", "cached"].iter().any(|k| t.get(*k) == Some(&Value::Bool(true)))
}
