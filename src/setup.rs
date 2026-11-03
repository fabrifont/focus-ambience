//! First-run preparation for `focus sync`: make sure rclone is installed and the Google Drive
//! remote exists. Uses the system's curl and unzip/tar instead of an HTTP/zip crate (both ship
//! with Windows 10 1803+, and Linux Mint has them), to keep the dependency tree small.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::download::hash_file;
use crate::paths;
use crate::progress::Out;

const BASE_URL: &str = "https://downloads.rclone.org";

/// Name of the rclone remote (the part before ':') or None if `remote` is a plain local path
/// (no ':', a Windows drive letter like `C:\x`, or a `:backend:` connection string).
pub fn remote_name(remote: &str) -> Option<&str> {
    let (name, _) = remote.split_once(':')?;
    let plain = name.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '));
    (name.chars().count() > 1 && plain).then_some(name)
}

/// Remote names from `rclone listremotes` output (one `name:` per line).
pub fn parse_listremotes(out: &str) -> Vec<&str> {
    out.lines().filter_map(|l| l.trim().strip_suffix(':')).filter(|n| !n.is_empty()).collect()
}

/// (os, arch) as rclone names them in its download file names.
pub fn platform(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    let os = match os {
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match arch {
        "x86_64" => "amd64",
        "x86" => "386",
        "aarch64" => "arm64",
        _ => return None,
    };
    Some((os, arch))
}

/// Versioned zip name, e.g. `rclone-v1.75.2-linux-amd64.zip`.
pub fn zip_name(version: &str, os: &str, arch: &str) -> String {
    format!("rclone-{version}-{os}-{arch}.zip")
}

/// `rclone v1.75.2` (contents of version.txt) → `v1.75.2`.
pub fn parse_version(txt: &str) -> Option<&str> {
    let v = txt.split_whitespace().nth(1)?;
    let digits = v.strip_prefix('v')?;
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '.')).then_some(v)
}

pub fn zip_url(version: &str, os: &str, arch: &str) -> String {
    format!("{BASE_URL}/{version}/{}", zip_name(version, os, arch))
}

/// Looks `file` up in a SHA256SUMS file (`<hex>  <name>` lines, possibly PGP-clearsigned).
pub fn expected_hash(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (hash, name) = (it.next()?, it.next()?);
        let valid = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
        (valid && name == file && it.next().is_none()).then(|| hash.to_ascii_lowercase())
    })
}

fn curl(url: &str, dest: &Path, out: &mut Out) -> Result<()> {
    let status = out
        .run(
            Command::new("curl")
                .args(["--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--output"])
                .arg(dest)
                .arg(url),
        )
        .context("no se pudo ejecutar curl (necesito curl para bajar rclone)")?;
    if !status.success() {
        bail!("no pude bajar {url}");
    }
    Ok(())
}

