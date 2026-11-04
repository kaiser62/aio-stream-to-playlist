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

pub fn load(path: &Path) -> Config {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(path: &Path, cfg: &Config) -> io::Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, serde_json::to_string_pretty(cfg).unwrap())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
