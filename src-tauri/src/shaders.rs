//! Anime4K (bloc97/Anime4K, MIT, see shaders/LICENSE-Anime4K.txt) GLSL presets for mpv's `glsl-shaders`.
//! The shaders are embedded, extracted to the app data dir on first use, and addressed only by preset name,
//! so the UI never passes file paths to mpv.
use std::path::Path;

macro_rules! sh { ($($n:literal),*) => { &[$(($n, include_bytes!(concat!("../shaders/", $n, ".glsl")) as &[u8])),*] } }
const FILES: &[(&str, &[u8])] = sh![
    "Anime4K_AutoDownscalePre_x2", "Anime4K_AutoDownscalePre_x4", "Anime4K_Clamp_Highlights",
    "Anime4K_Restore_CNN_M", "Anime4K_Restore_CNN_Soft_M", "Anime4K_Restore_CNN_Soft_VL", "Anime4K_Restore_CNN_VL",
    "Anime4K_Upscale_CNN_x2_M", "Anime4K_Upscale_CNN_x2_S", "Anime4K_Upscale_CNN_x2_VL",
    "Anime4K_Upscale_Denoise_CNN_x2_M", "Anime4K_Upscale_Denoise_CNN_x2_VL"
];

/// Shader chain for a preset (Anime4K README "Mode A/B/C", HQ for strong GPUs, Fast for light ones).
pub fn chain(preset: &str) -> Option<Vec<&'static str>> {
    let tail_hq = ["Anime4K_AutoDownscalePre_x2", "Anime4K_AutoDownscalePre_x4", "Anime4K_Upscale_CNN_x2_M"];
    let tail_fast = ["Anime4K_AutoDownscalePre_x2", "Anime4K_AutoDownscalePre_x4", "Anime4K_Upscale_CNN_x2_S"];
    let c = "Anime4K_Clamp_Highlights";
    let mut v = match preset {
        "off" => return Some(vec![]),
        "a-hq" => vec![c, "Anime4K_Restore_CNN_VL", "Anime4K_Upscale_CNN_x2_VL"],
        "b-hq" => vec![c, "Anime4K_Restore_CNN_Soft_VL", "Anime4K_Upscale_CNN_x2_VL"],
        "c-hq" => vec![c, "Anime4K_Upscale_Denoise_CNN_x2_VL"],
        "a-fast" => vec![c, "Anime4K_Restore_CNN_M", "Anime4K_Upscale_CNN_x2_M"],
        "b-fast" => vec![c, "Anime4K_Restore_CNN_Soft_M", "Anime4K_Upscale_CNN_x2_M"],
        "c-fast" => vec![c, "Anime4K_Upscale_Denoise_CNN_x2_M"],
        _ => return None,
    };
    v.extend(if preset.ends_with("fast") { tail_fast } else { tail_hq });
    Some(v)
}

/// Extract the embedded shaders and return mpv's `glsl-shaders` value (`:`-separated on unix, `;` on Windows).
pub fn value(dir: &Path, preset: &str) -> Result<String, String> {
    let names = chain(preset).ok_or_else(|| format!("unknown preset: {preset}"))?;
    let sd = dir.join("shaders");
    std::fs::create_dir_all(&sd).map_err(|e| e.to_string())?;
    for (n, b) in FILES {
        let p = sd.join(format!("{n}.glsl"));
        if std::fs::metadata(&p).map(|m| m.len() != b.len() as u64).unwrap_or(true) {
            std::fs::write(&p, b).map_err(|e| e.to_string())?;
        }
    }
    let sep = if cfg!(windows) { ";" } else { ":" };
    Ok(names.iter().map(|n| sd.join(format!("{n}.glsl")).to_string_lossy().into_owned()).collect::<Vec<_>>().join(sep))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_preset_resolves_to_embedded_files() {
        for p in ["off", "a-hq", "b-hq", "c-hq", "a-fast", "b-fast", "c-fast"] {
            for n in chain(p).unwrap() { assert!(FILES.iter().any(|(f, _)| *f == n), "{n}"); }
        }
        assert!(chain("bogus").is_none());
    }
}
