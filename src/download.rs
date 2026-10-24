use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::library::{self, ManifestEntry};
use crate::paths;
use crate::protocol::AddRequest;

/// Seconds blended between the end and the start so the loop has no seam.
const SEAM: f64 = 3.0;

pub struct Meta {
    pub title: String,
}

pub fn fetch_meta(url: &str) -> Result<Meta> {
    let out = yt_dlp()
        .args(["--no-playlist", "--no-warnings", "--print", "%(title)s", url])
        .stderr(Stdio::piped())
        .output()
        .context("no se pudo ejecutar yt-dlp")?;
    if !out.status.success() {
        bail!("yt-dlp: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let title = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if title.is_empty() {
        bail!("yt-dlp no devolvió el título");
    }
    Ok(Meta { title })
}

/// "Skyrim Ambience - Study & Relaxation Music | 3 hours" -> "Skyrim Ambience - Study & Relaxation Music"
pub fn clean_title(t: &str) -> String {
    let t = t.split(['|', '【', '〖', '[']).next().unwrap_or(t);
    let t: String = t.chars().filter(|c| c.is_alphanumeric() || " -–,.'&:".contains(*c)).collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    t.trim_matches(|c: char| " -–,.:".contains(c)).to_string()
}

/// Download, trim, bake the loop seam, encode to Opus and add to the manifest.
/// `progress(stage, fraction)` is called as work advances; fraction < 0 means unknown.
pub fn add(req: &AddRequest, mut progress: impl FnMut(&str, f32)) -> Result<ManifestEntry> {
    progress("buscando video", -1.0);
    let meta = fetch_meta(&req.url)?;
    let name = req.name.clone().unwrap_or_else(|| clean_title(&meta.title));
    let mut id = library::slugify(&name);
    id.truncate(48);
    let id = id.trim_end_matches('-').to_string();
    if id.is_empty() {
        bail!("no pude armar un id a partir de '{name}'");
    }

    let dir = paths::sounds_dir();
    let tmp = dir.join(format!(".tmp-{id}"));
    std::fs::create_dir_all(&tmp)?;
    let result = (|| -> Result<ManifestEntry> {
        let src = download_audio(&req.url, &tmp, &mut progress)?;
        let mut len = probe_duration(&src)?;
        if let Some(m) = req.trim {
            len = len.min(m as f64 * 60.0);
        }
        let file = format!("{id}.opus");
        let partial = tmp.join(&file);
        encode(&src, &partial, len, &mut progress)?;
        if len > SEAM * 10.0 {
            len -= SEAM; // the seam crossfade overlaps tail and head
        }
        progress("verificando", -1.0);
        let (sha256, bytes) = hash_file(&partial)?;
        std::fs::rename(&partial, dir.join(&file))?;
        Ok(ManifestEntry {
            id: id.clone(),
            name: name.clone(),
            theme: req.theme,
            visual: req.visual,
            url: req.url.clone(),
            file,
            sha256,
            duration: len.round() as u64,
            bytes,
            trim: req.trim,
        })
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    let entry = result?;

    let mut manifest = library::load_manifest()?;
    manifest.sounds.retain(|e| e.id != entry.id);
    manifest.sounds.push(entry.clone());
    library::save_manifest(&manifest)?;
    Ok(entry)
}

/// yt-dlp needs a JS runtime to solve YouTube's challenges; without one, downloads get throttled to a crawl.
fn yt_dlp() -> Command {
    let mut cmd = Command::new(paths::tool("yt-dlp"));
    if let Some(node) = paths::js_runtime() {
        cmd.arg("--js-runtimes").arg(node);
    }
    cmd
}

/// Always the full audio: the native downloader is fast, while `--download-sections` goes through
/// ffmpeg and crawls. Trimming happens in `encode`.
fn download_audio(url: &str, tmp: &Path, progress: &mut impl FnMut(&str, f32)) -> Result<std::path::PathBuf> {
    progress("descargando", 0.0);
    let mut cmd = yt_dlp();
    cmd.args(["--no-playlist", "--no-warnings", "--newline", "-f", "bestaudio[acodec=opus]/bestaudio/best"])
        .args(["--progress-template", "download:PROG %(progress._percent_str)s"])
        .arg("--ffmpeg-location")
        .arg(paths::tool("ffmpeg"))
        .arg("-o")
        .arg(tmp.join("src.%(ext)s"));
    cmd.arg(url);
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().context("no se pudo ejecutar yt-dlp")?;
    let mut stderr = child.stderr.take().unwrap();
    let errs = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
        if let Some(p) = line.trim().strip_prefix("PROG")
            && let Ok(v) = p.trim().trim_end_matches('%').parse::<f32>()
        {
            progress("descargando", v / 100.0);
        }
    }
    let status = child.wait()?;
    let errs = errs.join().unwrap_or_default();
    if !status.success() {
        bail!("yt-dlp falló: {}", last_line(&errs));
    }
    std::fs::read_dir(tmp)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.file_stem().is_some_and(|s| s == "src"))
        .context("yt-dlp no dejó ningún archivo")
}

fn probe_duration(path: &Path) -> Result<f64> {
    let out = Command::new(paths::tool("ffprobe"))
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(path)
        .output()
        .context("no se pudo ejecutar ffprobe")?;
    String::from_utf8_lossy(&out.stdout).trim().parse().context("ffprobe no devolvió una duración")
}

fn encode(src: &Path, dst: &Path, len: f64, progress: &mut impl FnMut(&str, f32)) -> Result<()> {
    let bitrate = paths::config().bitrate.unwrap_or(paths::DEFAULT_BITRATE);
    let base = "aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo";
    let filter = if len > SEAM * 10.0 {
        // Output = [SEAM, len-SEAM] followed by tail crossfaded into head. It ends exactly where it
        // starts (original t=SEAM), so looping is seamless.
        let (x, end) = (SEAM, len);
        let mid_end = end - x;
        format!(
            "[0:a]{base},atrim=0:{end},asplit=3[a][b][c];\
             [a]atrim={x}:{mid_end},asetpts=PTS-STARTPTS[mid];\
             [b]atrim={mid_end}:{end},asetpts=PTS-STARTPTS[tail];\
             [c]atrim=0:{x},asetpts=PTS-STARTPTS[head];\
             [tail][head]acrossfade=d={x}:c1=qsin:c2=qsin[seam];\
             [mid][seam]concat=n=2:v=0:a=1[out]"
        )
    } else {
        format!("[0:a]{base},atrim=0:{len}[out]")
    };
    progress("convirtiendo", 0.0);
    let mut child = Command::new(paths::tool("ffmpeg"))
        .args(["-hide_banner", "-nostdin", "-nostats", "-loglevel", "error", "-y", "-i"])
        .arg(src)
        .args(["-filter_complex", &filter, "-map", "[out]", "-map_metadata", "-1"])
        .args(["-c:a", "libopus", "-b:a", &format!("{bitrate}k"), "-vbr", "on"])
        // 60 ms frames: fewer packets to decode at playback.
        .args(["-application", "audio", "-frame_duration", "60", "-progress", "pipe:1"])
        .arg(dst)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("no se pudo ejecutar ffmpeg")?;
    let mut stderr = child.stderr.take().unwrap();
    let errs = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
        if let Some(us) = line.strip_prefix("out_time_us=")
            && let Ok(us) = us.parse::<f64>()
        {
            progress("convirtiendo", (us / 1e6 / len).clamp(0.0, 1.0) as f32);
        }
    }
    let status = child.wait()?;
    let errs = errs.join().unwrap_or_default();
    if !status.success() {
        bail!("ffmpeg falló: {}", last_line(&errs));
    }
    Ok(())
}

pub fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        total += n as u64;
    }
    let hex = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok((hex, total))
}

fn last_line(s: &str) -> String {
    s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("error desconocido").trim().to_string()
}
