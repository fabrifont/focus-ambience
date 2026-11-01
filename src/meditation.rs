//! Biblioteca de meditaciones guiadas: manifest propio (`library/meditations.toml`), separado de
//! los sonidos. Cada una tiene una categoría de duración fija elegida a mano.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::library::write_atomic;
use crate::paths;

/// Minutos de cada categoría, en el orden en que se muestran.
pub const CATEGORY_MINUTES: [u32; 8] = [5, 10, 15, 20, 25, 30, 40, 60];

/// Categoría de duración de una meditación. Solo existen las de `CATEGORY_MINUTES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Category(u32);

impl Category {
    pub const ALL: [Category; 8] = {
        let mut all = [Category(0); 8];
        let mut i = 0;
        while i < all.len() {
            all[i] = Category(CATEGORY_MINUTES[i]);
            i += 1;
        }
        all
    };

    pub fn minutes(self) -> u32 {
        self.0
    }

    /// "5 minutos", "1 hora".
    pub fn label(self) -> String {
        match self.0 {
            60 => "1 hora".to_string(),
            m => format!("{m} minutos"),
        }
    }
}

impl TryFrom<u32> for Category {
    type Error = anyhow::Error;
    fn try_from(minutes: u32) -> Result<Self> {
        if CATEGORY_MINUTES.contains(&minutes) {
            Ok(Category(minutes))
        } else {
            bail!("categoría inválida: {minutes} min (las válidas son 5, 10, 15, 20, 25, 30, 40 o 60)")
        }
    }
}

impl From<Category> for u32 {
    fn from(c: Category) -> u32 {
        c.0
    }
}

/// Una entrada de library/meditations.toml.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeditationEntry {
    pub id: String,
    pub name: String,
    /// Categoría de duración, elegida a mano (no es la duración real del audio).
    pub minutes: Category,
    pub url: String,
    pub file: String,
    pub sha256: String,
    /// Segundos reales del audio.
    pub duration: u64,
    pub bytes: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MeditationManifest {
    #[serde(default, rename = "meditation")]
    pub meditations: Vec<MeditationEntry>,
}

/// Una meditación como la ven el daemon y la TUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meditation {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub file: String,
    /// Segundos reales del audio.
    pub duration: u64,
    /// False si está en el manifest pero el .opus todavía no está en esta compu.
    pub available: bool,
}

const HEADER: &str = "# Meditaciones guiadas. Los .opus viven en meditations/ (no versionados) y se\n\
# sincronizan con `focus sync`. Este archivo sí va en git.\n\
# `minutes` es la categoría: 5, 10, 15, 20, 25, 30, 40 o 60.\n\n";

fn manifest_path() -> PathBuf {
    paths::library_dir().join("meditations.toml")
}

pub fn parse_manifest(s: &str) -> Result<MeditationManifest> {
    Ok(toml::from_str(s)?)
}

pub fn load_manifest() -> Result<MeditationManifest> {
    let path = manifest_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => parse_manifest(&s).with_context(|| format!("leyendo {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(MeditationManifest::default()),
        Err(e) => Err(e).with_context(|| format!("leyendo {}", path.display())),
    }
}

pub fn save_manifest(m: &MeditationManifest) -> Result<()> {
    write_atomic(&manifest_path(), &format!("{HEADER}{}", toml::to_string_pretty(m)?))
}

/// Agrega la entrada o, si ya existía una con ese id, la reemplaza en el mismo lugar.
pub fn upsert(m: &mut MeditationManifest, entry: MeditationEntry) {
    match m.meditations.iter_mut().find(|e| e.id == entry.id) {
        Some(e) => *e = entry,
        None => m.meditations.push(entry),
    }
}

/// Todas las meditaciones, ordenadas por categoría y después por nombre.
pub fn all_meditations() -> Result<Vec<Meditation>> {
    let dir = paths::meditations_dir();
    let mut v: Vec<Meditation> = load_manifest()?
        .meditations
        .into_iter()
        .map(|e| Meditation {
            available: dir.join(&e.file).exists(),
            id: e.id,
            name: e.name,
            category: e.minutes,
            file: e.file,
            duration: e.duration,
        })
        .collect();
    sort(&mut v);
    Ok(v)
}

fn sort(v: &mut [Meditation]) {
    v.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

/// Agrupa por categoría, en orden, y solo devuelve las que tienen al menos una meditación.
pub fn group(items: &[Meditation]) -> Vec<(Category, Vec<&Meditation>)> {
    Category::ALL
        .iter()
        .filter_map(|&c| {
            let in_cat: Vec<&Meditation> = items.iter().filter(|m| m.category == c).collect();
            (!in_cat.is_empty()).then_some((c, in_cat))
        })
        .collect()
}

/// Las meditaciones en el orden en que las muestra la lista agrupada.
pub fn display_order(items: &[Meditation]) -> Vec<&Meditation> {
    group(items).into_iter().flat_map(|(_, v)| v).collect()
}

/// Elige una meditación al azar puro (puede repetirse) de la categoría. Solo cuentan las que
/// ya están en esta compu. `None` si la categoría no tiene ninguna.
pub fn pick_random<'a>(items: &'a [Meditation], category: Category, rng: &mut fastrand::Rng) -> Option<&'a Meditation> {
    let candidates: Vec<&Meditation> = items.iter().filter(|m| m.category == category && m.available).collect();
    if candidates.is_empty() {
        return None;
    }
    Some(candidates[rng.usize(..candidates.len())])
}

