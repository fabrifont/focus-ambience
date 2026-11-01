use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::download::hash_file;
use crate::library;
use crate::meditation;
use crate::paths;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Pull,
    Push,
    Both,
}

pub fn remote() -> String {
    paths::config().remote.unwrap_or_else(|| paths::DEFAULT_REMOTE.to_string())
}

pub fn meditations_remote() -> String {
    paths::config().meditations_remote.unwrap_or_else(|| sibling_remote(&remote(), "meditations"))
}

/// A folder named `name` next to the last folder of `remote`:
/// "gdrive:focus-ambience/sounds" -> "gdrive:focus-ambience/meditations".
fn sibling_remote(remote: &str, name: &str) -> String {
    let remote = remote.trim_end_matches('/');
    match remote.rfind(['/', ':']) {
        Some(i) => format!("{}{name}", &remote[..=i]),
        None => name.to_string(),
    }
}

/// Lists that travel with the audio, relative to the library folder. They live only in Drive
/// (and each machine's library folder), never in git.
const DATA_FILES: [&str; 4] = ["sounds.toml", "presets.toml", "meditations.toml", "routines.toml"];

pub fn data_remote() -> String {
    sibling_remote(&remote(), "library")
}

/// One library folder and its remote counterpart, with what its manifest expects inside.
struct Set {
    label: &'static str,
    local: std::path::PathBuf,
    remote: String,
    /// (display name, file, sha256) per manifest entry.
    entries: Vec<(String, String, String)>,
}

fn sets() -> Result<[Set; 2]> {
    Ok([
        Set {
            label: "sonidos",
            local: paths::sounds_dir(),
            remote: remote(),
            entries: library::load_manifest()?.sounds.into_iter().map(|e| (e.name, e.file, e.sha256)).collect(),
        },
        Set {
            label: "meditaciones",
            local: paths::meditations_dir(),
            remote: meditations_remote(),
            entries: meditation::load_manifest()?.meditations.into_iter().map(|e| (e.name, e.file, e.sha256)).collect(),
        },
    ])
}

/// `only`: copy just these files instead of every .opus.
/// `update`: skip files that are newer at the destination.
fn rclone(from: &str, to: &str, only: Option<&[String]>, update: bool) -> Result<()> {
    println!("  {from}  →  {to}");
    let mut cmd = Command::new(paths::tool("rclone"));
    cmd.args(["copy", from, to, "--progress", "--stats-one-line"]);
    if update {
        cmd.arg("--update");
    }
    match only {
        Some(files) => files.iter().fold(&mut cmd, |c, f| c.arg("--include").arg(f)),
        None => cmd.args(["--include", "*.opus"]),
    };
    let status = cmd.status().context("no se pudo ejecutar rclone (¿está instalado?)")?;
    if !status.success() {
        bail!(
            "rclone falló. Si cambiaste de cuenta o el login venció, probá `focus setup`; \
             el remote se elige con `remote` en {}",
            paths::config_dir().join("config.toml").display()
        );
    }
    Ok(())
}

/// Copy the lists; the newer copy wins (`--update`), and a file missing on the source side is
/// simply not copied, so an empty remote never overwrites a local list and vice versa.
fn sync_data(from: &str, to: &str) -> Result<()> {
    let files = DATA_FILES.map(String::from);
    rclone(from, to, Some(&files), true)
}

