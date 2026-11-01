//! JSON lines over the daemon's Unix socket.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::library::{Preset, Sound, Theme, Visual};
use crate::meditation::Meditation;

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
    DeletePreset {
        name: String,
    },
    /// Save a preset built by hand (Configuración > Presets): replaces one with the same name.
    StorePreset {
        name: String,
        layers: BTreeMap<String, f32>,
    },
    Add(AddRequest),
    /// Remove a downloaded sound (manifest entry and .opus). Built-in generators can't be removed.
    DeleteSound {
        id: String,
    },
    AddMeditation(AddMeditationRequest),
    DeleteMeditation {
        id: String,
    },
    /// Start a meditation from the beginning (replacing the one that's playing, if any).
    PlayMeditation {
        id: String,
    },
    /// Pause / resume the meditation.
    ToggleMeditation,
    StopMeditation,
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
pub struct AddMeditationRequest {
    pub url: String,
    pub name: Option<String>,
    /// Duration category chosen by hand (5, 10, 15, 20, 25, 30, 40 or 60).
    pub minutes: u32,
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
    #[serde(default)]
    pub meditations: Vec<Meditation>,
    /// The meditation loaded in the player (playing or paused); gone once it finishes.
    #[serde(default)]
    pub meditation: Option<MeditationPlayback>,
}

impl State {
    pub fn is_playing(&self) -> bool {
        (!self.paused && !self.active.is_empty()) || self.meditation.as_ref().is_some_and(|m| !m.paused)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeditationPlayback {
    pub id: String,
    pub position_secs: u64,
    pub total_secs: u64,
    pub paused: bool,
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
    /// A meditation download (the focus screen only lists sounds).
    #[serde(default)]
    pub meditation: bool,
}