/// Busca por id o por nombre (sin distinguir mayúsculas ni acentos). Si no hay coincidencia
/// exacta, acepta un fragmento que identifique a una sola.
pub fn find<'a>(items: &'a [Meditation], query: &str) -> Result<&'a Meditation> {
    let q = crate::library::slugify(query);
    if q.is_empty() {
        bail!("falta el nombre de la meditación");
    }
    if let Some(m) = items.iter().find(|m| m.id == q || crate::library::slugify(&m.name) == q) {
        return Ok(m);
    }
    let partial: Vec<&Meditation> =
        items.iter().filter(|m| m.id.contains(&q) || crate::library::slugify(&m.name).contains(&q)).collect();
    match partial.as_slice() {
        [one] => Ok(one),
        [] => bail!("no existe la meditación «{query}» (mirá `focus meditacion listar`)"),
        many => {
            let names: Vec<&str> = many.iter().map(|m| m.name.as_str()).collect();
            bail!("«{query}» es ambiguo, puede ser: {}", names.join(", "))
        }
    }
}

/// "03:12" o "1:05:00" a partir de segundos.
pub fn clock(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn med(id: &str, minutes: u32, available: bool) -> Meditation {
        Meditation {
            id: id.into(),
            name: id.replace('-', " "),
            category: Category::try_from(minutes).unwrap(),
            file: format!("{id}.opus"),
            duration: minutes as u64 * 60,
            available,
        }
    }

    #[test]
    fn only_the_fixed_categories_are_valid() {
        for m in [5, 10, 15, 20, 25, 30, 40, 60] {
            assert_eq!(Category::try_from(m).unwrap().minutes(), m);
        }
        for m in [0, 1, 7, 35, 45, 50, 59, 61, 90, 120] {
            assert!(Category::try_from(m).is_err(), "{m} no debería ser válida");
        }
    }

    #[test]
    fn labels_are_in_spanish() {
        assert_eq!(Category::try_from(5).unwrap().label(), "5 minutos");
        assert_eq!(Category::try_from(40).unwrap().label(), "40 minutos");
        assert_eq!(Category::try_from(60).unwrap().label(), "1 hora");
    }

    #[test]
    fn manifest_roundtrips() {
        let m = MeditationManifest {
            meditations: vec![MeditationEntry {
                id: "calma-guiada".into(),
                name: "Calma guiada".into(),
                minutes: Category::try_from(10).unwrap(),
                url: "https://example.com/v".into(),
                file: "calma-guiada.opus".into(),
                sha256: "abc".into(),
                duration: 611,
                bytes: 1234,
            }],
        };
        let back = parse_manifest(&toml::to_string_pretty(&m).unwrap()).unwrap();
        assert_eq!(back.meditations, m.meditations);
    }

    #[test]
    fn manifest_rejects_an_invalid_category() {
        let bad = "[[meditation]]\nid = \"a\"\nname = \"a\"\nminutes = 12\nurl = \"u\"\nfile = \"a.opus\"\n\
                   sha256 = \"x\"\nduration = 1\nbytes = 1\n";
        assert!(parse_manifest(bad).is_err());
    }

    #[test]
    fn empty_manifest_parses_to_no_meditations() {
        assert!(parse_manifest("").unwrap().meditations.is_empty());
    }

    #[test]
    fn bundled_meditations_toml_parses() {
        parse_manifest(include_str!("../library/meditations.toml")).unwrap();
    }

    #[test]
    fn upsert_replaces_in_place_and_appends_new() {
        let entry = |id: &str, sha: &str| MeditationEntry {
            id: id.into(),
            name: id.into(),
            minutes: Category::try_from(5).unwrap(),
            url: String::new(),
            file: format!("{id}.opus"),
            sha256: sha.into(),
            duration: 1,
            bytes: 1,
        };
        let mut m = MeditationManifest::default();
        upsert(&mut m, entry("a", "1"));
        upsert(&mut m, entry("b", "1"));
        upsert(&mut m, entry("a", "2"));
        assert_eq!(m.meditations.len(), 2);
        assert_eq!(m.meditations[0].sha256, "2");
        assert_eq!(m.meditations[1].id, "b");
    }

    #[test]
    fn group_orders_by_category_and_skips_empty_ones() {
        let mut items = vec![med("c", 20, true), med("a", 5, true), med("b", 20, true), med("d", 60, true)];
        sort(&mut items);
        let g = group(&items);
        let cats: Vec<u32> = g.iter().map(|(c, _)| c.minutes()).collect();
        assert_eq!(cats, vec![5, 20, 60]);
        let names: Vec<&str> = g[1].1.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(names, vec!["b", "c"]);
    }

    #[test]
    fn group_of_nothing_is_empty() {
        assert!(group(&[]).is_empty());
    }

    #[test]
    fn display_order_follows_the_groups() {
        let mut items = vec![med("z", 30, true), med("y", 10, true)];
        sort(&mut items);
        let ids: Vec<&str> = display_order(&items).iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["y", "z"]);
    }

    #[test]
    fn pick_random_stays_in_the_category_and_only_picks_available() {
        let items = vec![med("a", 10, true), med("b", 10, true), med("c", 10, false), med("d", 20, true)];
        let ten = Category::try_from(10).unwrap();
        let mut rng = fastrand::Rng::with_seed(7);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let m = pick_random(&items, ten, &mut rng).unwrap();
            assert!(m.id == "a" || m.id == "b", "salió {}", m.id);
            seen.insert(m.id.clone());
        }
        assert_eq!(seen.len(), 2, "con 200 tiradas tienen que salir las dos");
    }

    #[test]
    fn pick_random_can_repeat_and_handles_a_single_candidate() {
        let items = vec![med("solo", 15, true)];
        let c = Category::try_from(15).unwrap();
        let mut rng = fastrand::Rng::with_seed(1);
        for _ in 0..5 {
            assert_eq!(pick_random(&items, c, &mut rng).unwrap().id, "solo");
        }
    }

    #[test]
    fn pick_random_without_candidates_is_none() {
        let items = vec![med("a", 10, false), med("b", 20, true)];
        let mut rng = fastrand::Rng::with_seed(3);
        assert!(pick_random(&items, Category::try_from(10).unwrap(), &mut rng).is_none());
        assert!(pick_random(&items, Category::try_from(5).unwrap(), &mut rng).is_none());
        assert!(pick_random(&[], Category::try_from(5).unwrap(), &mut rng).is_none());
    }

    #[test]
    fn find_by_id_name_or_unique_fragment() {
        let items =
            vec![med("calma-guiada", 10, true), med("escaneo-corporal", 10, true), med("calma-total", 20, true)];
        assert_eq!(find(&items, "calma-guiada").unwrap().id, "calma-guiada");
        assert_eq!(find(&items, "Escaneo Corporal").unwrap().id, "escaneo-corporal");
        assert_eq!(find(&items, "escaneo").unwrap().id, "escaneo-corporal");
        assert!(find(&items, "calma").unwrap_err().to_string().contains("ambiguo"));
        assert!(find(&items, "nada").is_err());
        assert!(find(&items, "  ").is_err());
    }

    #[test]
    fn clock_formats_minutes_and_hours() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(192), "03:12");
        assert_eq!(clock(3600), "1:00:00");
        assert_eq!(clock(3905), "1:05:05");
    }
}
