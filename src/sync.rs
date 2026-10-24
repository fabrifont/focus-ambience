use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::download::hash_file;
use crate::library;
use crate::paths;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Pull,
    Push,
    Both,
}

fn remote() -> String {
    paths::config().remote.unwrap_or_else(|| paths::DEFAULT_REMOTE.to_string())
}

/// `only`: copy just these files instead of every .opus.
fn rclone(from: &str, to: &str, only: Option<&[String]>) -> Result<()> {
    println!("  {from}  →  {to}");
    let mut cmd = Command::new(paths::tool("rclone"));
    cmd.args(["copy", from, to, "--progress", "--stats-one-line"]);
    match only {
        Some(files) => files.iter().fold(&mut cmd, |c, f| c.arg("--include").arg(f)),
        None => cmd.args(["--include", "*.opus"]),
    };
    let status = cmd
        .status()
        .context("no se pudo ejecutar rclone (¿está instalado?)")?;
    if !status.success() {
        bail!(
            "rclone falló. Si es la primera vez, configurá Google Drive con `rclone config` \
             (remote llamado \"gdrive\") o cambiá `remote` en {}",
            paths::config_dir().join("config.toml").display()
        );
    }
    Ok(())
}

pub fn run(dir: Direction) -> Result<()> {
    let local = paths::sounds_dir();
    std::fs::create_dir_all(&local)?;
    let local = local.to_string_lossy().to_string();
    let remote = remote();
    if dir != Direction::Push {
        // Only what's missing or doesn't match the manifest, so an old copy on the remote
        // never overwrites a newer local one.
        let wanted = outdated()?;
        if wanted.is_empty() {
            println!("⇣ no falta bajar nada");
        } else {
            println!("⇣ bajando sonidos que falten");
            rclone(&remote, &local, Some(&wanted))?;
        }
    }
    if dir != Direction::Pull {
        println!("⇡ subiendo sonidos nuevos");
        rclone(&local, &remote, None)?;
    }
    verify()
}

/// Files in the manifest that are missing locally or whose hash differs.
fn outdated() -> Result<Vec<String>> {
    let dir = paths::sounds_dir();
    Ok(library::load_manifest()?
        .sounds
        .into_iter()
        .filter(|e| hash_file(&dir.join(&e.file)).ok().is_none_or(|(h, _)| h != e.sha256))
        .map(|e| e.file)
        .collect())
}

/// Compare what's on disk with the manifest.
pub fn verify() -> Result<()> {
    let manifest = library::load_manifest()?;
    let dir = paths::sounds_dir();
    let (mut ok, mut missing, mut bad) = (0, Vec::new(), Vec::new());
    for e in &manifest.sounds {
        let path = dir.join(&e.file);
        if !path.exists() {
            missing.push(&e.name);
            continue;
        }
        match hash_file(&path) {
            Ok((h, _)) if h == e.sha256 => ok += 1,
            _ => bad.push(&e.name),
        }
    }
    println!("✓ {ok} sonidos verificados");
    for n in &missing {
        println!("  ☁ falta: {n}  (no está ni local ni en el remoto; volvé a bajarlo con focus add)");
    }
    for n in &bad {
        println!("  ✗ corrupto: {n}");
    }
    Ok(())
}
