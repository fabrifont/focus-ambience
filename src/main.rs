mod audio;
mod client;
mod daemon;
mod download;
mod library;
mod paths;
mod protocol;
mod sync;
mod tui;

use anyhow::{Result, bail};

use crate::library::{Theme, Visual};
use crate::protocol::{AddRequest, Command};

const HELP: &str = "\
focus — ambientes para concentrarse

USO
  focus                      abre la interfaz
  focus play <preset>        carga un preset (sin abrir la interfaz)
  focus stop                 apaga todos los sonidos
  focus pause                pausa / reanuda
  focus timer <min|off>      temporizador con fade-out
  focus status               qué está sonando
  focus quit                 cierra el audio en background
  focus list                 sonidos y presets disponibles
  focus add <url> [opciones] baja el audio de un video de YouTube
      --name <nombre>        nombre a mostrar (por defecto, el título)
      --theme <tema>         skyrim | lotr | starwars | nature | other
      --visual <efecto>      rain | snow | fireflies | stars | embers | leaves
      --trim <min>           guarda solo los primeros N minutos
  focus sync [pull|push]     sincroniza los .opus con Google Drive (rclone)
  focus verify               compara los archivos con el manifest
";

fn main() {
    if let Err(e) = run() {
        eprintln!("focus: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        None => tui::run(),
        Some("daemon") => daemon::run(),
        Some("-h" | "--help" | "help") => {
            print!("{HELP}");
            Ok(())
        }
        Some("play") => {
            let name = rest.join(" ");
            if name.is_empty() {
                bail!("uso: focus play <preset>");
            }
            let state = client::oneshot(Some(Command::LoadPreset { name: name.clone() }), true)?;
            if !state.is_some_and(|s| s.presets.iter().any(|p| p.name == name)) {
                bail!("no existe el preset «{name}» (mirá `focus list`)");
            }
            Ok(())
        }
        Some("stop") => client::oneshot(Some(Command::StopAll), false).map(drop),
        Some("pause") => client::oneshot(Some(Command::PauseAll), false).map(drop),
        Some("quit") => {
            if client::oneshot(Some(Command::Quit), false)?.is_none() {
                println!("no había nada sonando");
            }
            Ok(())
        }
        Some("timer") => {
            let minutes = match rest.first().map(String::as_str) {
                Some("off") | None => None,
                Some(m) => Some(m.parse().map_err(|_| anyhow::anyhow!("minutos inválidos: {m}"))?),
            };
            client::oneshot(Some(Command::Timer { minutes }), true).map(drop)
        }
        Some("status") => status(),
        Some("list") => list(),
        Some("add") => add(rest),
        Some("sync") => sync::run(match rest.first().map(String::as_str) {
            Some("pull") => sync::Direction::Pull,
            Some("push") => sync::Direction::Push,
            _ => sync::Direction::Both,
        }),
        Some("verify") => sync::verify(),
        Some(other) => bail!("comando desconocido: {other}\n\n{HELP}"),
    }
}

fn status() -> Result<()> {
    let Some(state) = client::oneshot(None, false)? else {
        println!("silencio (el daemon no está corriendo)");
        return Ok(());
    };
    if state.active.is_empty() {
        println!("silencio");
    }
    for id in &state.active {
        let name = state.sounds.iter().find(|s| s.id == *id).map_or(id.as_str(), |s| s.name.as_str());
        let vol = state.volumes.get(id).copied().unwrap_or(0.0);
        println!("{} {name}  {:.0}%", if state.paused { "⏸" } else { "♫" }, vol * 100.0);
    }
    if let Some(t) = state.timer {
        println!("⏱ {}:{:02}", t.remaining_secs / 60, t.remaining_secs % 60);
    }
    Ok(())
}

fn list() -> Result<()> {
    let sounds = library::all_sounds()?;
    let mut theme = None;
    for s in &sounds {
        if theme != Some(s.theme) {
            theme = Some(s.theme);
            println!("\n{}", s.theme.label());
        }
        let extra = match s.duration {
            0 => String::new(),
            d => format!("  {}h{:02}", d / 3600, d / 60 % 60),
        };
        println!("  {}{:<44} {}{extra}", if s.available { " " } else { "☁" }, s.name, s.id);
    }
    let presets = library::load_presets();
    if !presets.presets.is_empty() {
        println!("\nPresets");
        for p in &presets.presets {
            println!("  {}", p.name);
        }
    }
    Ok(())
}

fn add(args: &[String]) -> Result<()> {
    let mut url = None;
    let mut req = AddRequest { url: String::new(), name: None, theme: Theme::Other, visual: None, trim: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| anyhow::anyhow!("falta el valor de {a}"));
        match a.as_str() {
            "--name" => req.name = Some(value()?),
            "--theme" => req.theme = value()?.parse()?,
            "--visual" => req.visual = Some(value()?.parse::<Visual>()?),
            "--trim" => req.trim = Some(value()?.parse()?),
            u if url.is_none() && !u.starts_with("--") => url = Some(u.to_string()),
            other => bail!("opción desconocida: {other}"),
        }
    }
    let Some(url) = url else { bail!("uso: focus add <url> [--name ..] [--theme ..] [--trim min]") };
    req.url = url;

    let mut last_stage = String::new();
    let entry = download::add(&req, |stage, p| {
        use std::io::Write;
        if stage != last_stage {
            if !last_stage.is_empty() {
                eprintln!();
            }
            last_stage = stage.to_string();
        }
        if p >= 0.0 {
            eprint!("\r  {stage:<14} {}", bar(p, 30));
        } else {
            eprint!("\r  {stage}…");
        }
        let _ = std::io::stderr().flush();
    })?;
    eprintln!();
    println!(
        "✓ {} ({}h{:02}, {:.0} MB) → {}",
        entry.name,
        entry.duration / 3600,
        entry.duration / 60 % 60,
        entry.bytes as f64 / 1e6,
        entry.id
    );
    let _ = client::oneshot(Some(Command::Reload), false);
    Ok(())
}

fn bar(p: f32, width: usize) -> String {
    let filled = (p.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{} {:>3.0}%", "━".repeat(filled), "─".repeat(width - filled), p * 100.0)
}
