mod app;
mod font;
mod scene;
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

pub fn run() -> Result<()> {
    let stream = client::connect_or_spawn()?;
    let mut writer = stream.try_clone()?;
    let (tx, rx) = mpsc::channel::<Option<Event>>();
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if let Ok(ev) = serde_json::from_str::<Event>(&line)
                && tx.send(Some(ev)).is_err()
            {
                return;
            }
        }
        let _ = tx.send(None);
    });
    client::send(&mut writer, &Command::Watch { levels: true })?;

    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);
    let mut app = App::new();
    let result = (|| -> Result<Exit> {
        let mut last = Instant::now();
        loop {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    Some(Event::State(s)) => {
                        app.connected = true;
                        app.set_state(s);
                    }
                    Some(Event::Levels { layers, .. }) => app.set_levels(layers),
                    Some(Event::Notice { text }) => app.notify(text),
                    None => anyhow::bail!("se perdió la conexión con el audio"),
                }
            }

            let now = Instant::now();
            let dt = (now - last).as_secs_f32().min(0.25);
            last = now;
            let size = terminal.size()?;
            let [scene, _, _] = ui::areas(ratatui::layout::Rect::new(0, 0, size.width, size.height));
            app.animate(dt, scene.width, scene.height);
            terminal.draw(|f| ui::draw(f, &app))?;

            let frame = if app.animating() || app.popup.is_some() { FRAME_BUSY } else { FRAME_IDLE };
            let deadline = now + frame;
            // Wait for input until the next frame is due.
            while let Some(wait) = deadline.checked_duration_since(Instant::now()) {
                if !event::poll(wait)? {
                    break;
                }
                match event::read()? {
                    TermEvent::Key(k) if k.kind == KeyEventKind::Press => app.key(k),
                    TermEvent::Paste(s) => app.paste(&s),
                    TermEvent::Resize(..) => break,
                    _ => {}
                }
                for cmd in app.outbox.drain(..) {
                    client::send(&mut writer, &cmd)?;
                }
                if app.exit.is_some() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{self, Preset};
    use ratatui::{Terminal, backend::TestBackend};

    fn render(w: u16, h: u16, setup: impl FnOnce(&mut App)) -> String {
        let mut app = App::new();
        let mut state = crate::protocol::State::default();
        state.sounds = library::all_sounds().unwrap();
        state.volumes = state.sounds.iter().map(|s| (s.id.clone(), 0.6)).collect();
        state.master = 0.8;
        state.presets = vec![Preset {
            name: "Biblioteca de Rivendell".into(),
            layers: [("noise-brown".to_string(), 0.3), ("binaural-40".to_string(), 0.2)].into(),
        }];
        app.set_state(state);
        setup(&mut app);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        for _ in 0..60 {
            let [scene, _, _] = ui::areas(ratatui::layout::Rect::new(0, 0, w, h));
            app.animate(0.05, scene.width, scene.height);
        }
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn snapshot() {
        println!(
            "{}",
            render(110, 34, |a| {
                a.state.active = vec!["noise-brown".into(), "binaural-40".into()];
                a.set_levels(vec![("noise-brown".into(), 0.2), ("binaural-40".into(), 0.15)]);
                a.cursor = 2;
                a.state.timer = Some(crate::protocol::TimerState { total_secs: 3000, remaining_secs: 1873 });
            })
        );
        println!("{}", render(70, 24, |a| a.popup = Some(app::Popup::Help)));
    }
}
