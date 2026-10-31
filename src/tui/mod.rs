mod app;
mod font;
mod menu;
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
        app.screen = app::Screen::Focus;
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

    fn key(code: ratatui::crossterm::event::KeyCode) -> ratatui::crossterm::event::KeyEvent {
        ratatui::crossterm::event::KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE)
    }

    fn press(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.key(key(ratatui::crossterm::event::KeyCode::Char(c)));
        }
    }

    fn med(id: &str, name: &str, minutes: u32) -> crate::meditation::Meditation {
        crate::meditation::Meditation {
            id: id.into(),
            name: name.into(),
            category: crate::meditation::Category::try_from(minutes).unwrap(),
            file: format!("{id}.opus"),
            duration: minutes as u64 * 60,
            available: true,
        }
    }

    fn app_with_meditations() -> App {
        let mut app = App::new();
        app.set_state(crate::protocol::State {
            meditations: vec![
                med("resp", "Respiración 4-7-8", 5),
                med("calma", "Calma guiada", 10),
                med("scan", "Escaneo corporal", 10),
            ],
            ..Default::default()
        });
        app
    }

    fn screen_text(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui::draw(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    #[test]
    fn starts_on_the_home_menu_with_its_four_options() {
        let app = App::new();
        assert_eq!(app.screen, app::Screen::Home);
        let t = screen_text(&app, 80, 20);
        for s in ["Continuar", "Elegir otra rutina", "Ir a Focus", "Configuración", "1-4 directo"] {
            assert!(t.contains(s), "falta «{s}» en:\n{t}");
        }
    }

    #[test]
    fn routines_options_say_they_are_not_there_yet() {
        let mut app = App::new();
        press(&mut app, "1");
        assert!(app.notice.as_ref().unwrap().0.contains("todavía no está"));
        assert_eq!(app.screen, app::Screen::Home);
        press(&mut app, "2");
        assert!(app.notice.as_ref().unwrap().0.contains("Elegir otra rutina"));
        assert_eq!(app.screen, app::Screen::Home);
    }

    #[test]
    fn go_to_focus_and_esc_back_toggle_the_level_stream() {
        let mut app = App::new();
        press(&mut app, "3");
        assert_eq!(app.screen, app::Screen::Focus);
        assert!(matches!(app.outbox.last(), Some(Command::Watch { levels: true })));
        app.key(key(ratatui::crossterm::event::KeyCode::Esc));
        assert_eq!(app.screen, app::Screen::Home);
        assert!(matches!(app.outbox.last(), Some(Command::Watch { levels: false })));
    }

    #[test]
    fn esc_walks_back_one_step_at_a_time() {
        use ratatui::crossterm::event::KeyCode::{Enter, Esc};
        let mut app = App::new();
        press(&mut app, "4");
        assert_eq!(app.screen, app::Screen::Config);
        press(&mut app, "j");
        app.key(key(Enter));
        assert_eq!(app.screen, app::Screen::Meditations);
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Config);
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Home);
    }

    #[test]
    fn config_lists_its_entries_and_only_meditations_opens() {
        let mut app = App::new();
        press(&mut app, "4");
        let t = screen_text(&app, 80, 20);
        for s in ["Rutinas", "Meditaciones", "Sonidos ambientales", "Presets"] {
            assert!(t.contains(s), "falta «{s}»");
        }
        app.key(key(ratatui::crossterm::event::KeyCode::Enter)); // Rutinas
        assert_eq!(app.screen, app::Screen::Config);
        assert!(app.notice.is_some());
    }

    #[test]
    fn meditations_screen_groups_by_duration() {
        let mut app = app_with_meditations();
        app.screen = app::Screen::Meditations;
        let t = screen_text(&app, 80, 20);
        let pos = |s: &str| t.find(s).unwrap_or_else(|| panic!("falta «{s}» en:\n{t}"));
        assert!(pos("5 minutos") < pos("Respiración 4-7-8"));
        assert!(pos("Respiración 4-7-8") < pos("10 minutos"));
        assert!(pos("10 minutos") < pos("Calma guiada"));
        assert!(!t.contains("20 minutos"), "las categorías vacías no se muestran");
        assert!(t.contains("agregar desde YouTube"));
    }

    #[test]
    fn enter_plays_then_toggles_and_shows_progress() {
        use ratatui::crossterm::event::KeyCode::Enter;
        let mut app = app_with_meditations();
        app.screen = app::Screen::Meditations;
        app.key(key(Enter));
        assert!(matches!(app.outbox.pop(), Some(Command::PlayMeditation { id }) if id == "resp"));
        app.state.meditation = Some(crate::protocol::MeditationPlayback {
            id: "resp".into(),
            position_secs: 192,
            total_secs: 300,
            paused: false,
        });
        assert!(screen_text(&app, 80, 20).contains("▶ 03:12 / 05:00"));
        app.key(key(Enter));
        assert!(matches!(app.outbox.pop(), Some(Command::ToggleMeditation)));
        app.state.meditation.as_mut().unwrap().paused = true;
        assert!(screen_text(&app, 80, 20).contains("⏸ 03:12 / 05:00"));
    }

    #[test]
    fn adding_asks_for_the_link_then_the_category() {
        use ratatui::crossterm::event::KeyCode::{Down, Enter};
        let mut app = app_with_meditations();
        app.screen = app::Screen::Meditations;
        press(&mut app, "a");
        app.key(key(Enter)); // sin link no avanza
        assert!(matches!(app.med_popup, Some(app::MedPopup::Add { category: None, .. })));
        app.paste("https://youtu.be/abc");
        app.key(key(Enter));
        assert!(screen_text(&app, 80, 24).contains("¿De cuántos minutos es?"));
        app.key(key(Down));
        app.key(key(Down)); // 5 -> 10 -> 15
        app.key(key(Enter));
        assert!(app.med_popup.is_none());
        match app.outbox.pop() {
            Some(Command::AddMeditation(r)) => assert_eq!((r.url.as_str(), r.minutes), ("https://youtu.be/abc", 15)),
            other => panic!("salió {other:?}"),
        }
    }

    #[test]
    fn deleting_needs_confirmation() {
        use ratatui::crossterm::event::KeyCode::Esc;
        let mut app = app_with_meditations();
        app.screen = app::Screen::Meditations;
        press(&mut app, "d");
        assert!(app.outbox.is_empty());
        app.key(key(Esc));
        assert!(app.med_popup.is_none() && app.outbox.is_empty());
        press(&mut app, "d");
        assert!(screen_text(&app, 80, 20).contains("¿Borrar «Respiración 4-7-8»?"));
        press(&mut app, "s");
        assert!(matches!(app.outbox.pop(), Some(Command::DeleteMeditation { id }) if id == "resp"));
    }

    #[test]
    fn menu_screens_are_still_so_the_tui_idles() {
        let mut app = App::new();
        app.state.active = vec!["noise-brown".into()];
        assert!(!app.animating(), "con sonido en background, el menú no anima");
    }
}