fn rclone_works() -> bool {
    Command::new(paths::tool("rclone"))
        .arg("version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Unzip into `dir` (empty). Linux: unzip; Windows: bsdtar (tar.exe) reads zips.
fn extract(zip: &Path, dir: &Path, out: &mut Out) -> Result<()> {
    let mut cmd;
    if cfg!(windows) {
        cmd = Command::new("tar");
        cmd.arg("-xf").arg(zip).arg("-C").arg(dir);
    } else {
        cmd = Command::new("unzip");
        cmd.arg("-q").arg(zip).arg("-d").arg(dir);
    }
    let tool = if cfg!(windows) { "tar" } else { "unzip" };
    let status = out
        .run(&mut cmd)
        .with_context(|| format!("no se pudo ejecutar {tool} (necesito {tool} para instalar rclone)"))?;
    if !status.success() {
        bail!("no pude descomprimir {}", zip.display());
    }
    Ok(())
}

/// Downloads rclone, checks its SHA256 against the official sums for that exact version, and
/// installs the binary in `paths::bin_dir()`.
fn install_rclone(out: &mut Out) -> Result<()> {
    let (os, arch) = platform(std::env::consts::OS, std::env::consts::ARCH).with_context(|| {
        format!("no sé bajar rclone para {}/{}; instalalo a mano", std::env::consts::OS, std::env::consts::ARCH)
    })?;
    let work = std::env::temp_dir().join(format!("focus-rclone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let result = install_in(&work, os, arch, out);
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn install_in(work: &Path, os: &str, arch: &str, out: &mut Out) -> Result<()> {
    out.line("rclone no está instalado: bajándolo de downloads.rclone.org...");
    let version_file = work.join("version.txt");
    curl(&format!("{BASE_URL}/version.txt"), &version_file, out)?;
    let text = std::fs::read_to_string(&version_file)?;
    let version = parse_version(&text).context("version.txt de rclone con formato inesperado")?;

    let zip = work.join("rclone.zip");
    let sums = work.join("SHA256SUMS");
    curl(&zip_url(version, os, arch), &zip, out)?;
    curl(&format!("{BASE_URL}/{version}/SHA256SUMS"), &sums, out)?;

    let name = zip_name(version, os, arch);
    let want = expected_hash(&std::fs::read_to_string(&sums)?, &name)
        .with_context(|| format!("{name} no figura en el SHA256SUMS oficial"))?;
    let (got, _) = hash_file(&zip)?;
    if got != want {
        bail!("el SHA256 de {name} no coincide con el oficial; no lo instalo");
    }
    out.line(format!("✓ SHA256 verificado ({version})"));

    let unzipped = work.join("out");
    std::fs::create_dir(&unzipped)?;
    extract(&zip, &unzipped, out)?;
    let exe = format!("rclone{}", std::env::consts::EXE_SUFFIX);
    let built = unzipped.join(name.trim_end_matches(".zip")).join(&exe);
    if !built.is_file() {
        bail!("el zip de rclone no trae {exe}");
    }
    let bin = paths::bin_dir();
    std::fs::create_dir_all(&bin)?;
    // Copy to a temp name and rename so a half-written file is never taken for rclone.
    let dest: PathBuf = bin.join(&exe);
    let partial = bin.join(format!("{exe}.part"));
    std::fs::copy(&built, &partial)?;
    std::fs::rename(&partial, &dest)?;
    out.line(format!("✓ rclone instalado en {}", dest.display()));
    Ok(())
}

/// Install rclone if missing and create the Drive remote if it doesn't exist. Idempotent.
pub fn ensure_rclone(out: &mut Out) -> Result<()> {
    if rclone_works() {
        return Ok(());
    }
    install_rclone(out)?;
    if !rclone_works() {
        bail!("instalé rclone pero no se puede ejecutar");
    }
    Ok(())
}

/// Every rclone remote `focus sync` talks to (sounds, meditations and library), once each.
pub fn ensure_remote(out: &mut Out) -> Result<()> {
    let mut names: Vec<String> = Vec::new();
    for remote in [crate::sync::remote(), crate::sync::meditations_remote(), crate::sync::data_remote()] {
        if let Some(name) = remote_name(&remote)
            && !names.iter().any(|n| n == name)
        {
            names.push(name.to_string());
        }
    }
    names.iter().try_for_each(|name| ensure_one_remote(name, out))
}

fn ensure_one_remote(name: &str, out: &mut Out) -> Result<()> {
    let listed =
        Command::new(paths::tool("rclone")).arg("listremotes").output().context("no se pudo ejecutar rclone")?;
    if !listed.status.success() {
        bail!("rclone listremotes falló: {}", String::from_utf8_lossy(&listed.stderr).trim());
    }
    if parse_listremotes(&String::from_utf8_lossy(&listed.stdout)).contains(&name) {
        return Ok(());
    }
    out.line(format!("Abriendo el navegador para que inicies sesión en Google (remote «{name}»)..."));
    out.line("  (solo va a poder ver lo que suba focus: permiso drive.file)");
    let status = out
        .run(Command::new(paths::tool("rclone")).args(["config", "create", name, "drive", "scope=drive.file"]))
        .context("no se pudo ejecutar rclone")?;
    if !status.success() {
        bail!("no se pudo configurar Google Drive; probá de nuevo con `focus setup`");
    }
    out.line("✓ Google Drive configurado");
    Ok(())
}

/// Everything `focus sync` needs; quiet when there's nothing to do.
pub fn prepare(out: &mut Out) -> Result<()> {
    ensure_rclone(out)?;
    ensure_remote(out)
}

pub fn run(out: &mut Out) -> Result<()> {
    prepare(out)?;
    out.line("✓ todo listo para `focus sync`");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMS: &str = "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA1\n\n\
349ac8fba6ff65d6247043f1750cdcb518ec5d500ef91463a10d37c0ccdf3702  rclone-v1.75.2-linux-amd64.zip\n\
184ca0903eede5f627ed10f5b19c5e087273493285572e289e1561db496fbc98  rclone-v1.75.2-linux-arm.zip\n\
7e1e8d69654941b7b7df84ee74c5f7fb09cce7d5496947fb7bf16b55ff427d10  rclone-v1.75.2-linux-arm64.zip\n\
-----BEGIN PGP SIGNATURE-----\n\niQIzBAEBCAAdFiEE\n-----END PGP SIGNATURE-----\n";

    #[test]
    fn picks_platform() {
        assert_eq!(platform("linux", "x86_64"), Some(("linux", "amd64")));
        assert_eq!(platform("windows", "aarch64"), Some(("windows", "arm64")));
        assert_eq!(platform("windows", "x86"), Some(("windows", "386")));
        assert_eq!(platform("macos", "x86_64"), None);
        assert_eq!(platform("linux", "riscv64"), None);
    }

    #[test]
    fn builds_url() {
        assert_eq!(
            zip_url("v1.75.2", "windows", "amd64"),
            "https://downloads.rclone.org/v1.75.2/rclone-v1.75.2-windows-amd64.zip"
        );
    }

    #[test]
    fn parses_version() {
        assert_eq!(parse_version("rclone v1.75.2\n"), Some("v1.75.2"));
        assert_eq!(parse_version("rclone v1.75.2-beta\n"), None);
        assert_eq!(parse_version("<html>"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn finds_hash_exact_name() {
        assert!(expected_hash(SUMS, "rclone-v1.75.2-linux-amd64.zip").unwrap().starts_with("349ac8fb"));
        // arm must not match arm64 (and vice versa)
        assert!(expected_hash(SUMS, "rclone-v1.75.2-linux-arm.zip").unwrap().starts_with("184ca090"));
        assert!(expected_hash(SUMS, "rclone-v1.75.2-linux-arm64.zip").unwrap().starts_with("7e1e8d69"));
        assert_eq!(expected_hash(SUMS, "rclone-v1.75.2-windows-amd64.zip"), None);
        assert_eq!(expected_hash("zzzz  rclone-v1.zip", "rclone-v1.zip"), None);
    }

    #[test]
    fn parses_listremotes() {
        assert_eq!(parse_listremotes("gdrive:\nother:\n"), ["gdrive", "other"]);
        assert!(parse_listremotes("").is_empty());
    }

    #[test]
    fn extracts_remote_name() {
        assert_eq!(remote_name("gdrive:focus-ambience/sounds"), Some("gdrive"));
        assert_eq!(remote_name("mi drive:x"), Some("mi drive"));
        assert_eq!(remote_name("/mnt/backup/sounds"), None);
        assert_eq!(remote_name(r"C:\sounds"), None);
        assert_eq!(remote_name(":s3,env_auth:bucket"), None);
    }
}
