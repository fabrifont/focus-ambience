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
pub const DEFAULT_BITRATE: u32 = 64;

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("focus-ambience")
}

pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("focus-ambience")
}

pub fn config() -> Config {
    std::fs::read_to_string(config_dir().join("config.toml"))
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// Library lookup order: $FOCUS_LIBRARY, config file, then the repo this binary was built from.
pub fn library_dir() -> PathBuf {
    if let Some(p) = env::var_os("FOCUS_LIBRARY") {
        return PathBuf::from(p);
    }
    if let Some(p) = config().library {
        return p;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("library")
}

pub fn sounds_dir() -> PathBuf {
    library_dir().join("sounds")
}

pub fn socket_path() -> PathBuf {
    let dir = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(env::temp_dir);
    dir.join("focus-ambience.sock")
}

pub fn state_file() -> PathBuf {
    state_dir().join("state.toml")
}

pub fn log_file() -> PathBuf {
    state_dir().join("daemon.log")
}

/// Resolve an external tool, preferring ~/.local/bin (where the up-to-date yt-dlp lives).
pub fn tool(name: &str) -> PathBuf {
    let local = home().join(".local/bin").join(name);
    if local.exists() { local } else { PathBuf::from(name) }
}

/// A JavaScript runtime for yt-dlp, as its `--js-runtimes` value (e.g. "node:/path/to/node").
pub fn js_runtime() -> Option<String> {
    let path = env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = env::split_paths(&path).collect();
    dirs.push(home().join(".n/bin"));
    dirs.push(home().join(".deno/bin"));
    for name in ["deno", "node"] {
        if let Some(p) = dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) {
            return Some(format!("{name}:{}", p.display()));
        }
    }
    None
}
