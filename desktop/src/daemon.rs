//! The background process: a Unix socket in front of a `focus_core::Player`. It owns the things
//! that don't belong in the core: the socket and its clients, downloads (yt-dlp/ffmpeg) and the
//! desktop notification when the timer ends.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command as Process;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use focus_core::meditation;
use focus_core::player::{Player, PlayerOptions};
use focus_core::protocol::{AddMeditationRequest, Command, Event};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

use crate::download;
use crate::paths;

const IDLE_EXIT: Duration = Duration::from_secs(60);

struct Client {
    stream: UnixStream,
    levels: bool,
}

type Clients = Arc<Mutex<HashMap<u64, Client>>>;

fn lock(clients: &Clients) -> MutexGuard<'_, HashMap<u64, Client>> {
    clients.lock().unwrap_or_else(|e| e.into_inner())
}

enum Msg {
    Quit,
}

pub fn run() -> Result<()> {
    let sock = paths::socket_path();
    if UnixStream::connect(&sock).is_ok() {
        return Ok(()); // already running
    }
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).with_context(|| format!("no pude abrir {}", sock.display()))?;
    // Survive the terminal that launched us closing.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }

    let player = Arc::new(Player::with_options(
        paths::library_dir(),
        PlayerOptions { state_file: Some(paths::state_file()), output: None },
    )?);
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    // Nobody to show states to until the first client connects.
    player.set_state_events(false);
    {
        let clients = clients.clone();
        player.set_event_listener(Box::new(move |ev| forward(&clients, &ev)));
    }

    let (tx, rx) = mpsc::channel();
    {
        let (player, clients) = (player.clone(), clients.clone());
        thread::spawn(move || accept(listener, player, clients, tx));
    }

    let mut idle_since: Option<Instant> = None;
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Msg::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let idle = lock(&clients).is_empty() && player.is_idle();
        match (idle, idle_since) {
            (true, None) => idle_since = Some(Instant::now()),
            (true, Some(t)) if t.elapsed() > IDLE_EXIT => break,
            (false, _) => idle_since = None,
            _ => {}
        }
    }

    player.shutdown();
    let _ = std::fs::remove_file(&sock);
    Ok(())
}

/// What the clients need to know changed: tell the ones that want it. The timer's desktop
/// notification goes out here too, with or without clients.
fn forward(clients: &Clients, ev: &Event) {
    if let Event::TimerFinished { minutes } = ev {
        notify(*minutes);
        return;
    }
    let line = encode(ev);
    let only_watchers = matches!(ev, Event::Levels { .. });
    lock(clients).retain(|_, c| (only_watchers && !c.levels) || c.stream.write_all(line.as_bytes()).is_ok());
}

/// The player only produces state and levels while somebody can use them.
fn refresh_interest(player: &Player, clients: &Clients) {
    let (any, watching) = {
        let c = lock(clients);
        (!c.is_empty(), c.values().any(|c| c.levels))
    };
    player.set_state_events(any);
    player.set_watch_levels(watching);
}

fn accept(listener: UnixListener, player: Arc<Player>, clients: Clients, quit: Sender<Msg>) {
    for (n, stream) in listener.incoming().enumerate() {
        let Ok(stream) = stream else { continue };
        let id = n as u64;
        let Ok(write) = stream.try_clone() else { continue };
        let _ = write.set_write_timeout(Some(Duration::from_millis(250)));
        lock(&clients).insert(id, Client { stream: write, levels: false });
        refresh_interest(&player, &clients);
        send_to(&clients, id, &Event::State(Box::new(player.state())));
        let (player, clients, quit) = (player.clone(), clients.clone(), quit.clone());
        thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Ok(cmd) = serde_json::from_str::<Command>(&line) {
                    handle(&player, &clients, &quit, id, cmd);
                }
            }
            lock(&clients).remove(&id);
            refresh_interest(&player, &clients);
        });
    }
}

fn handle(player: &Arc<Player>, clients: &Clients, quit: &Sender<Msg>, client: u64, cmd: Command) {
    match cmd {
        Command::Quit => {
            let _ = quit.send(Msg::Quit);
        }
        Command::Watch { levels } => {
            if let Some(c) = lock(clients).get_mut(&client) {
                c.levels = levels;
            }
            refresh_interest(player, clients);
        }
        Command::Add(req) => {
            let name = req.name.clone();
            start_download(player, req.url.clone(), name, false, move |p| download::add(&req, p).map(|e| e.name));
        }
        Command::AddMeditation(req) => add_meditation(player, clients, req),
        cmd => {
            let _ = player.send(cmd);
        }
    }
}

fn add_meditation(player: &Arc<Player>, clients: &Clients, req: AddMeditationRequest) {
    if let Err(e) = meditation::Category::try_from(req.minutes) {
        forward(clients, &Event::Notice { text: format!("✗ {e}") });
        return;
    }
    let name = req.name.clone();
    start_download(player, req.url.clone(), name, true, move |p| download::add_meditation(&req, p).map(|e| e.name));
}

/// Runs `job` on its own thread, reporting progress to the player. The job returns the display
/// name of what it added.
fn start_download<F>(player: &Arc<Player>, url: String, name: Option<String>, meditation: bool, job: F)
where
    F: FnOnce(&mut dyn FnMut(&str, f32)) -> Result<String> + Send + 'static,
{
    if !player.download_started(&url, name.as_deref(), meditation) {
        return;
    }
    let player = player.clone();
    thread::spawn(move || {
        let mut last = (String::new(), -1.0f32);
        let res = job(&mut |stage, p| {
            // Don't flood clients: only report whole-percent changes.
            if stage != last.0 || (p - last.1).abs() >= 0.01 {
                last = (stage.to_string(), p);
                player.download_progress(&url, stage, p);
            }
        });
        player.download_finished(&url, res.map_err(|e| format!("{e:#}")));
    });
}

fn send_to(clients: &Clients, id: u64, ev: &Event) {
    let line = encode(ev);
    let mut c = lock(clients);
    if let Some(client) = c.get_mut(&id)
        && client.stream.write_all(line.as_bytes()).is_err()
    {
        c.remove(&id);
    }
}

fn encode(ev: &Event) -> String {
    let mut s = serde_json::to_string(ev).unwrap_or_default();
    s.push('\n');
    s
}

fn notify(mins: u64) {
    let body = match mins {
        0 => "Se terminó el temporizador.".to_string(),
        m => format!("{m} min de foco. Buen momento para estirarte y tomar agua."),
    };
    #[cfg(unix)]
    let _ =
        Process::new("notify-send").args(["-a", "focus", "-i", "alarm-symbolic", "Sesión terminada", &body]).spawn();
    #[cfg(windows)]
    toast("Sesión terminada", &body);
}

/// Silent Windows toast through PowerShell's WinRT bridge (no extra dependencies).
#[cfg(windows)]
fn toast(title: &str, body: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\'', "''");
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null;\
         [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] > $null;\
         $x = New-Object Windows.Data.Xml.Dom.XmlDocument;\
         $x.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual><audio silent=\"true\"/></toast>');\
         $app = '{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe';\
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($app).Show([Windows.UI.Notifications.ToastNotification]::new($x))",
        esc(title),
        esc(body)
    );
    let _ = Process::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