pub fn run(dir: Direction) -> Result<()> {
    crate::setup::prepare()?;
    let library = paths::library_dir();
    std::fs::create_dir_all(&library)?;
    let data_local = library.to_string_lossy().into_owned();
    // Before reading the manifests: a pulled sounds.toml says which .opus to fetch.
    if dir != Direction::Push {
        println!("⇣ bajando las listas (sonidos, presets, meditaciones, rutinas)");
        let pulled = sync_data(&data_remote(), &data_local);
        // First sync: the remote folder doesn't exist yet. Keep going so the push below creates
        // it with the local lists.
        match pulled {
            Err(e) if dir == Direction::Both => println!("  (no pude bajar las listas: {e}; sigo con lo local)"),
            other => other?,
        }
    }
    let sets = sets()?;
    for set in &sets {
        std::fs::create_dir_all(&set.local)?;
    }
    if dir != Direction::Push {
        for set in &sets {
            // Only what's missing or doesn't match the manifest, so an old copy on the remote
            // never overwrites a newer local one.
            let wanted = outdated(set);
            if wanted.is_empty() {
                println!("⇣ no falta bajar nada de {}", set.label);
            } else {
                println!("⇣ bajando {} que falten", set.label);
                rclone(&set.remote, &set.local.to_string_lossy(), Some(&wanted), false)?;
            }
        }
    }
    if dir != Direction::Pull {
        for set in &sets {
            println!("⇡ subiendo {} nuevos", set.label);
            rclone(&set.local.to_string_lossy(), &set.remote, None, false)?;
        }
        println!("⇡ subiendo las listas (gana la más nueva)");
        sync_data(&data_local, &data_remote())?;
    }
    verify()
}

/// Files in the manifest that are missing locally or whose hash differs.
fn outdated(set: &Set) -> Vec<String> {
    set.entries
        .iter()
        .filter(|(_, file, sha)| hash_file(&set.local.join(file)).ok().is_none_or(|(h, _)| h != *sha))
        .map(|(_, file, _)| file.clone())
        .collect()
}

/// Compare what's on disk with the manifests.
pub fn verify() -> Result<()> {
    for set in sets()? {
        let (mut ok, mut missing, mut bad) = (0, Vec::new(), Vec::new());
        for (name, file, sha) in &set.entries {
            let path = set.local.join(file);
            if !path.exists() {
                missing.push(name);
                continue;
            }
            match hash_file(&path) {
                Ok((h, _)) if h == *sha => ok += 1,
                _ => bad.push(name),
            }
        }
        println!("✓ {ok} {} verificad{}", set.label, if set.label == "sonidos" { "os" } else { "as" });
        for n in &missing {
            println!("  ☁ falta: {n}  (no está ni local ni en el remoto; volvé a bajarlo con focus add)");
        }
        for n in &bad {
            println!("  ✗ corrupto: {n}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meditations_remote_sits_next_to_the_sounds_one() {
        assert_eq!(sibling_remote("gdrive:focus-ambience/sounds", "meditations"), "gdrive:focus-ambience/meditations");
        assert_eq!(sibling_remote("gdrive:focus-ambience/sounds/", "meditations"), "gdrive:focus-ambience/meditations");
        assert_eq!(sibling_remote("gdrive:sonidos", "meditations"), "gdrive:meditations");
        assert_eq!(sibling_remote("gdrive:", "meditations"), "gdrive:meditations");
        assert_eq!(sibling_remote("/mnt/backup/sounds", "meditations"), "/mnt/backup/meditations");
    }

    #[test]
    fn data_files_go_to_a_library_folder_next_to_the_sounds() {
        assert_eq!(sibling_remote("gdrive:focus-ambience/sounds", "library"), "gdrive:focus-ambience/library");
        for f in ["sounds.toml", "presets.toml", "meditations.toml", "routines.toml"] {
            assert!(DATA_FILES.contains(&f), "{f}");
        }
    }

    #[test]
    fn outdated_lists_missing_and_mismatching_files() {
        let dir = std::env::temp_dir().join(format!("focus-sync-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ok.opus"), b"hola").unwrap();
        std::fs::write(dir.join("viejo.opus"), b"otra cosa").unwrap();
        let (sha_ok, _) = hash_file(&dir.join("ok.opus")).unwrap();
        let entry = |f: &str, sha: &str| (f.to_string(), f.to_string(), sha.to_string());
        let set = Set {
            label: "meditaciones",
            local: dir.clone(),
            remote: String::new(),
            entries: vec![entry("ok.opus", &sha_ok), entry("viejo.opus", "otro-hash"), entry("falta.opus", "x")],
        };
        assert_eq!(outdated(&set), vec!["viejo.opus", "falta.opus"]);
        std::fs::remove_dir_all(dir).ok();
    }
}
