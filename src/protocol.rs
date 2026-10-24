//! JSON lines over the daemon's Unix socket.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::library::{Preset, Sound, Theme, Visual};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Toggle {
        id: String,
    },
    SetActive {
        id: String,
        active: bool,
    },
    SetVolume {
        id: String,
        volume: f32,
    },
    SetMaster {
        volume: f32,
    },
    /// Pause/resume everything without forgetting which layers were on.
    PauseAll,
    StopAll,
    Timer {
        minutes: Option<u32>,
    },
    LoadPreset {
        name: String,
    },
    SavePreset {
        name: String,
    },
    DeletePreset {
        name: String,
    },
    Add(AddRequest),
    Reload,
    /// Ask the daemon to push level updates for the animations.
    Watch {
        levels: bool,
    },
    Quit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRequest {
    pub url: String,
    pub name: Option<String>,
    pub theme: Theme,
    pub visual: Option<Visual>,
    /// Keep only the first N minutes (for nature sounds that loop anyway).
    pub trim: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    State(State),
    Levels { master: f32, layers: Vec<(String, f32)> },
    Notice { text: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    pub sounds: Vec<Sound>,
    pub presets: Vec<Preset>,
    /// Sound id -> volume, for every sound that has one.
    pub volumes: BTreeMap<String, f32>,
    /// Sounds the user turned on (still listed while paused).
    pub active: Vec<String>,
    pub paused: bool,
    pub master: f32,
    pub timer: Option<TimerState>,
    pub downloads: Vec<Download>,
}

impl State {
    pub fn is_playing(&self) -> bool {
        !self.paused && !self.active.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerState {
    pub total_secs: u64,
    pub remaining_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub name: String,
    pub stage: String,
    /// 0..1, or negative when unknown.
    pub progress: f32,
    pub error: Option<String>,
}
