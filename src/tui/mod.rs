mod app;
mod font;
mod help;
mod menu;
mod routine_ui;
mod routines;
mod scene;
mod syncmenu;
mod theme;
mod ui;

use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event as TermEvent, KeyEventKind};
use ratatui::crossterm::execute;

use crate::client;
use crate::protocol::{Command, Event};
use app::{App, Exit};

/// ~20 fps while something moves, a few fps otherwise.
const FRAME_BUSY: Duration = Duration::from_millis(50);
const FRAME_IDLE: Duration = Duration::from_millis(250);

/// Everything the main loop waits on arrives through one channel, so a key
/// or an answer from the daemon wakes it up right away.
enum Msg {
    Term(TermEvent),
    TermClosed,
    Daemon(Option<Event>),
}

/// How long to wait for messages before the next frame is due (`None` = already due).
fn wait_until(deadline: Instant, now: Instant) -> Option<Duration> {
    deadline.checked_duration_since(now).filter(|d| !d.is_zero())
}

fn frame_for(busy: bool) -> Duration {
    if busy { FRAME_BUSY } else { FRAME_IDLE }
}

pub fn run() -> Result<()> {
    let stream = client::connect_or_spawn()?;
    let mut writer = stream.try_clone()?;
    let (tx, rx) = mpsc::channel::<Msg>();
    let daemon_tx = tx.clone();
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if let Ok(ev) = serde_json::from_str::<Event>(&line)
                && daemon_tx.send(Msg::Daemon(Some(ev))).is_err()
            {
                return;
            }
        }
        let _ = daemon_tx.send(Msg::Daemon(None));
    });

    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);
    // Only reads: it never touches the terminal modes, so leaving it blocked
    // on `read` when the process exits is harmless.
    thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if tx.send(Msg::Term(ev)).is_err() {
                return;
            }
        }
        let _ = tx.send(Msg::TermClosed);
    });
    let mut app = App::new();
    app.load_routines();
    let result = (|| -> Result<Exit> {
        let mut last = Instant::now();
        loop {
            let now = Instant::now();
            let dt = (now - last).as_secs_f32().min(0.25);
            last = now;
            let size = terminal.size()?;
            let full = ratatui::layout::Rect::new(0, 0, size.width, size.height);
            let scene = if app.screen == app::Screen::Focus {
                ui::areas(full, app.deep != app::Deep::Off)[0]
            } else {
                menu::band_area(full)
            };
            app.animate(dt, scene.width, scene.height);
            app.persist();
            terminal.draw(|f| ui::draw(f, &app))?;

            let deadline = now + frame_for(app.animating() || app.popup.is_some() || app.sync.is_some());
            // Sleep until the next frame is due; input or daemon news cut it short.
            while let Some(wait) = wait_until(deadline, Instant::now()) {
                let Ok(first) = rx.recv_timeout(wait) else { break };
                let mut redraw = handle(&mut app, first)?;
                // Key repeat: apply everything already queued, then draw once.
                while let Ok(msg) = rx.try_recv() {
                    redraw |= handle(&mut app, msg)?;
                }
                app.persist();
                for cmd in app.outbox.drain(..) {
                    client::send(&mut writer, &cmd)?;
                }
                if app.exit.is_some() || redraw {
                    break;
                }
            }
            if let Some(e) = app.exit {
                return Ok(e);
            }
        }
    })();
    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();

    match result? {
        Exit::Quit => client::send(&mut writer, &Command::Quit)?,
        Exit::Detach => {
            let _ = client::send(&mut writer, &Command::Watch { levels: false });
            if app.playing() {
                println!("♫ sigue sonando en background · `focus` para volver · `focus quit` para apagar");
            }
        }
    }
    Ok(())
}

/// Applies one message; returns whether the screen needs redrawing right away.
/// Level meters are left for the next frame so they don't force extra draws.
fn handle(app: &mut App, msg: Msg) -> Result<bool> {
    Ok(match msg {
        Msg::Term(TermEvent::Key(k)) if k.kind == KeyEventKind::Press => {
            app.key(k);
            true
        }
        Msg::Term(TermEvent::Paste(s)) => {
            app.paste(&s);
            true
        }
        Msg::Term(TermEvent::Resize(..)) => true,
        Msg::Term(_) => false,
        Msg::TermClosed => anyhow::bail!("se cerró la entrada de la terminal"),
        Msg::Daemon(Some(Event::State(s))) => {
            app.connected = true;
            app.set_state(*s);
            true
        }
        Msg::Daemon(Some(Event::Levels { layers, .. })) => {
            app.set_levels(layers);
            false
        }
        Msg::Daemon(Some(Event::Notice { text })) => {
            app.notify(text);
            true
        }
        Msg::Daemon(None) => anyhow::bail!("se perdió la conexión con el audio"),
    })
}

#[cfg(test)]
mod tests;
