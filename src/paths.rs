use std::env;
use std::path::PathBuf;

use serde::Deserialize;

/// Optional user config at ~/.config/focus-ambience/config.toml.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Folder holding sounds.toml, meditations.toml, presets.toml, sounds/ and meditations/.
    pub library: Option<PathBuf>,
    /// rclone destination for the sound .opus files.
    pub remote: Option<String>,
    /// rclone destination for the meditation .opus files. By default, a `meditations` folder next
    /// to the one in `remote`.
    pub meditations_remote: Option<String>,
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

/// Library lookup order: $FOCUS_LIBRARY, config file, the repo this binary was built from (so a
/// build from a clone keeps using the lists and audio already in its `library/`), and the user
/// data dir. The lists
/// (`*.toml`) aren't created here: a missing one is an empty list until something is saved,
/// and `focus sync` brings them from the remote.
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
    data_dir().join("library")
}

pub fn sounds_dir() -> PathBuf {
    library_dir().join("sounds")
}

pub fn meditations_dir() -> PathBuf {
    library_dir().join("meditations")
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

/// Where focus installs tools it downloads itself (rclone): ~/.local/bin, or the app folder on
/// Windows (which has no such convention).
#[cfg(unix)]
pub fn bin_dir() -> PathBuf {
    home().join(".local/bin")
}

#[cfg(windows)]
pub fn bin_dir() -> PathBuf {
    data_dir().join("bin")
}

/// Resolve an external tool, preferring `bin_dir()` (where the up-to-date yt-dlp lives).
pub fn tool(name: &str) -> PathBuf {
    let local = bin_dir().join(exe(name));
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
