mod app;
mod font;
mod menu;
mod routine_ui;
mod routines;
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
    app.load_routines();
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
                app.persist();
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
        let app = app_with_routines();
        assert_eq!(app.screen, app::Screen::Home);
        let t = screen_text(&app, 80, 20);
        for s in ["Continuar", "Elegir otra rutina", "Ir a Focus", "Configuración", "1-4 directo"] {
            assert!(t.contains(s), "falta «{s}» en:\n{t}");
        }
    }

    #[test]
    fn go_to_focus_and_esc_back_toggle_the_level_stream() {
        let mut app = App::new();
        press(&mut app, "3");
        assert_eq!(app.screen, app::Screen::Focus);
        assert!(matches!(app.outbox.last(), Some(Command::Watch { levels: true })));
        app.key(key(ratatui::crossterm::event::KeyCode::Esc)); // saltea la ventanita
        assert_eq!(app.screen, app::Screen::Focus);
        app.key(key(ratatui::crossterm::event::KeyCode::Esc));
        assert_eq!(app.screen, app::Screen::Home);
        assert!(matches!(app.outbox.last(), Some(Command::Watch { levels: false })));
    }

    // ------------------------------------------------- ventanita de Focus

    fn app_with_presets() -> App {
        use crate::library::Preset;
        let mut app = App::new();
        app.set_state(crate::protocol::State {
            presets: vec![
                Preset { name: "Templo Jedi".into(), layers: [("noise-brown".to_string(), 0.3)].into() },
                Preset { name: "Lluvia y fuego".into(), layers: [("noise-pink".to_string(), 0.4)].into() },
            ],
            ..Default::default()
        });
        app
    }

    fn intro(app: &App) -> Option<app::Intro> {
        match app.popup {
            Some(app::Popup::Intro(i)) => Some(i),
            _ => None,
        }
    }

    #[test]
    fn going_to_focus_opens_the_time_step_with_the_last_value() {
        let mut app = app_with_presets();
        app.focus_minutes = 35;
        press(&mut app, "3");
        assert_eq!(app.screen, app::Screen::Focus);
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 35, typed: false }));
        let t = screen_text(&app, 90, 30);
        assert!(t.contains("Antes de arrancar · 1 de 2") && t.contains("[ 35 ] minutos"), "{t}");
        assert!(app.outbox.iter().all(|c| !matches!(c, Command::Timer { .. })), "todavía no arranca nada");
    }

    #[test]
    fn the_first_run_starts_at_fifty_minutes() {
        let mut app = App::new();
        press(&mut app, "3");
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 50, typed: false }));
    }

    #[test]
    fn arrows_change_the_time_by_five_and_never_below_one() {
        let mut app = App::new();
        press(&mut app, "3");
        app.key(key(Up));
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 55, typed: false }));
        for _ in 0..20 {
            app.key(key(Down));
        }
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 1, typed: false }));
    }

    #[test]
    fn typing_replaces_the_value_then_appends_and_backspace_erases() {
        let mut app = App::new();
        press(&mut app, "3");
        press(&mut app, "4");
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 4, typed: true }));
        press(&mut app, "5");
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 45, typed: true }));
        press(&mut app, "99"); // 4599 no entra: el tope es 999
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 459, typed: true }));
        app.key(key(ratatui::crossterm::event::KeyCode::Backspace));
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 45, typed: true }));
        press(&mut app, "x"); // letras no hacen nada
        assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 45, typed: true }));
    }

    #[test]
    fn zero_minutes_cannot_continue() {
        let mut app = App::new();
        press(&mut app, "3");
        press(&mut app, "0");
        app.key(key(Enter));
        assert!(matches!(intro(&app), Some(app::Intro::Time { minutes: 0, .. })));
        assert!(app.notice.is_some());
    }

    #[test]
    fn esc_on_the_time_step_goes_straight_to_focus_without_starting_anything() {
        let mut app = app_with_presets();
        press(&mut app, "3");
        sent(&mut app);
        app.key(key(Esc));
        assert!(app.popup.is_none());
        assert_eq!(app.screen, app::Screen::Focus);
        assert!(sent(&mut app).is_empty());
        assert!(!app.last_dirty, "no recuerda nada si se saltea");
    }

    #[test]
    fn enter_remembers_the_time_and_moves_to_the_sounds_step_listing_presets_and_custom() {
        let mut app = app_with_presets();
        press(&mut app, "3");
        press(&mut app, "25");
        app.key(key(Enter));
        assert_eq!(app.focus_minutes, 25);
        assert!(app.last_dirty);
        assert_eq!(intro(&app), Some(app::Intro::Sounds { minutes: 25, sel: 0 }));
        let t = screen_text(&app, 90, 30);
        let pos = |s: &str| t.find(s).unwrap_or_else(|| panic!("falta «{s}» en:\n{t}"));
        assert!(t.contains("2 de 2"));
        assert!(pos("Templo Jedi") < pos("Lluvia y fuego") && pos("Lluvia y fuego") < pos("Sonidos personalizados"));
    }

    #[test]
    fn esc_on_the_sounds_step_enters_focus_without_a_timer_or_a_preset() {
        let mut app = app_with_presets();
        press(&mut app, "3");
        app.key(key(Enter));
        sent(&mut app);
        app.key(key(Esc));
        assert!(app.popup.is_none());
        assert_eq!(app.screen, app::Screen::Focus);
        assert!(sent(&mut app).is_empty());
    }

    #[test]
    fn choosing_a_preset_starts_the_timer_and_loads_it() {
        let mut app = app_with_presets();
        press(&mut app, "3");
        press(&mut app, "30");
        app.key(key(Enter));
        app.key(key(Down));
        app.key(key(Enter));
        assert!(app.popup.is_none());
        let out = sent(&mut app);
        assert!(out.iter().any(|c| matches!(c, Command::Timer { minutes: Some(30) })));
        assert!(out.iter().any(|c| matches!(c, Command::LoadPreset { name } if name == "Lluvia y fuego")));
    }

    #[test]
    fn custom_sounds_starts_only_the_timer() {
        let mut app = app_with_presets();
        press(&mut app, "3");
        app.key(key(Enter));
        press(&mut app, "jjj"); // más allá de la última fila no pasa
        assert_eq!(intro(&app), Some(app::Intro::Sounds { minutes: 50, sel: 2 }));
        app.key(key(Enter));
        let out = sent(&mut app);
        assert!(out.iter().any(|c| matches!(c, Command::Timer { minutes: Some(50) })));
        assert!(!out.iter().any(|c| matches!(c, Command::LoadPreset { .. })));
        assert_eq!(app.screen, app::Screen::Focus);
    }

    #[test]
    fn without_presets_the_only_option_is_custom_sounds() {
        let mut app = App::new();
        press(&mut app, "3");
        app.key(key(Enter));
        assert!(screen_text(&app, 90, 30).contains("Sonidos personalizados"));
        app.key(key(Enter));
        assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Timer { minutes: Some(50) })));
    }

    #[test]
    fn the_window_also_opens_when_a_routine_ends() {
        let mut app = app_with_routines();
        press(&mut app, "1");
        press(&mut app, " s   ");
        assert_eq!(app.screen, app::Screen::Focus);
        assert!(matches!(intro(&app), Some(app::Intro::Time { .. })));
    }

    #[test]
    fn it_shows_up_every_time_focus_is_entered() {
        let mut app = App::new();
        press(&mut app, "3");
        app.key(key(Esc));
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Home);
        press(&mut app, "3");
        assert!(intro(&app).is_some());
    }

    #[test]
    fn q_does_not_quit_from_inside_the_window() {
        let mut app = App::new();
        press(&mut app, "3");
        press(&mut app, "qQ");
        assert!(app.exit.is_none());
    }

    // ------------------------------------- Configuración > Sonidos ambientales

    fn file_sound(id: &str, name: &str, theme: library::Theme, available: bool) -> library::Sound {
        library::Sound {
            id: id.into(),
            name: name.into(),
            theme,
            visual: theme.default_visual(),
            kind: library::SoundKind::File { file: format!("{id}.opus") },
            duration: 3600,
            available,
        }
    }

    fn app_with_sounds() -> App {
        let mut app = App::new();
        let mut sounds = vec![
            file_sound("templo", "Templo Jedi", library::Theme::Starwars, true),
            file_sound("lluvia", "Lluvia", library::Theme::Nature, true),
            file_sound("fuego", "Fuego", library::Theme::Nature, false),
        ];
        sounds.extend(library::all_sounds().unwrap().into_iter().filter(|s| s.theme == library::Theme::Generated));
        app.set_state(crate::protocol::State { sounds, ..Default::default() });
        app
    }

    fn open_sounds(app: &mut App) {
        press(app, "4jj");
        app.key(key(Enter));
        assert_eq!(app.screen, app::Screen::Sounds);
    }

    #[test]
    fn config_opens_the_sounds_list_with_downloaded_sounds_only() {
        let mut app = app_with_sounds();
        open_sounds(&mut app);
        let t = screen_text(&app, 80, 24);
        for s in ["Sonidos ambientales", "Templo Jedi", "Lluvia", "☁ Fuego", "agregar desde YouTube", "borrar"] {
            assert!(t.contains(s), "falta «{s}» en:\n{t}");
        }
        assert!(!t.contains("Ruido blanco") && !t.contains("Binaural"), "los generados no se listan:\n{t}");
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Config);
    }

    #[test]
    fn empty_sounds_list_says_what_to_do_and_d_does_nothing() {
        let mut app = App::new();
        open_sounds(&mut app);
        assert!(screen_text(&app, 100, 24).contains("Todavía no hay sonidos descargados"));
        press(&mut app, "d");
        assert!(app.popup.is_none());
    }

    #[test]
    fn a_adds_a_sound_with_the_same_form_and_flow_as_before() {
        let mut app = app_with_sounds();
        open_sounds(&mut app);
        press(&mut app, "a");
        assert!(matches!(app.popup, Some(app::Popup::Add(_))));
        assert!(screen_text(&app, 90, 30).contains("agregar desde YouTube"));
        app.paste("https://youtu.be/xyz");
        app.key(key(Enter)); // URL -> nombre
        app.key(key(Enter)); // nombre -> tema
        app.key(key(Enter)); // tema -> recorte
        app.key(key(Enter)); // descargar
        assert!(app.popup.is_none());
        match sent(&mut app).pop() {
            Some(Command::Add(r)) => assert_eq!(r.url, "https://youtu.be/xyz"),
            other => panic!("salió {other:?}"),
        }
    }

    #[test]
    fn pasting_a_link_on_the_sounds_screen_opens_the_form_with_it() {
        let mut app = app_with_sounds();
        open_sounds(&mut app);
        app.paste("https://youtu.be/xyz");
        match &app.popup {
            Some(app::Popup::Add(f)) => assert_eq!((f.url.as_str(), f.field), ("https://youtu.be/xyz", 1)),
            _ => panic!("no abrió el formulario"),
        }
        app.key(key(Esc));
        app.paste("hola");
        assert!(app.popup.is_none(), "un texto que no es un link no abre nada");
    }

    #[test]
    fn d_asks_before_deleting_the_selected_sound() {
        let mut app = app_with_sounds();
        open_sounds(&mut app);
        // En pantalla van agrupados por tema: el orden del estado manda para el cursor.
        let first = app.selected_file_sound().unwrap().name.clone();
        press(&mut app, "d");
        assert!(sent(&mut app).is_empty());
        assert!(screen_text(&app, 80, 24).contains(&format!("¿Borrar «{first}»?")));
        app.key(key(Esc));
        assert!(app.popup.is_none() && sent(&mut app).is_empty());
        press(&mut app, "dn");
        assert!(app.popup.is_none() && sent(&mut app).is_empty());
        press(&mut app, "j");
        let second = app.selected_file_sound().unwrap().id.clone();
        press(&mut app, "ds");
        assert!(matches!(sent(&mut app).pop(), Some(Command::DeleteSound { id }) if id == second));
    }

    #[test]
    fn the_sound_cursor_stays_inside_the_list_after_a_delete() {
        let mut app = app_with_sounds();
        open_sounds(&mut app);
        press(&mut app, "G");
        assert_eq!(app.sound_cursor, 2);
        let mut st = app.state.clone();
        st.sounds.remove(2);
        app.set_state(st);
        assert_eq!(app.sound_cursor, 1);
    }

    #[test]
    fn the_sounds_screen_is_still() {
        let mut app = app_with_sounds();
        open_sounds(&mut app);
        assert!(!app.animating());
    }

    // ------------------------------------------- Configuración > Presets

    fn open_presets(app: &mut App) {
        press(app, "4jjj");
        app.key(key(Enter));
        assert_eq!(app.screen, app::Screen::Presets);
    }

    fn app_with_sounds_and_presets() -> App {
        let mut app = app_with_sounds();
        let mut st = app.state.clone();
        st.presets = app_with_presets().state.presets;
        st.volumes.insert("lluvia".into(), 0.7);
        app.set_state(st);
        app
    }

    #[test]
    fn presets_screen_lists_them_with_their_sounds_and_volumes() {
        let mut app = app_with_sounds_and_presets();
        open_presets(&mut app);
        let t = screen_text(&app, 90, 24);
        for s in ["Presets", "Templo Jedi", "Ruido marrón 30%", "Lluvia y fuego", "n nuevo", "d borrar"] {
            assert!(t.contains(s), "falta «{s}» en:\n{t}");
        }
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Config);
    }

    #[test]
    fn empty_presets_list_says_what_to_do() {
        let mut app = App::new();
        open_presets(&mut app);
        assert!(screen_text(&app, 100, 24).contains("Todavía no hay presets"));
        press(&mut app, "d");
        assert!(app.popup.is_none());
    }

    #[test]
    fn d_asks_before_deleting_a_preset() {
        let mut app = app_with_sounds_and_presets();
        open_presets(&mut app);
        press(&mut app, "j");
        press(&mut app, "d");
        assert!(sent(&mut app).is_empty());
        assert!(screen_text(&app, 80, 24).contains("¿Borrar «Lluvia y fuego»?"));
        app.key(key(Esc));
        assert!(app.popup.is_none() && sent(&mut app).is_empty());
        press(&mut app, "dn");
        assert!(sent(&mut app).is_empty());
        press(&mut app, "ds");
        assert!(matches!(sent(&mut app).pop(), Some(Command::DeletePreset { name }) if name == "Lluvia y fuego"));
    }

    fn open_builder(app: &mut App) {
        open_presets(app);
        press(app, "n");
        assert_eq!(app.screen, app::Screen::PresetBuilder);
    }

    #[test]
    fn the_builder_lists_every_sound_with_volume_and_plays_nothing() {
        let mut app = app_with_sounds_and_presets();
        sent(&mut app);
        open_builder(&mut app);
        let t = screen_text(&app, 90, 30);
        for s in ["Nuevo preset", "Templo Jedi", "Lluvia", "Ruido blanco", "Binaural", " 70%", "g guardar"] {
            assert!(t.contains(s), "falta «{s}» en:\n{t}");
        }
        assert!(sent(&mut app).iter().all(|c| matches!(c, Command::Watch { .. })), "no manda comandos de audio");
        assert!(!app.animating());
    }

    #[test]
    fn building_a_preset_picks_sounds_sets_volumes_and_saves_it_with_a_name() {
        let mut app = app_with_sounds_and_presets();
        open_builder(&mut app);
        press(&mut app, " "); // primero: Templo Jedi (volumen por defecto 50 %)
        press(&mut app, "lll"); // +15 %
        press(&mut app, "j");
        press(&mut app, " ");
        press(&mut app, "hh"); // Lluvia parte de 70 %
        press(&mut app, "g");
        assert!(matches!(app.popup, Some(app::Popup::SavePreset { .. })));
        type_text(&mut app, "Estudio");
        app.key(key(Enter));
        assert!(app.popup.is_none());
        assert_eq!(app.screen, app::Screen::Presets);
        assert!(app.builder.is_none());
        match sent(&mut app).pop() {
            Some(Command::StorePreset { name, layers }) => {
                assert_eq!(name, "Estudio");
                assert_eq!(layers.len(), 2);
                assert!((layers["templo"] - 0.65).abs() < 1e-4, "{layers:?}");
                assert!((layers["lluvia"] - 0.6).abs() < 1e-4, "{layers:?}");
            }
            other => panic!("salió {other:?}"),
        }
    }

    #[test]
    fn a_sound_that_is_toggled_twice_is_left_out() {
        let mut app = app_with_sounds_and_presets();
        open_builder(&mut app);
        press(&mut app, "  g");
        assert!(app.popup.is_none(), "sin sonidos no se puede guardar");
        assert!(app.notice.is_some());
    }

    #[test]
    fn sounds_missing_on_this_machine_cannot_go_in_a_preset() {
        let mut app = app_with_sounds_and_presets();
        open_builder(&mut app);
        press(&mut app, "jj "); // Fuego: ☁
        assert!(app.builder.as_ref().unwrap().picked.is_empty());
        assert!(app.notice.is_some());
    }

    #[test]
    fn esc_in_the_builder_discards_it_and_the_name_popup_keeps_it() {
        let mut app = app_with_sounds_and_presets();
        open_builder(&mut app);
        press(&mut app, " g");
        app.key(key(Esc)); // cancela el nombre, no el borrador
        assert!(app.popup.is_none() && app.builder.as_ref().is_some_and(|b| b.picked.len() == 1));
        assert_eq!(app.screen, app::Screen::PresetBuilder);
        sent(&mut app);
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Presets);
        assert!(app.builder.is_none() && sent(&mut app).is_empty());
    }

    #[test]
    fn the_preset_needs_a_name_and_it_cannot_repeat_one() {
        let mut app = app_with_sounds_and_presets();
        open_builder(&mut app);
        press(&mut app, " g");
        app.key(key(Enter)); // sin nombre no avanza
        assert!(app.popup.is_some() && sent(&mut app).is_empty());
        type_text(&mut app, "templo JEDI");
        app.key(key(Enter));
        assert!(matches!(app.popup, Some(app::Popup::SavePreset { .. })), "sigue pidiendo nombre");
        assert!(app.builder.is_some() && sent(&mut app).is_empty());
        assert!(app.notice.is_some());
    }

    #[test]
    fn pasting_a_name_in_the_save_popup_works_on_any_screen() {
        let mut app = app_with_sounds_and_presets();
        open_builder(&mut app);
        press(&mut app, " g");
        app.paste("Mi mezcla\n");
        match &app.popup {
            Some(app::Popup::SavePreset { name }) => assert_eq!(name, "Mi mezcla"),
            _ => panic!("no hay popup"),
        }
    }

    // ------------------------------------------------ Focus sin a, s, d

    #[test]
    fn focus_no_longer_adds_saves_or_deletes() {
        let mut app = app_with_sounds_and_presets();
        press(&mut app, "3");
        app.key(key(Esc)); // saltea la ventanita
        app.state.active = vec!["noise-brown".into()];
        sent(&mut app);
        press(&mut app, "asd");
        assert!(app.popup.is_none() && sent(&mut app).is_empty());
        app.key(key(ratatui::crossterm::event::KeyCode::Tab)); // panel de presets
        press(&mut app, "ad");
        assert!(app.popup.is_none() && sent(&mut app).is_empty());
        app.paste("https://youtu.be/xyz");
        assert!(app.popup.is_none(), "pegar un link en Focus ya no abre «agregar»");
    }

    #[test]
    fn focus_hints_and_help_no_longer_mention_a_s_d() {
        let mut app = app_with_sounds_and_presets();
        press(&mut app, "3");
        app.key(key(Esc));
        let t = screen_text(&app, 120, 34);
        for gone in ["guardar", "agregar", "borrar"] {
            assert!(!t.contains(gone), "no tendría que decir «{gone}»:\n{t}");
        }
        app.popup = Some(app::Popup::Help);
        let t = screen_text(&app, 80, 30);
        for gone in ["guardar la mezcla", "agregar sonido", "pegar un link"] {
            assert!(!t.contains(gone), "la ayuda no tendría que decir «{gone}»:\n{t}");
        }
        assert!(t.contains("temporizador") && t.contains("volver al menú"));
        app.popup = None;
        app.key(key(ratatui::crossterm::event::KeyCode::Tab));
        let t = screen_text(&app, 120, 34);
        assert!(t.contains("cargar") && !t.contains("borrar"), "{t}");
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
    fn config_lists_its_entries() {
        let mut app = App::new();
        press(&mut app, "4");
        let t = screen_text(&app, 80, 20);
        for s in ["Rutinas", "Meditaciones", "Sonidos ambientales", "Presets"] {
            assert!(t.contains(s), "falta «{s}»");
        }
        press(&mut app, "jjj");
        app.key(key(ratatui::crossterm::event::KeyCode::Enter));
        assert_eq!(app.screen, app::Screen::Presets);
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

    // ------------------------------------------------------------ rutinas

    use crate::meditation::Category;
    use crate::routine::{MeditationSetting, Routine};
    use ratatui::crossterm::event::KeyCode::{Down, Enter, Esc, Left, Right, Up};

    fn cat(m: u32) -> Category {
        Category::try_from(m).unwrap()
    }

    fn setting(m: Option<u32>) -> Option<MeditationSetting> {
        Some(MeditationSetting { minutes: m.map(cat) })
    }

    fn sample_routine() -> Routine {
        Routine {
            name: "Mañana de estudio".into(),
            tasks: vec!["Ordenar el escritorio".into(), "Preparar mate".into(), "Abrir el material".into()],
            meditation: setting(Some(10)),
        }
    }

    fn app_with_routines() -> App {
        let mut app = app_with_meditations();
        app.routines = vec![
            sample_routine(),
            Routine { name: "Tarde".into(), tasks: vec!["x".into()], meditation: setting(None) },
        ];
        app
    }

    fn sent(app: &mut App) -> Vec<Command> {
        std::mem::take(&mut app.outbox)
    }

    fn type_text(app: &mut App, text: &str) {
        press(app, text);
    }

    #[test]
    fn home_continues_the_last_routine_or_offers_to_create_the_first() {
        let mut app = App::new();
        assert!(screen_text(&app, 80, 20).contains("Crear mi primera rutina"));
        press(&mut app, "1");
        assert_eq!(app.screen, app::Screen::RoutineEditor, "sin rutinas, la opción 1 crea");
        let mut app = app_with_routines();
        assert!(screen_text(&app, 80, 20).contains("Continuar \"Mañana de estudio\""));
        app.last_routine = Some("Tarde".into());
        assert_eq!(app.home_items()[0], "Continuar \"Tarde\"");
        press(&mut app, "1");
        assert_eq!(app.screen, app::Screen::RoutineRun);
        assert_eq!(app.run.as_ref().unwrap().routine.name, "Tarde");
        assert!(app.last_dirty);
    }

    #[test]
    fn a_missing_last_routine_falls_back_to_the_first() {
        let mut app = app_with_routines();
        app.last_routine = Some("Borrada".into());
        assert_eq!(app.home_items()[0], "Continuar \"Mañana de estudio\"");
    }

    #[test]
    fn list_shows_summaries_and_esc_goes_back_to_where_it_came_from() {
        let mut app = app_with_routines();
        press(&mut app, "2");
        assert_eq!(app.screen, app::Screen::Routines);
        let t = screen_text(&app, 80, 20);
        for s in ["Mañana de estudio", "4 pasos · 1 meditación", "Enter", "empezar"] {
            assert!(t.contains(s), "falta «{s}» en:\n{t}");
        }
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Home);
        press(&mut app, "4");
        app.key(key(Enter)); // Rutinas
        assert_eq!(app.screen, app::Screen::Routines);
        assert!(!screen_text(&app, 80, 20).contains("empezar"), "desde Configuración no se empieza");
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Config);
    }

    #[test]
    fn deleting_a_routine_asks_first_and_marks_the_file_dirty() {
        let mut app = app_with_routines();
        press(&mut app, "2d");
        assert!(screen_text(&app, 80, 20).contains("¿Borrar la rutina «Mañana de estudio»?"));
        app.key(key(Esc));
        assert_eq!(app.routines.len(), 2);
        assert!(!app.routines_dirty);
        press(&mut app, "ds");
        assert_eq!(app.routines.len(), 1);
        assert_eq!(app.routines[0].name, "Tarde");
        assert!(app.routines_dirty);
    }

    #[test]
    fn a_broken_routines_file_is_never_overwritten() {
        let mut app = app_with_routines();
        app.routines_locked = Some("toml roto".into());
        press(&mut app, "2ds");
        assert_eq!(app.routines.len(), 2);
        assert!(!app.routines_dirty);
        press(&mut app, "n");
        assert_eq!(app.screen, app::Screen::Routines);
    }

    #[test]
    fn new_routine_asks_the_name_then_collects_steps_and_saves() {
        let mut app = app_with_meditations();
        press(&mut app, "2n");
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        assert!(screen_text(&app, 80, 24).contains("Nombre de la rutina"));
        app.key(key(Enter)); // vacío: no avanza
        assert!(app.editor.as_ref().unwrap().popup.is_some());
        type_text(&mut app, "Tarde");
        app.key(key(Enter));
        press(&mut app, "g");
        assert_eq!(app.screen, app::Screen::RoutineEditor, "sin pasos no se guarda");
        press(&mut app, "a"); // agrega una tarea directo, sin preguntar el tipo
        assert!(screen_text(&app, 80, 24).contains("¿Qué hay que hacer?"));
        type_text(&mut app, "Preparar mate");
        app.key(key(Enter));
        assert!(screen_text(&app, 80, 24).contains("Meditación · no"));
        press(&mut app, "j"); // la fila de la meditación, después de las tareas
        app.key(key(Enter));
        assert_eq!(app.screen, app::Screen::RoutineStep);
        app.key(key(Right)); // sí
        app.key(key(Down));
        app.key(key(Down));
        app.key(key(Right)); // 10 -> 15
        app.key(key(Enter));
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        press(&mut app, "g");
        assert_eq!(app.screen, app::Screen::Routines);
        assert!(app.routines_dirty);
        assert_eq!(app.routines.len(), 1);
        assert_eq!(app.routines[0].name, "Tarde");
        assert_eq!(app.routines[0].tasks, vec!["Preparar mate"]);
        assert_eq!(app.routines[0].meditation, setting(Some(15)));
    }

    #[test]
    fn a_routine_with_only_the_meditation_can_be_saved() {
        let mut app = app_with_meditations();
        press(&mut app, "2nSola");
        app.key(key(Enter));
        press(&mut app, "g");
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        press(&mut app, "j");
        app.key(key(Enter));
        app.key(key(Right));
        app.key(key(Enter));
        press(&mut app, "g");
        assert_eq!(app.screen, app::Screen::Routines);
        assert_eq!(app.routines[0].meditation, setting(Some(10)));
        assert!(app.routines[0].tasks.is_empty());
    }

    #[test]
    fn cancelling_the_name_of_a_new_routine_leaves_the_editor() {
        let mut app = app_with_routines();
        press(&mut app, "2n");
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Routines);
        assert!(app.editor.is_none());
    }

    #[test]
    fn names_cannot_repeat() {
        let mut app = app_with_routines();
        press(&mut app, "2e");
        press(&mut app, "r");
        for _ in 0..30 {
            app.key(key(ratatui::crossterm::event::KeyCode::Backspace));
        }
        type_text(&mut app, "tarde");
        app.key(key(Enter));
        assert!(app.notice.as_ref().unwrap().0.contains("ya hay otra rutina"));
        assert!(app.editor.as_ref().unwrap().popup.is_some());
    }

    #[test]
    fn editor_lists_tasks_then_the_meditation_setting_apart() {
        let mut app = app_with_routines();
        press(&mut app, "2e");
        let t = screen_text(&app, 80, 24);
        assert!(t.contains("3  Abrir el material") && t.contains("Meditación · 10 min fija"), "{t}");
        assert!(!t.contains("4  "), "la meditación no se numera como paso:\n{t}");
    }

    #[test]
    fn editor_moves_and_deletes_tasks_with_confirmation() {
        let mut app = app_with_routines();
        press(&mut app, "2ej"); // tarea 2
        press(&mut app, "K");
        assert_eq!(app.editor.as_ref().unwrap().routine.tasks[0], "Preparar mate");
        assert_eq!(app.editor.as_ref().unwrap().cursor, 0);
        press(&mut app, "JJJ");
        assert_eq!(app.editor.as_ref().unwrap().cursor, 2, "J no mueve tareas fuera de la lista");
        assert_eq!(app.editor.as_ref().unwrap().routine.tasks[2], "Preparar mate");
        press(&mut app, "d");
        assert_eq!(app.editor.as_ref().unwrap().routine.tasks.len(), 3);
        assert!(screen_text(&app, 80, 24).contains("¿Borrar la tarea"));
        press(&mut app, "s");
        assert_eq!(app.editor.as_ref().unwrap().routine.tasks.len(), 2);
        assert!(!app.routines_dirty, "hasta guardar no se toca la lista");
    }

    #[test]
    fn m_opens_the_meditation_setting_from_any_row() {
        let mut app = app_with_routines();
        press(&mut app, "2e"); // el cursor está en la primera tarea
        assert_eq!(app.editor.as_ref().unwrap().cursor, 0);
        press(&mut app, "m");
        assert_eq!(app.screen, app::Screen::RoutineStep);
        let d = app.editor.as_ref().unwrap().step.as_ref().unwrap();
        assert!(d.enabled && !d.free, "parte de lo que ya tiene la rutina");
        app.key(key(Down));
        app.key(key(Right)); // modo libre
        app.key(key(Enter));
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        assert_eq!(app.editor.as_ref().unwrap().routine.meditation, setting(None));
        assert_eq!(app.editor.as_ref().unwrap().cursor, 0, "el cursor no se mueve");
    }

    #[test]
    fn m_does_not_open_the_setting_while_typing_a_task() {
        let mut app = app_with_routines();
        press(&mut app, "2ea");
        type_text(&mut app, "mm");
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        assert!(app.editor.as_ref().unwrap().popup.is_some());
    }

    #[test]
    fn editor_help_shows_the_m_key() {
        let mut app = app_with_routines();
        press(&mut app, "2e");
        assert!(screen_text(&app, 100, 24).contains("m meditación"));
    }

    #[test]
    fn the_meditation_row_cannot_be_moved_or_deleted_like_a_task() {
        let mut app = app_with_routines();
        press(&mut app, "2ejjjKd");
        let ed = app.editor.as_ref().unwrap();
        assert!(ed.popup.is_none() && !ed.dirty());
        assert_eq!(ed.cursor, 3);
        press(&mut app, "j");
        assert_eq!(app.editor.as_ref().unwrap().cursor, 3, "la fila de la meditación es la última");
    }

    #[test]
    fn esc_with_unsaved_changes_asks_and_without_them_just_leaves() {
        let mut app = app_with_routines();
        press(&mut app, "2e");
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Routines);
        press(&mut app, "ed");
        press(&mut app, "s");
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        assert!(screen_text(&app, 80, 24).contains("cambios sin guardar"));
        app.key(key(Esc)); // seguir editando
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        app.key(key(Esc));
        press(&mut app, "d"); // descartar
        assert_eq!(app.screen, app::Screen::Routines);
        assert_eq!(app.routines[0].tasks.len(), 3);
        press(&mut app, "eds");
        app.key(key(Esc));
        press(&mut app, "g"); // guardar
        assert_eq!(app.screen, app::Screen::Routines);
        assert_eq!(app.routines[0].tasks.len(), 2);
        assert!(app.routines_dirty);
    }

    #[test]
    fn renaming_the_last_routine_keeps_it_as_the_last() {
        let mut app = app_with_routines();
        app.last_routine = Some("Mañana de estudio".into());
        press(&mut app, "2er");
        type_text(&mut app, "!");
        app.key(key(Enter));
        press(&mut app, "g");
        assert_eq!(app.last_routine.as_deref(), Some("Mañana de estudio!"));
        assert!(app.last_dirty);
    }

    #[test]
    fn meditation_screen_switches_mode_and_duration() {
        let mut app = app_with_routines();
        press(&mut app, "2ejjj");
        app.key(key(Enter));
        assert_eq!(app.screen, app::Screen::RoutineStep);
        let t = screen_text(&app, 80, 20);
        assert!(t.contains("Duración fija") && t.contains("10 minutos"), "{t}");
        app.key(key(Down));
        app.key(key(Down));
        app.key(key(Left)); // 10 -> 5
        app.key(key(Up));
        app.key(key(Right)); // modo libre
        let t = screen_text(&app, 80, 20);
        assert!(t.contains("se elige en el momento"), "{t}");
        app.key(key(Enter));
        assert_eq!(app.editor.as_ref().unwrap().routine.meditation, setting(None));
        press(&mut app, "g");
        assert_eq!(app.routines[0].meditation, setting(None));
        assert_eq!(app.routines[0].tasks.len(), 3);
    }

    #[test]
    fn meditation_can_be_turned_off_and_on_again() {
        let mut app = app_with_routines();
        press(&mut app, "2ejjj");
        app.key(key(Enter));
        app.key(key(Left)); // no
        let t = screen_text(&app, 80, 20);
        assert!(!t.contains("Duración fija"), "sin meditación no hay modo ni duración:\n{t}");
        app.key(key(Down));
        app.key(key(Right)); // sin efecto estando en «no»
        app.key(key(Enter));
        assert_eq!(app.editor.as_ref().unwrap().routine.meditation, None);
        assert!(screen_text(&app, 80, 24).contains("Meditación · no"));
        press(&mut app, "g");
        assert_eq!(app.routines[0].summary(), "3 pasos");
        press(&mut app, "ejjj");
        app.key(key(Enter));
        app.key(key(Right)); // sí: arranca en fija de 10
        app.key(key(Enter));
        assert_eq!(app.editor.as_ref().unwrap().routine.meditation, setting(Some(10)));
    }

    #[test]
    fn esc_on_the_meditation_screen_discards_the_change() {
        let mut app = app_with_routines();
        press(&mut app, "2ejjj");
        app.key(key(Enter));
        app.key(key(Down));
        app.key(key(Right));
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::RoutineEditor);
        assert!(!app.editor.as_ref().unwrap().dirty());
    }

    #[test]
    fn running_a_routine_silences_ambient_sounds_and_gives_them_back() {
        let mut app = app_with_routines();
        app.state.active = vec!["noise-brown".into()];
        press(&mut app, "1");
        assert!(matches!(sent(&mut app).as_slice(), [Command::PauseAll]));
        app.state.paused = true; // lo que contestaría el daemon
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::Home);
        assert!(app.run.is_none());
        assert!(matches!(sent(&mut app).as_slice(), [Command::PauseAll]));
    }

    #[test]
    fn running_does_not_touch_the_sound_when_nothing_was_playing() {
        let mut app = app_with_routines();
        press(&mut app, "1");
        app.key(key(Esc));
        assert!(sent(&mut app).is_empty());
    }

    #[test]
    fn marks_go_in_any_order_count_and_undo_without_asking() {
        let mut app = app_with_routines();
        press(&mut app, "1");
        assert!(screen_text(&app, 80, 24).contains("0 de 4 listos"));
        press(&mut app, "jjj"); // último paso primero
        press(&mut app, " ");
        press(&mut app, "k");
        press(&mut app, "k");
        press(&mut app, "s"); // saltea el 2
        let t = screen_text(&app, 80, 24);
        assert!(t.contains("2 de 4 listos") && t.contains("salteado"), "{t}");
        press(&mut app, "u");
        assert!(screen_text(&app, 80, 24).contains("1 de 4 listos"));
        assert_eq!(app.run.as_ref().unwrap().cursor, 1, "u vuelve al paso deshecho");
        press(&mut app, "uu");
        assert!(app.notice.as_ref().unwrap().0.contains("nada para deshacer"));
        assert!(screen_text(&app, 80, 24).contains("0 de 4 listos"));
        assert!(app.popup.is_none() && app.med_popup.is_none());
    }

    #[test]
    fn a_marked_step_is_not_marked_twice() {
        let mut app = app_with_routines();
        press(&mut app, "1 ");
        app.key(key(Up));
        press(&mut app, "s");
        assert_eq!(app.run.as_ref().unwrap().done_count(), 1);
        assert!(app.notice.is_some());
    }

    #[test]
    fn finishing_every_step_enters_focus_and_gives_back_the_ambient() {
        let mut app = app_with_routines();
        app.state.active = vec!["noise-brown".into()];
        press(&mut app, "1");
        sent(&mut app);
        app.state.paused = true;
        press(&mut app, " s   ");
        assert_eq!(app.screen, app::Screen::Focus);
        assert!(app.run.is_none());
        let out = sent(&mut app);
        assert!(out.iter().any(|c| matches!(c, Command::PauseAll)));
        assert!(out.iter().any(|c| matches!(c, Command::Watch { levels: true })));
    }

    #[test]
    fn fixed_meditation_gets_a_random_pick_and_enter_plays_then_toggles() {
        let mut app = app_with_routines();
        press(&mut app, "1jjj");
        let t = screen_text(&app, 90, 24);
        assert!(t.contains("Meditación 10 min · ") && t.contains("(al azar)"), "{t}");
        app.key(key(Enter));
        let id = match sent(&mut app).pop() {
            Some(Command::PlayMeditation { id }) => id,
            other => panic!("salió {other:?}"),
        };
        assert!(id == "calma" || id == "scan", "tiene que ser de la categoría de 10: {id}");
        app.state.meditation =
            Some(crate::protocol::MeditationPlayback { id, position_secs: 192, total_secs: 600, paused: false });
        assert!(screen_text(&app, 90, 24).contains("▶ 03:12 / 10:00"));
        app.key(key(Enter));
        assert!(matches!(sent(&mut app).pop(), Some(Command::ToggleMeditation)));
        app.state.meditation.as_mut().unwrap().paused = true;
        assert!(screen_text(&app, 90, 24).contains("⏸ 03:12 / 10:00"));
    }

    #[test]
    fn the_audio_ending_does_not_mark_the_step() {
        let mut app = app_with_routines();
        press(&mut app, "1jjj");
        app.key(key(Enter));
        app.state.meditation = None; // terminó
        assert_eq!(app.run.as_ref().unwrap().marks[3], crate::tui::routines::Mark::Pending);
    }

    #[test]
    fn free_meditation_asks_how_long_with_only_categories_that_have_something() {
        let mut app = app_with_routines();
        press(&mut app, "2j"); // «Tarde»
        app.key(key(Enter));
        press(&mut app, "j"); // paso 2: meditación libre
        let t = screen_text(&app, 80, 24);
        assert!(t.contains("Meditación · duración libre") && t.contains("Enter para elegir"), "{t}");
        app.key(key(Enter));
        let t = screen_text(&app, 80, 24);
        assert!(t.contains("¿cuánto tiempo?") && t.contains("5 minutos") && t.contains("10 minutos"), "{t}");
        assert!(!t.contains("20 minutos") && !t.contains("15 minutos"), "{t}");
        app.key(key(Down));
        app.key(key(Enter));
        let run = app.run.as_ref().unwrap();
        assert!(run.chooser.is_none());
        assert_eq!(run.picks[1].as_ref().unwrap().category, cat(10));
        assert!(sent(&mut app).is_empty(), "elegir la duración no reproduce todavía");
        app.key(key(Enter));
        assert!(matches!(sent(&mut app).pop(), Some(Command::PlayMeditation { .. })));
    }

    #[test]
    fn esc_in_the_duration_screen_goes_back_to_the_steps() {
        let mut app = app_with_routines();
        press(&mut app, "2j");
        app.key(key(Enter));
        press(&mut app, "j");
        app.key(key(Enter));
        app.key(key(Esc));
        assert_eq!(app.screen, app::Screen::RoutineRun);
        assert!(app.run.as_ref().unwrap().chooser.is_none());
    }

    #[test]
    fn fixed_step_without_meditations_warns_and_lets_pick_another_duration_or_skip() {
        let mut app = app_with_routines();
        app.routines[0].meditation = setting(Some(20));
        press(&mut app, "1jjj");
        let t = screen_text(&app, 90, 24);
        assert!(t.contains("No hay meditaciones de 20 minutos") && t.contains("s saltea"), "{t}");
        app.key(key(Enter));
        assert!(screen_text(&app, 90, 24).contains("¿cuánto tiempo?"));
        app.key(key(Enter)); // 5 minutos
        assert_eq!(app.run.as_ref().unwrap().picks[3].as_ref().unwrap().id, "resp");
        // Y la otra salida: saltearlo.
        let mut app = app_with_routines();
        app.routines[0].meditation = setting(Some(20));
        press(&mut app, "1jjjs");
        assert_eq!(app.run.as_ref().unwrap().marks[3], crate::tui::routines::Mark::Skipped);
    }

    #[test]
    fn meditations_that_are_not_downloaded_here_are_not_offered() {
        let mut app = app_with_routines();
        for m in &mut app.state.meditations {
            m.available = m.category != cat(5);
        }
        assert_eq!(app.available_categories(), vec![cat(10)]);
    }

    #[test]
    fn skipping_the_playing_meditation_stops_it() {
        let mut app = app_with_routines();
        press(&mut app, "1jjj");
        app.key(key(Enter));
        sent(&mut app);
        press(&mut app, "s");
        assert!(matches!(sent(&mut app).as_slice(), [Command::StopMeditation]));
    }

    #[test]
    fn leaving_a_routine_stops_the_meditation_it_started() {
        let mut app = app_with_routines();
        press(&mut app, "1jjj");
        app.key(key(Enter));
        sent(&mut app);
        app.key(key(Esc));
        assert!(sent(&mut app).iter().any(|c| matches!(c, Command::StopMeditation)));
    }

    #[test]
    fn routine_screens_do_not_animate() {
        let mut app = app_with_routines();
        press(&mut app, "1");
        assert!(!app.animating());
    }

    #[test]
    fn q_asks_before_quitting_on_every_screen_but_focus() {
        use app::Screen::*;
        // (teclas para llegar, pantalla esperada)
        let routes = [
            ("", Home),
            ("4", Config),
            ("4jk", Config),
            ("4j\n", Meditations),
            ("4jj\n", Sounds),
            ("4jjj\n", Presets),
            ("4jjj\nn", PresetBuilder),
            ("2", Routines),
            ("2je", RoutineEditor),
            ("2jej\n", RoutineStep),
            ("1", RoutineRun),
        ];
        for (keys, screen) in routes {
            let mut app = app_with_routines();
            for c in keys.chars() {
                if c == '\n' {
                    app.key(key(Enter));
                } else {
                    press(&mut app, &c.to_string());
                }
            }
            assert_eq!(app.screen, screen, "ruta «{keys}»");
            sent(&mut app);

            press(&mut app, "q");
            assert!(matches!(app.popup, Some(app::Popup::ConfirmQuit)), "{screen:?}: q pregunta");
            assert!(app.exit.is_none(), "{screen:?}: todavía no sale");
            assert!(screen_text(&app, 80, 20).contains("¿Salir de WorkHub?"));

            app.key(key(Esc));
            assert!(app.popup.is_none() && app.exit.is_none(), "{screen:?}: Esc cancela");
            assert_eq!(app.screen, screen);

            press(&mut app, "qn");
            assert!(app.popup.is_none() && app.exit.is_none(), "{screen:?}: n cancela");
            assert_eq!(app.screen, screen);

            press(&mut app, "qs");
            assert!(app.exit == Some(app::Exit::Quit), "{screen:?}: s confirma");
            app.exit = None;
            press(&mut app, "q");
            app.key(key(Enter));
            assert!(app.exit == Some(app::Exit::Quit), "{screen:?}: Enter confirma");
        }
    }

    #[test]
    fn q_in_a_text_popup_types_instead_of_quitting() {
        let mut app = app_with_routines();
        press(&mut app, "2");
        press(&mut app, "n");
        assert!(app.editor.is_some());
        press(&mut app, "q");
        assert!(app.popup.is_none() && app.exit.is_none());
    }
}
