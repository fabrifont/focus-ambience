use std::env;
use std::path::PathBuf;

use serde::Deserialize;

/// Optional user config at ~/.config/focus-ambience/config.toml.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Folder holding sounds.toml, presets.toml and sounds/.
    pub library: Option<PathBuf>,
    /// rclone destination for the .opus files.
    pub remote: Option<String>,
    /// Opus bitrate in kbps used by `focus add`.
    pub bitrate: Option<u32>,
}

pub const DEFAULT_REMOTE: &str = "gdrive:focus-ambience/sounds";
pub const DEFAULT_BITRATE: u32 = 160;

fn home() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(var).map(PathBuf::from).unwrap_or_else(env::temp_dir)
}

#[cfg(unix)]
fn xdg(var: &str, fallback: &str) -> PathBuf {
    env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

/// %APPDATA% (roaming) or %LOCALAPPDATA%.
#[cfg(windows)]
fn appdata(var: &str) -> PathBuf {
    env::var_os(var).map(PathBuf::from).unwrap_or_else(|| home().join("AppData").join("Roaming"))
}

#[cfg(unix)]
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("focus-ambience")
}

#[cfg(windows)]
pub fn config_dir() -> PathBuf {
    appdata("APPDATA").join("focus-ambience")
}

#[cfg(unix)]
pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("focus-ambience")
}

#[cfg(windows)]
pub fn state_dir() -> PathBuf {
    appdata("LOCALAPPDATA").join("focus-ambience")
}

#[cfg(unix)]
fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("focus-ambience")
}

#[cfg(windows)]
fn data_dir() -> PathBuf {
    appdata("APPDATA").join("focus-ambience")
}

pub fn config() -> Config {
    std::fs::read_to_string(config_dir().join("config.toml"))
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// Library lookup order: $FOCUS_LIBRARY, config file, the repo this binary was built from, and
/// for a downloaded binary (whose build repo doesn't exist here) the user data dir.
pub fn library_dir() -> PathBuf {
    if let Some(p) = env::var_os("FOCUS_LIBRARY") {
        return PathBuf::from(p);
    }
    if let Some(p) = config().library {
        return p;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("library");
    if repo.is_dir() {
        return repo;
    }
    let dir = data_dir().join("library");
    seed_library(&dir);
    dir
}

/// First run of a downloaded binary: start from the manifest and presets it was built with, so
/// `focus sync pull` can fetch the audio. On later runs of a newer binary, folds in whatever
/// changed in the embedded manifest (e.g. hashes fixed after a quality bump) without touching
/// the user's own additions or presets — see `update_sounds_toml`.
fn seed_library(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    update_sounds_toml(dir);
    let presets = dir.join("presets.toml");
    if !presets.exists() {
        let _ = std::fs::write(presets, include_str!("../library/presets.toml"));
    }
}

/// Writes the embedded sounds.toml on first run; on later runs, merges it into the user's copy
/// if the embedded one carries a newer `version` (see `crate::library::merge_manifest`).
/// Doesn't go through `library::save_manifest`/`library_dir()` to avoid recursing back into
/// `seed_library`. If the file exists but can't be read or parsed (e.g. a hand-edit typo), it's
/// left untouched instead of being clobbered with the bundled copy.
fn update_sounds_toml(dir: &std::path::Path) {
    let path = dir.join("sounds.toml");
    if !path.exists() {
        let _ = std::fs::write(&path, include_str!("../library/sounds.toml"));
        return;
    }
    let Ok(contents) = std::fs::read_to_string(&path) else {
        eprintln!("⚠ no pude leer {}; lo dejo como está", path.display());
        return;
    };
    let Ok(user) = toml::from_str::<crate::library::Manifest>(&contents) else {
        eprintln!("⚠ {} no se pudo interpretar (¿lo editaste a mano?); lo dejo como está", path.display());
        return;
    };
    let Ok(bundled) = toml::from_str::<crate::library::Manifest>(include_str!("../library/sounds.toml")) else {
        return;
    };
    if bundled.version > user.version {
        let merged = crate::library::merge_manifest(user, bundled);
        let _ = crate::library::write_manifest_at(&path, &merged);
    }
}

pub fn sounds_dir() -> PathBuf {
    library_dir().join("sounds")
}

#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    let dir = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(env::temp_dir);
    dir.join("focus-ambience.sock")
}

/// Windows has AF_UNIX too (10 1803+); %TEMP% is per user.
#[cfg(windows)]
pub fn socket_path() -> PathBuf {
    env::temp_dir().join("focus-ambience.sock")
}

pub fn state_file() -> PathBuf {
    state_dir().join("state.toml")
}

pub fn log_file() -> PathBuf {
    state_dir().join("daemon.log")
}

/// Resolve an external tool, preferring ~/.local/bin (where the up-to-date yt-dlp lives).
pub fn tool(name: &str) -> PathBuf {
    let local = home().join(".local/bin").join(exe(name));
    if local.exists() { local } else { PathBuf::from(name) }
}

fn exe(name: &str) -> String {
    format!("{name}{}", env::consts::EXE_SUFFIX)
}

/// A JavaScript runtime for yt-dlp, as its `--js-runtimes` value (e.g. "node:/path/to/node").
pub fn js_runtime() -> Option<String> {
    let path = env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = env::split_paths(&path).collect();
    dirs.push(home().join(".n/bin"));
    dirs.push(home().join(".deno/bin"));
    for name in ["deno", "node"] {
        if let Some(p) = dirs.iter().map(|d| d.join(exe(name))).find(|p| p.is_file()) {
            return Some(format!("{name}:{}", p.display()));
        }
    }
    None
}
