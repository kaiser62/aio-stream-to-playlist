//! Persistent settings: TorBox key + the user's addon list. One JSON file per device.
use crate::addons::Addon;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Config {
    #[serde(default)]
    pub torbox_key: String,
    #[serde(default)]
    pub addons: Vec<Addon>,
    #[serde(default)]
    pub seeded: bool,
}

/// A 32-byte data key. It lives in the OS keychain (see the shell's vault), never next to the file it protects.
pub type DataKey = [u8; 32];

pub fn new_data_key() -> DataKey {
    let mut k = [0u8; 32];
    getrandom::getrandom(&mut k).expect("os randomness");
    k
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (s.len() % 2 == 0).then(|| ()).and_then(|_| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect())
}

fn seal(plain: &[u8], key: &DataKey) -> String {
    use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
    let mut n = [0u8; 12];
    getrandom::getrandom(&mut n).expect("os randomness");
    let ct = ChaCha20Poly1305::new(key.into()).encrypt(Nonce::from_slice(&n), plain).expect("encrypt");
    serde_json::json!({"enc": 1, "n": hex(&n), "c": hex(&ct)}).to_string()
}

fn open(text: &str, key: &DataKey) -> Option<Vec<u8>> {
    use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let n = unhex(v.get("n")?.as_str()?)?;
    let c = unhex(v.get("c")?.as_str()?)?;
    if n.len() != 12 {
        return None;
    }
    ChaCha20Poly1305::new(key.into()).decrypt(Nonce::from_slice(&n), c.as_slice()).ok()
}

/// True when the file on disk is the encrypted envelope (so a data key is needed to read it).
pub fn is_encrypted(path: &Path) -> bool {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).map(|v| v.get("enc").is_some()).unwrap_or(false)
}

/// Load plain or encrypted config. An encrypted file with a wrong/missing key loads as empty (and is not overwritten by `load` itself).
pub fn load_with(path: &Path, key: Option<&DataKey>) -> Config {
    let Ok(text) = fs::read_to_string(path) else { return Config::default() };
    if is_encrypted(path) {
        return key.and_then(|k| open(&text, k)).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    }
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn load(path: &Path) -> Config {
    load_with(path, None)
}

/// Write the config atomically with owner-only permissions from the first byte (the file holds the TorBox key).
pub fn save(path: &Path, cfg: &Config) -> io::Result<()> {
    save_with(path, cfg, None)
}

pub fn save_with(path: &Path, cfg: &Config, key: Option<&DataKey>) -> io::Result<()> {
    use std::io::Write;
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    let body = match key {
        Some(k) => seal(serde_json::to_string(cfg).unwrap().as_bytes(), k),
        None => serde_json::to_string_pretty(cfg).unwrap(),
    };
    f.write_all(body.as_bytes())?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_roundtrip_hides_the_key_and_needs_the_data_key() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config.json");
        let cfg = Config { torbox_key: "SECRET-KEY-123".into(), ..Default::default() };
        let k = new_data_key();
        save_with(&p, &cfg, Some(&k)).unwrap();
        let raw = fs::read_to_string(&p).unwrap();
        assert!(!raw.contains("SECRET-KEY-123") && is_encrypted(&p));
        assert_eq!(load_with(&p, Some(&k)).torbox_key, "SECRET-KEY-123");
        assert_eq!(load_with(&p, Some(&new_data_key())).torbox_key, "");
        assert_eq!(load_with(&p, None).torbox_key, "");
    }

    #[test]
    fn plain_files_still_load_for_migration() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config.json");
        save(&p, &Config { torbox_key: "abc".into(), ..Default::default() }).unwrap();
        assert!(!is_encrypted(&p));
        assert_eq!(load_with(&p, Some(&new_data_key())).torbox_key, "abc");
    }
}
