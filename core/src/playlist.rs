//! m3u8 building/parsing and the on-disk playlist library.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::{Path, PathBuf}, time::UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub title: String,
    pub url: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct PlaylistInfo {
    pub file: String,
    pub name: String,
    pub count: usize,
    pub mtime: u64,
}

pub fn build_m3u8(entries: &[Entry]) -> String {
    let mut s = String::from("#EXTM3U\n");
    for e in entries {
        s.push_str(&format!("#EXTINF:-1,{}\n{}\n", e.title.replace('\n', " "), e.url));
    }
    s
}

pub fn parse_m3u8(text: &str) -> Vec<Entry> {
    let (mut out, mut title) = (Vec::new(), None::<String>);
    for line in text.lines().map(str::trim) {
        if let Some(r) = line.strip_prefix("#EXTINF:") {
            title = Some(r.split_once(',').map(|x| x.1).unwrap_or(r).to_string());
        } else if !line.is_empty() && !line.starts_with('#') {
            let t = title.take().unwrap_or_else(|| line.rsplit('/').next().unwrap_or(line).to_string());
            out.push(Entry { title: t, url: line.to_string() });
        }
    }
    out
}

pub fn safe_name(n: &str) -> String {
    let kept: String = n.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ' ' | '-')).collect();
    let s = kept.trim().replace(' ', "_");
    let s: String = s.chars().take(80).collect();
    if s.is_empty() { "playlist".into() } else { s }
}

fn path_for(dir: &Path, file: &str) -> PathBuf {
    let stem = file.strip_suffix(".m3u8").unwrap_or(file);
    dir.join(format!("{}.m3u8", safe_name(stem)))
}

/// Writes and returns the file name.
pub fn write(dir: &Path, name: &str, entries: &[Entry]) -> io::Result<String> {
    fs::create_dir_all(dir)?;
    let p = path_for(dir, name);
    fs::write(&p, build_m3u8(entries))?;
    Ok(p.file_name().unwrap().to_string_lossy().into_owned())
}

pub fn read(dir: &Path, file: &str) -> io::Result<Vec<Entry>> {
    Ok(parse_m3u8(&fs::read_to_string(path_for(dir, file))?))
}

pub fn read_text(dir: &Path, file: &str) -> io::Result<String> {
    fs::read_to_string(path_for(dir, file))
}

pub fn full_path(dir: &Path, file: &str) -> PathBuf {
    path_for(dir, file)
}

pub fn delete(dir: &Path, file: &str) -> io::Result<()> {
    fs::remove_file(path_for(dir, file))
}

/// Save, optionally renaming (removes the old file when the name changed).
pub fn save_as(dir: &Path, name: &str, old: Option<&str>, entries: &[Entry]) -> io::Result<String> {
    let f = write(dir, name, entries)?;
    if let Some(o) = old {
        let op = path_for(dir, o);
        if op != path_for(dir, name) {
            let _ = fs::remove_file(op);
        }
    }
    Ok(f)
}

pub fn list(dir: &Path) -> Vec<PlaylistInfo> {
    let mut v: Vec<PlaylistInfo> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let name = file.strip_suffix(".m3u8")?.to_string();
            let mtime = e.metadata().ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
            let count = read(dir, &file).map(|x| x.len()).unwrap_or(0);
            Some(PlaylistInfo { file, name, count, mtime })
        })
        .collect();
    v.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_rename() {
        let d = tempfile::tempdir().unwrap();
        let e = vec![Entry { title: "A".into(), url: "http://x/1".into() }, Entry { title: "B".into(), url: "http://x/2".into() }];
        write(d.path(), "My Show", &e).unwrap();
        assert_eq!(read(d.path(), "My_Show.m3u8").unwrap(), e);
        save_as(d.path(), "Renamed", Some("My_Show"), &e[..1]).unwrap();
        let names: Vec<_> = list(d.path()).into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["Renamed"]);
        delete(d.path(), "Renamed.m3u8").unwrap();
        assert!(list(d.path()).is_empty());
    }

    #[test]
    fn safe_name_blocks_traversal() {
        assert!(!safe_name("../../etc/passwd").contains('/'));
        assert_eq!(safe_name(""), "playlist");
    }
}
