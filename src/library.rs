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
    /// Sube cada vez que el manifest embebido en el binario cambia (nuevos sonidos, hashes
    /// corregidos). Los manifests viejos no tienen este campo y quedan en 0, así que siempre
    /// se actualizan la primera vez que corre un binario más nuevo. Ver `merge_manifest`.
    #[serde(default)]
    pub version: u32,
    #[serde(default, rename = "sound")]
    pub sounds: Vec<ManifestEntry>,
}

/// Mezcla el manifest embebido en el binario con el del usuario: las entradas cuyo id coincide
/// se reemplazan por la versión embebida (así se corrigen hashes/duración stale sin que el
/// usuario tenga que hacer nada), las que el usuario agregó con `focus add` se conservan tal
/// cual, y las entradas nuevas del embebido se agregan al final.
///
/// Si el usuario editó a mano una entrada que también viene en el embebido (p. ej. el `trim`),
/// esa edición se pierde en la próxima actualización: no hay forma de distinguir "el usuario la
/// tocó a propósito" de "esto es una copia vieja que hay que corregir", y corregir sonidos stale
/// es el caso que nos importa resolver.
pub fn merge_manifest(mut user: Manifest, bundled: Manifest) -> Manifest {
    for entry in bundled.sounds {
        match user.sounds.iter_mut().find(|e| e.id == entry.id) {
            Some(e) => *e = entry,
            None => user.sounds.push(entry),
        }
    }
    user.version = user.version.max(bundled.version);
    user
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
    write_manifest_at(&manifest_path(), m)
}

/// Como `save_manifest`, pero a una ruta explícita en vez de `library_dir()`. La usa
/// `paths::seed_library` para no depender de `library_dir()`, que es quien dispara el seed.
pub fn write_manifest_at(path: &std::path::Path, m: &Manifest) -> Result<()> {
    write_atomic(path, &format!("{MANIFEST_HEADER}{}", toml::to_string_pretty(m)?))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, sha256: &str) -> ManifestEntry {
        ManifestEntry {
            id: id.into(),
            name: id.into(),
            theme: Theme::Nature,
            visual: None,
            url: "https://example.com".into(),
            file: format!("{id}.opus"),
            sha256: sha256.into(),
            duration: 100,
            bytes: 100,
            trim: None,
        }
    }

    #[test]
    fn manifest_without_version_field_parses_as_zero() {
        let m: Manifest = toml::from_str("[[sound]]\nid = \"a\"\nname = \"a\"\ntheme = \"nature\"\n\
            url = \"https://example.com\"\nfile = \"a.opus\"\nsha256 = \"old\"\nduration = 1\nbytes = 1\n")
            .unwrap();
        assert_eq!(m.version, 0);
    }

    #[test]
    fn merge_updates_bundled_entry_in_place_by_id() {
        let user = Manifest { version: 0, sounds: vec![entry("lluvia", "old-hash"), entry("chimenea", "c-hash")] };
        let bundled = Manifest { version: 1, sounds: vec![entry("lluvia", "new-hash")] };
        let merged = merge_manifest(user, bundled);
        // Same position, hash updated.
        assert_eq!(merged.sounds[0].id, "lluvia");
        assert_eq!(merged.sounds[0].sha256, "new-hash");
        assert_eq!(merged.sounds[1].id, "chimenea");
    }

    #[test]
    fn merge_preserves_user_added_entries_not_in_bundled() {
        let user = Manifest { version: 0, sounds: vec![entry("mio", "mi-hash")] };
        let bundled = Manifest { version: 1, sounds: vec![entry("lluvia", "new-hash")] };
        let merged = merge_manifest(user, bundled);
        assert!(merged.sounds.iter().any(|e| e.id == "mio" && e.sha256 == "mi-hash"));
    }

    #[test]
    fn merge_appends_new_bundled_entries() {
        let user = Manifest { version: 0, sounds: vec![entry("lluvia", "hash")] };
        let bundled = Manifest { version: 1, sounds: vec![entry("lluvia", "hash"), entry("chimenea", "c-hash")] };
        let merged = merge_manifest(user, bundled);
        assert_eq!(merged.sounds.len(), 2);
        assert_eq!(merged.sounds[1].id, "chimenea");
    }

    #[test]
    fn merge_bumps_version_to_bundled() {
        let user = Manifest { version: 0, sounds: vec![] };
        let bundled = Manifest { version: 3, sounds: vec![] };
        assert_eq!(merge_manifest(user, bundled).version, 3);
    }

    #[test]
    fn bundled_sounds_toml_parses_with_a_version() {
        let m: Manifest = toml::from_str(include_str!("../library/sounds.toml")).unwrap();
        assert!(m.version >= 1);
        assert!(!m.sounds.is_empty());
    }
}
