use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

/// Visual theme: drives colors and the animated scene in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Skyrim,
    Lotr,
    Starwars,
    Nature,
    Generated,
    Other,
}

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Theme::Skyrim => "Skyrim",
            Theme::Lotr => "Tierra Media",
            Theme::Starwars => "Star Wars",
            Theme::Nature => "Naturaleza",
            Theme::Generated => "Generados",
            Theme::Other => "Otros",
        }
    }

    pub fn default_visual(self) -> Visual {
        match self {
            Theme::Skyrim => Visual::Snow,
            Theme::Lotr => Visual::Fireflies,
            Theme::Starwars => Visual::Stars,
            Theme::Nature => Visual::Leaves,
            Theme::Generated => Visual::Static,
            Theme::Other => Visual::Fireflies,
        }
    }
}

impl std::str::FromStr for Theme {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.to_lowercase().as_str() {
            "skyrim" => Theme::Skyrim,
            "lotr" | "tierra-media" | "middle-earth" => Theme::Lotr,
            "starwars" | "star-wars" => Theme::Starwars,
            "nature" | "naturaleza" => Theme::Nature,
            "other" | "otros" => Theme::Other,
            _ => anyhow::bail!("tema desconocido: {s} (skyrim, lotr, starwars, nature, other)"),
        })
    }
}

/// Particle effect a sound contributes to the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visual {
    Rain,
    Snow,
    Fireflies,
    Stars,
    Embers,
    Leaves,
    Static,
    Waves,
}

impl std::str::FromStr for Visual {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.to_lowercase().as_str() {
            "rain" => Visual::Rain,
            "snow" => Visual::Snow,
            "fireflies" => Visual::Fireflies,
            "stars" => Visual::Stars,
            "embers" => Visual::Embers,
            "leaves" => Visual::Leaves,
            "static" => Visual::Static,
            "waves" => Visual::Waves,
            _ => anyhow::bail!("visual desconocido: {s}"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Generator {
    White,
    Pink,
    Brown,
    Binaural40,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SoundKind {
    Generated { generator: Generator },
    File { file: String },
}

/// A sound as the daemon and TUI see it (manifest entries plus built-in generators).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sound {
    pub id: String,
    pub name: String,
    pub theme: Theme,
    pub visual: Visual,
    pub kind: SoundKind,
    /// Seconds; 0 for generated sounds.
    pub duration: u64,
    /// False when the .opus is listed in the manifest but not on this machine yet.
    pub available: bool,
}

/// One entry of library/sounds.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    pub name: String,
    pub theme: Theme,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual: Option<Visual>,
    pub url: String,
    pub file: String,
    pub sha256: String,
    pub duration: u64,
    pub bytes: u64,
    /// Minutes kept from the start of the video, if it was trimmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default, rename = "sound")]
    pub sounds: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    /// Sound id -> volume (0..1).
    pub layers: BTreeMap<String, f32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Presets {
    #[serde(default, rename = "preset")]
    pub presets: Vec<Preset>,
}

fn manifest_path() -> PathBuf {
    paths::library_dir().join("sounds.toml")
}

fn presets_path() -> PathBuf {
    paths::library_dir().join("presets.toml")
}

pub fn load_manifest() -> Result<Manifest> {
    let path = manifest_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("leyendo {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::default()),
        Err(e) => Err(e).with_context(|| format!("leyendo {}", path.display())),
    }
}

const MANIFEST_HEADER: &str = "# Sonidos descargados. Los .opus viven en sounds/ (no versionados) y se\n\
# sincronizan con `focus sync`. Este archivo sí va en git.\n\n";

pub fn save_manifest(m: &Manifest) -> Result<()> {
    write_atomic(&manifest_path(), &format!("{MANIFEST_HEADER}{}", toml::to_string_pretty(m)?))
}

pub fn load_presets() -> Presets {
    std::fs::read_to_string(presets_path()).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_presets(p: &Presets) -> Result<()> {
    write_atomic(&presets_path(), &toml::to_string_pretty(p)?)
}

pub fn write_atomic(path: &std::path::Path, contents: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn generated() -> Vec<Sound> {
    let g = |id: &str, name: &str, generator, visual| Sound {
        id: id.into(),
        name: name.into(),
        theme: Theme::Generated,
        visual,
        kind: SoundKind::Generated { generator },
        duration: 0,
        available: true,
    };
    vec![
        g("noise-white", "Ruido blanco", Generator::White, Visual::Static),
        g("noise-pink", "Ruido rosa", Generator::Pink, Visual::Static),
        g("noise-brown", "Ruido marrón", Generator::Brown, Visual::Static),
        g("binaural-40", "Binaural 40 Hz · gamma", Generator::Binaural40, Visual::Waves),
    ]
}

/// Every playable sound, grouped by theme in display order.
pub fn all_sounds() -> Result<Vec<Sound>> {
    let dir = paths::sounds_dir();
    let mut sounds: Vec<Sound> = load_manifest()?
        .sounds
        .into_iter()
        .map(|e| Sound {
            available: dir.join(&e.file).exists(),
            visual: e.visual.unwrap_or_else(|| e.theme.default_visual()),
            kind: SoundKind::File { file: e.file },
            id: e.id,
            name: e.name,
            theme: e.theme,
            duration: e.duration,
        })
        .collect();
    sounds.extend(generated());
    sounds.sort_by_key(|s| s.theme);
    Ok(sounds)
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}
