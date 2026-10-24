use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Command as Process, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::paths;
use crate::protocol::{Command, Event};
#[cfg(windows)]
use uds_windows::UnixStream;

pub fn connect() -> Option<UnixStream> {
    UnixStream::connect(paths::socket_path()).ok()
}

/// Connect to the daemon, starting it in the background if it isn't running.
pub fn connect_or_spawn() -> Result<UnixStream> {
    if let Some(s) = connect() {
        return Ok(s);
    }
    std::fs::create_dir_all(paths::state_dir())?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(paths::log_file())?;
    let mut cmd = Process::new(std::env::current_exe()?);
    cmd.arg("daemon").stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
    // New session: no controlling terminal, so closing it doesn't take the audio with it.
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    // Same on Windows: no console attached, own process group (Ctrl+C doesn't reach it).
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().context("no pude lanzar el daemon")?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(s) = connect() {
            return Ok(s);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bail!("el daemon no arrancó; mirá {}", paths::log_file().display())
}

pub fn send(stream: &mut UnixStream, cmd: &Command) -> Result<()> {
    let mut line = serde_json::to_string(cmd)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    Ok(())
}

/// Send one command and return the first state the daemon answers with.
pub fn oneshot(cmd: Option<Command>, spawn: bool) -> Result<Option<crate::protocol::State>> {
    let stream = if spawn {
        connect_or_spawn()?
    } else {
        match connect() {
            Some(s) => s,
            None => return Ok(None),
        }
    };
    let mut writer = stream.try_clone()?;
    let mut lines = BufReader::new(stream).lines();
    // The daemon greets every client with its state.
    let mut state = None;
    if let Some(Ok(l)) = lines.next()
        && let Ok(Event::State(s)) = serde_json::from_str(&l)
    {
        state = Some(s);
    }
    if let Some(cmd) = cmd {
        let quitting = matches!(cmd, Command::Quit);
        send(&mut writer, &cmd)?;
        if quitting {
            return Ok(state);
        }
        for l in lines.map_while(Result::ok) {
            match serde_json::from_str(&l) {
                Ok(Event::State(s)) => return Ok(Some(s)),
                Ok(Event::Notice { text }) => eprintln!("{text}"),
                _ => {}
            }
        }
    }
    Ok(state)
}
