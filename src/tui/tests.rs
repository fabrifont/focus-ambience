use super::*;
use crate::library::{self, Preset};
use crate::meditation::Category;
use crate::routine::{MeditationSetting, Routine};
use ratatui::crossterm::event::{KeyCode, KeyCode::*, KeyEvent, KeyModifiers};
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
        let [scene, _, _] = ui::areas(ratatui::layout::Rect::new(0, 0, w, h), app.deep != app::Deep::Off);
        app.animate(0.05, scene.width, scene.height);
    }
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

#[test]
fn wait_until_is_none_once_the_deadline_passed() {
    let now = Instant::now();
    assert_eq!(wait_until(now + FRAME_IDLE, now), Some(FRAME_IDLE));
    assert_eq!(wait_until(now, now), None);
    assert_eq!(wait_until(now, now + FRAME_BUSY), None);
    assert!(frame_for(true) < frame_for(false));
}

#[test]
fn levels_do_not_force_a_redraw_but_state_and_keys_do() {
    let mut app = App::new();
    assert!(!handle(&mut app, Msg::Daemon(Some(Event::Levels { layers: vec![], master: 0.0 }))).unwrap());
    assert!(handle(&mut app, Msg::Daemon(Some(Event::Notice { text: "hola".into() }))).unwrap());
    assert!(handle(&mut app, Msg::Term(TermEvent::Resize(80, 24))).unwrap());
    assert!(handle(&mut app, Msg::Daemon(None)).is_err());
}

#[test]
fn shown_volume_starts_at_the_real_one_and_then_eases_towards_it() {
    let mut app = app_with_sounds();
    app.go(app::Screen::Focus);
    app.animate(0.05, 80, 20);
    assert!(app.state.sounds.iter().all(|s| app.shown_volume[&s.id] == app.volume(&s.id)));
    app.state.volumes.insert("lluvia".into(), 1.0);
    app.animate(0.05, 80, 20);
    let shown = app.shown_volume["lluvia"];
    assert!(shown > 0.5 && shown < 1.0, "{shown}");
    for _ in 0..100 {
        app.animate(0.05, 80, 20);
    }
    assert!((app.shown_volume["lluvia"] - 1.0).abs() < 0.001);
}

#[test]
fn snapshot() {
    println!(
        "{}",
        render(110, 34, |a| {
            a.state.active = vec!["noise-brown".into(), "binaural-40".into()];
            a.set_levels(vec![("noise-brown".into(), 0.2), ("binaural-40".into(), 0.15)]);
            a.cursor = 2;
            a.state.timer = Some(crate::protocol::TimerState { total_secs: 3000, remaining_secs: 1873, paused: false });
        })
    );
    println!("{}", render(70, 24, |a| a.help = Some(help::Topic::Focus)));
}

// ------------------------------------------------------------ ayudas de prueba

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Escribe `keys` letra por letra (el espacio vale Enter, como en la app).
fn press(app: &mut App, keys: &str) {
    for c in keys.chars() {
        app.key(key(Char(c)));
    }
}

fn ctrl(app: &mut App, keys: &str) {
    for c in keys.chars() {
        app.key(KeyEvent::new(Char(c), KeyModifiers::CONTROL));
    }
}

/// Teclas separadas por espacios: `down enter esc tab left right up bs`, `down*3` las repite y
/// una letra suelta (`t`, `0`, `?`) se escribe tal cual.
fn ks(app: &mut App, script: &str) {
    for tok in script.split_whitespace() {
        let (name, n) = match tok.split_once('*') {
            Some((name, n)) if !name.is_empty() => (name, n.parse::<usize>().expect("repeticiones")),
            _ => (tok, 1),
        };
        let code = match name {
            "up" => Up,
            "down" => Down,
            "left" => Left,
            "right" => Right,
            "enter" => Enter,
            "esc" => Esc,
            "tab" => Tab,
            "bs" => Backspace,
            "home" => Home,
            "end" => End,
            "del" => Delete,
            "backtab" => BackTab,
            c if c.chars().count() == 1 => Char(c.chars().next().unwrap()),
            other => panic!("tecla desconocida «{other}»"),
        };
        for _ in 0..n {
            app.key(key(code));
        }
    }
}

fn sent(app: &mut App) -> Vec<Command> {
    std::mem::take(&mut app.outbox)
}

fn type_text(app: &mut App, text: &str) {
    press(app, text);
}

fn screen_text(app: &App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

/// Opción `i` del menú de inicio (0 Continuar, 1 Elegir otra rutina, 2 Ir a Focus, 3 Configuración, 4 Salir).
fn home(app: &mut App, i: usize) {
    app.home_cursor = 0;
    for _ in 0..i {
        app.key(key(Down));
    }
    app.key(key(Enter));
}

/// Entrada `i` de Configuración (0 Rutinas, 1 Meditaciones, 2 Sonidos, 3 Presets, 4 Sincronización).
fn config(app: &mut App, i: usize) {
    home(app, 3);
    app.config_cursor = 0;
    for _ in 0..i {
        app.key(key(Down));
    }
    app.key(key(Enter));
}

fn med(id: &str, name: &str, minutes: u32) -> crate::meditation::Meditation {
    crate::meditation::Meditation {
        id: id.into(),
        name: name.into(),
        category: Category::try_from(minutes).unwrap(),
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

fn app_with_presets() -> App {
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

// ------------------------------------------------------------------- inicio

fn app_with_routines() -> App {
    let mut app = app_with_meditations();
    app.routines =
        vec![sample_routine(), Routine { name: "Tarde".into(), tasks: vec!["x".into()], meditation: setting(None) }];
    app
}

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

#[test]
fn starts_on_the_home_menu_with_its_five_options() {
    let app = app_with_routines();
    assert_eq!(app.screen, app::Screen::Home);
    let t = screen_text(&app, 80, 20);
    for s in
        ["Continuar", "Elegir otra rutina", "Ir a Focus", "Configuración", "Salir", "↑↓ mover   Enter elegir   ? ayuda"]
    {
        assert!(t.contains(s), "falta «{s}» en:\n{t}");
    }
    for gone in ["1-4", "q salir", "directo"] {
        assert!(!t.contains(gone), "no tendría que decir «{gone}»:\n{t}");
    }
}

#[test]
fn go_to_focus_and_esc_back_toggle_the_level_stream() {
    let mut app = App::new();
    home(&mut app, 2);
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(matches!(app.outbox.last(), Some(Command::Watch { levels: true })));
    ks(&mut app, "esc"); // saltea la ventanita
    assert_eq!(app.screen, app::Screen::Focus);
    ks(&mut app, "esc");
    assert_eq!(app.screen, app::Screen::Home);
    assert!(matches!(app.outbox.last(), Some(Command::Watch { levels: false })));
}

#[test]
fn home_has_a_salir_row_that_asks_before_leaving() {
    // En silencio: «¿Salir de WorkHub?» con «No» marcada; Enter no sale, Esc tampoco.
    let mut app = App::new();
    ks(&mut app, "down*10");
    assert_eq!(app.home_cursor, 4, "Salir es la quinta fila");
    ks(&mut app, "enter");
    assert!(matches!(app.popup, Some(app::Popup::ConfirmQuit { sel: 1 })));
    let t = screen_text(&app, 80, 20);
    assert!(t.contains("¿Salir de WorkHub?") && t.contains("Sí, salir") && t.contains("> No"), "{t}");
    ks(&mut app, "enter");
    assert!(app.popup.is_none() && app.exit.is_none(), "Enter sobre «No» no sale");
    ks(&mut app, "enter esc");
    assert!(app.popup.is_none() && app.exit.is_none(), "Esc es «no»");
    ks(&mut app, "enter up enter");
    assert_eq!(app.exit, Some(app::Exit::Quit));
}

#[test]
fn quitting_with_sound_playing_offers_background_or_off_with_back_preselected() {
    let mut app = App::new();
    app.state.active = vec!["noise-brown".into()];
    ks(&mut app, "down*4 enter");
    assert!(matches!(app.popup, Some(app::Popup::Quit { sel: 2 })));
    let t = screen_text(&app, 80, 20);
    for s in ["Seguir escuchando de fondo", "Apagar todo y salir", "> Volver"] {
        assert!(t.contains(s), "falta «{s}» en:\n{t}");
    }
    ks(&mut app, "enter");
    assert!(app.popup.is_none() && app.exit.is_none(), "Enter de entrada solo vuelve");
    ks(&mut app, "down*4 enter esc");
    assert!(app.popup.is_none() && app.exit.is_none(), "Esc vuelve");
    ks(&mut app, "enter up*2 enter");
    assert_eq!(app.exit, Some(app::Exit::Detach));
    app.exit = None;
    ks(&mut app, "enter up enter");
    assert_eq!(app.exit, Some(app::Exit::Quit));
}

// ------------------------------------------------- ventanita de Focus

#[test]
fn going_to_focus_opens_the_time_step_with_the_last_value() {
    let mut app = app_with_presets();
    app.focus_minutes = 35;
    home(&mut app, 2);
    assert_eq!(app.screen, app::Screen::Focus);
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 35, typed: false }));
    let t = screen_text(&app, 90, 30);
    assert!(t.contains("Antes de arrancar · 1 de 2") && t.contains("[ 35 ] minutos"), "{t}");
    assert!(app.outbox.iter().all(|c| !matches!(c, Command::Timer { .. })), "todavía no arranca nada");
}

#[test]
fn going_to_focus_skips_the_popup_when_a_sound_is_playing() {
    let mut app = app_with_presets();
    app.state.active = vec!["rain".into()];
    app.state.paused = false;
    home(&mut app, 2);
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(app.popup.is_none());
    assert_eq!(app.deep, app::Deep::Off);
}

#[test]
fn going_to_focus_skips_the_popup_when_the_timer_is_running() {
    let mut app = app_with_presets();
    app.state.timer = Some(crate::protocol::TimerState { total_secs: 600, remaining_secs: 300, paused: false });
    home(&mut app, 2);
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(app.popup.is_none());
}

#[test]
fn going_to_focus_skips_the_popup_when_sounds_are_only_paused_and_no_timer() {
    let mut app = app_with_presets();
    app.state.active = vec!["rain".into()];
    app.state.paused = true;
    home(&mut app, 2);
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(app.popup.is_none());
}

#[test]
fn the_first_run_starts_at_fifty_minutes() {
    let mut app = App::new();
    home(&mut app, 2);
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 50, typed: false }));
}

#[test]
fn arrows_change_the_time_by_five_and_never_below_one() {
    let mut app = App::new();
    home(&mut app, 2);
    app.key(key(Up));
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 55, typed: false }));
    ks(&mut app, "down*20");
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 1, typed: false }));
}

#[test]
fn typing_replaces_the_value_then_appends_and_backspace_erases() {
    let mut app = App::new();
    home(&mut app, 2);
    press(&mut app, "4");
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 4, typed: true }));
    press(&mut app, "5");
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 45, typed: true }));
    press(&mut app, "99"); // 4599 no entra: el tope es 999
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 459, typed: true }));
    app.key(key(Backspace));
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 45, typed: true }));
    press(&mut app, "x"); // letras no hacen nada
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 45, typed: true }));
}

#[test]
fn j_and_k_no_longer_change_the_minutes_or_the_preset() {
    let mut app = app_with_presets();
    home(&mut app, 2);
    press(&mut app, "kjkj");
    assert_eq!(intro(&app), Some(app::Intro::Time { minutes: 50, typed: false }));
    ks(&mut app, "enter");
    press(&mut app, "jjkk");
    assert_eq!(intro(&app), Some(app::Intro::Sounds { minutes: 50, sel: 0 }));
}

#[test]
fn zero_minutes_cannot_continue() {
    let mut app = App::new();
    home(&mut app, 2);
    press(&mut app, "0");
    app.key(key(Enter));
    assert!(matches!(intro(&app), Some(app::Intro::Time { minutes: 0, .. })));
    assert!(app.notice.is_some());
}

#[test]
fn esc_on_the_time_step_goes_straight_to_focus_without_starting_anything() {
    let mut app = app_with_presets();
    home(&mut app, 2);
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
    home(&mut app, 2);
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
    home(&mut app, 2);
    app.key(key(Enter));
    sent(&mut app);
    app.key(key(Esc));
    assert!(app.popup.is_none());
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(sent(&mut app).is_empty());
    assert_eq!(app.deep, app::Deep::Off);
}

#[test]
fn choosing_a_preset_starts_the_timer_and_loads_it() {
    let mut app = app_with_presets();
    home(&mut app, 2);
    press(&mut app, "30");
    ks(&mut app, "enter down enter");
    assert!(app.popup.is_none());
    let out = sent(&mut app);
    assert!(out.iter().any(|c| matches!(c, Command::Timer { minutes: Some(30) })));
    assert!(out.iter().any(|c| matches!(c, Command::LoadPreset { name } if name == "Lluvia y fuego")));
    assert_eq!(app.deep, app::Deep::On, "con preset arranca el modo deep work");
}

#[test]
fn custom_sounds_starts_only_the_timer() {
    let mut app = app_with_presets();
    home(&mut app, 2);
    ks(&mut app, "enter down*3"); // más allá de la última fila no pasa
    assert_eq!(intro(&app), Some(app::Intro::Sounds { minutes: 50, sel: 2 }));
    app.key(key(Enter));
    let out = sent(&mut app);
    assert!(out.iter().any(|c| matches!(c, Command::Timer { minutes: Some(50) })));
    assert!(!out.iter().any(|c| matches!(c, Command::LoadPreset { .. })));
    assert_eq!(app.screen, app::Screen::Focus);
    assert_eq!(app.deep, app::Deep::Off, "con sonidos personalizados no");
}

#[test]
fn without_presets_the_only_option_is_custom_sounds() {
    let mut app = App::new();
    home(&mut app, 2);
    app.key(key(Enter));
    assert!(screen_text(&app, 90, 30).contains("Sonidos personalizados"));
    app.key(key(Enter));
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Timer { minutes: Some(50) })));
}

#[test]
fn the_window_also_opens_when_a_routine_ends() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    press(&mut app, "   "); // las tres tareas
    assert_eq!(app.screen, app::Screen::RoutineRun, "falta la meditación");
    let id = start_meditation(&mut app);
    sent(&mut app);
    end_meditation(&mut app, &id);
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(matches!(intro(&app), Some(app::Intro::Time { .. })));
}

#[test]
fn it_shows_up_every_time_focus_is_entered() {
    let mut app = App::new();
    home(&mut app, 2);
    ks(&mut app, "esc esc");
    assert_eq!(app.screen, app::Screen::Home);
    home(&mut app, 2);
    assert!(intro(&app).is_some());
}

#[test]
fn the_window_cannot_quit_the_app() {
    let mut app = App::new();
    home(&mut app, 2);
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
    config(app, 2);
    assert_eq!(app.screen, app::Screen::Sounds);
}

#[test]
fn config_opens_the_sounds_list_with_downloaded_sounds_only() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    let t = screen_text(&app, 80, 24);
    for s in ["+ Agregar desde YouTube", "Templo Jedi", "Lluvia", "☁ Fuego", "↑↓ mover   Enter elegir"] {
        assert!(t.contains(s), "falta «{s}» en:\n{t}");
    }
    assert!(!t.contains("Ruido blanco") && !t.contains("Binaural"), "los generados no se listan:\n{t}");
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Config);
}

#[test]
fn the_plus_row_adds_a_sound_with_the_same_form_and_flow_as_before() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    assert_eq!(app.sound_cursor, 0, "arranca sobre el primer sonido");
    ks(&mut app, "down*40");
    assert_eq!(app.sound_cursor, app.file_sounds().len(), "la fila «+» es la última");
    ks(&mut app, "enter");
    assert!(matches!(app.popup, Some(app::Popup::Add(_))));
    assert!(screen_text(&app, 90, 30).contains("agregar desde YouTube"));
    app.paste("https://youtu.be/xyz");
    ks(&mut app, "enter*4"); // URL -> nombre -> tema -> recorte -> descargar
    assert!(app.popup.is_none());
    match sent(&mut app).pop() {
        Some(Command::Add(r)) => assert_eq!(r.url, "https://youtu.be/xyz"),
        other => panic!("salió {other:?}"),
    }
}

#[test]
fn empty_sounds_list_still_has_the_plus_row_and_says_what_to_do() {
    let mut app = App::new();
    open_sounds(&mut app);
    let t = screen_text(&app, 100, 24);
    assert!(t.contains("Todavía no hay sonidos descargados") && t.contains("+ Agregar desde YouTube"), "{t}");
    assert_eq!(app.sound_cursor, 0, "con la lista vacía arranca sobre la fila «+»");
    ks(&mut app, "down*3");
    assert_eq!(app.sound_cursor, 0);
    ks(&mut app, "enter");
    assert!(matches!(app.popup, Some(app::Popup::Add(_))));
}

#[test]
fn the_add_form_moves_between_fields_with_arrows_and_picks_the_theme_with_left_right() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    ks(&mut app, "down*40 enter");
    let field = |app: &App| match &app.popup {
        Some(app::Popup::Add(f)) => (f.field, f.url.clone(), f.name.clone(), f.trim.clone(), f.theme),
        _ => panic!("falta el formulario"),
    };
    press(&mut app, "a b");
    assert_eq!(field(&app).1, "a b", "el espacio se escribe en la URL");
    ks(&mut app, "down");
    press(&mut app, "c d");
    assert_eq!(field(&app).2, "c d");
    ks(&mut app, "down");
    let theme = field(&app).4;
    press(&mut app, " ");
    assert_eq!(field(&app).0, 2, "espacio no avanza de campo");
    assert_eq!(field(&app).4, theme);
    ks(&mut app, "right");
    assert_eq!(field(&app).4, theme + 1);
    ks(&mut app, "left*2");
    assert_eq!(field(&app).4, app::ADD_THEMES.len() - 1, "da la vuelta");
    ks(&mut app, "down");
    press(&mut app, "1 2x");
    let f = field(&app);
    assert_eq!((f.0, f.3.as_str()), (3, "12"));
    ks(&mut app, "up*3");
    assert_eq!(field(&app).0, 0);
    let t = screen_text(&app, 90, 30);
    assert!(t.contains("↑↓ campo") && !t.contains("tab"), "{t}");
    assert!(sent(&mut app).is_empty());
}

#[test]
fn tab_does_nothing_in_the_add_form() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    ks(&mut app, "down*40 enter tab backtab");
    match &app.popup {
        Some(app::Popup::Add(f)) => assert_eq!(f.field, 0),
        _ => panic!("falta el formulario"),
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
fn enter_on_a_sound_asks_before_deleting_it_with_no_preselected() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    ks(&mut app, "down");
    // El orden del estado manda para el cursor (en pantalla van agrupados por tema).
    let first = app.selected_file_sound().unwrap().name.clone();
    ks(&mut app, "enter");
    assert!(sent(&mut app).is_empty());
    let t = screen_text(&app, 80, 24);
    assert!(t.contains(&format!("¿Borrar «{first}»?")) && t.contains("Sí, borrar") && t.contains("> No"), "{t}");
    assert!(matches!(app.popup, Some(app::Popup::DeleteSound { sel: 1, .. })));
    ks(&mut app, "esc");
    assert!(app.popup.is_none() && sent(&mut app).is_empty(), "Esc = no");
    ks(&mut app, "enter enter");
    assert!(app.popup.is_none() && sent(&mut app).is_empty(), "Enter sobre «No» no borra");
    ks(&mut app, "down enter up enter");
    let second = app.selected_file_sound().unwrap().id.clone();
    assert!(matches!(sent(&mut app).pop(), Some(Command::DeleteSound { id }) if id == second));
}

#[test]
fn the_sound_cursor_stays_inside_the_list_after_a_delete() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    ks(&mut app, "down*9");
    assert_eq!(app.sound_cursor, 3, "tres sonidos y la fila «+»");
    let mut st = app.state.clone();
    st.sounds.remove(2);
    app.set_state(st);
    assert_eq!(app.sound_cursor, 2);
}

#[test]
fn the_sounds_screen_is_still() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    assert!(!app.animating());
}

// ------------------------------------------------- Configuración > Presets

fn open_presets(app: &mut App) {
    config(app, 3);
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
    for s in ["+ Nuevo preset", "Templo Jedi", "Ruido marrón 30%", "Lluvia y fuego", "Enter elegir"] {
        assert!(t.contains(s), "falta «{s}» en:\n{t}");
    }
    let pos = |s: &str| t.find(s).unwrap();
    assert!(pos("+ Nuevo preset") > pos("Lluvia y fuego"), "la fila «+» va al final");
    assert!(t.contains("> Templo Jedi"), "el cursor arranca en el primer preset:\n{t}");
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Config);
}

#[test]
fn empty_presets_list_has_the_plus_row() {
    let mut app = App::new();
    open_presets(&mut app);
    assert!(screen_text(&app, 100, 24).contains("Todavía no hay presets"));
    assert_eq!(app.preset_list_cursor, 0, "con la lista vacía arranca sobre la fila «+»");
    ks(&mut app, "down*2");
    assert_eq!(app.preset_list_cursor, 0);
    ks(&mut app, "enter");
    assert_eq!(app.screen, app::Screen::PresetBuilder);
}

#[test]
fn enter_on_a_preset_asks_before_deleting_it_with_no_preselected() {
    let mut app = app_with_sounds_and_presets();
    open_presets(&mut app);
    ks(&mut app, "down");
    ks(&mut app, "enter");
    assert!(sent(&mut app).is_empty());
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("¿Borrar «Lluvia y fuego»?") && t.contains("Sí, borrar") && t.contains("> No"), "{t}");
    ks(&mut app, "esc");
    assert!(app.popup.is_none() && sent(&mut app).is_empty());
    ks(&mut app, "enter enter");
    assert!(app.popup.is_none() && sent(&mut app).is_empty());
    ks(&mut app, "enter up enter");
    assert!(matches!(sent(&mut app).pop(), Some(Command::DeletePreset { name }) if name == "Lluvia y fuego"));
}

fn open_builder(app: &mut App) {
    open_presets(app);
    ks(app, "down*40 enter");
    assert_eq!(app.screen, app::Screen::PresetBuilder);
}

/// Baja hasta «✓ Guardar preset» y lo elige.
fn builder_save(app: &mut App) {
    ks(app, "down*40 enter");
}

#[test]
fn the_builder_lists_every_sound_with_volume_and_a_save_row_and_starts_silent() {
    let mut app = app_with_sounds_and_presets();
    sent(&mut app);
    open_builder(&mut app);
    let t = screen_text(&app, 90, 40);
    for s in [
        "Elegí los sonidos",
        "Templo Jedi",
        "Lluvia",
        "Ruido blanco",
        "Binaural",
        " 70%",
        "✓ Guardar preset",
        "←→ volumen",
    ] {
        assert!(t.contains(s), "falta «{s}» en:\n{t}");
    }
    assert!(!t.contains("g guardar"), "{t}");
    assert!(sent(&mut app).iter().all(|c| matches!(c, Command::Watch { .. })), "no manda comandos de audio");
    assert!(!app.animating());
}

#[test]
fn building_a_preset_picks_sounds_sets_volumes_and_saves_it_with_a_name() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    ks(&mut app, "enter right*3"); // primero: Templo Jedi (volumen por defecto 50 %), +15 %
    ks(&mut app, "down enter left*2"); // Lluvia parte de 70 %
    builder_save(&mut app);
    assert!(matches!(app.popup, Some(app::Popup::SavePreset { .. })));
    type_text(&mut app, "Estudio");
    app.key(key(Enter));
    assert!(app.popup.is_none());
    assert_eq!(app.screen, app::Screen::Presets);
    assert!(app.builder.is_none());
    match sent(&mut app).into_iter().find(|c| matches!(c, Command::StorePreset { .. })) {
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
fn the_save_row_without_sounds_does_not_save() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    builder_save(&mut app);
    assert!(app.popup.is_none(), "sin sonidos no se puede guardar");
    assert!(app.notice.is_some());
}

#[test]
fn left_right_on_the_save_row_do_nothing() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    ks(&mut app, "down*40");
    sent(&mut app);
    ks(&mut app, "left right");
    assert!(sent(&mut app).is_empty());
}

#[test]
fn sounds_missing_on_this_machine_cannot_go_in_a_preset() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    ks(&mut app, "down*2 enter"); // Fuego: ☁
    assert!(app.builder.as_ref().unwrap().picked.is_empty());
    assert!(app.notice.is_some());
}

#[test]
fn esc_in_the_builder_asks_to_discard_with_keep_building_preselected() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    ks(&mut app, "enter");
    builder_save(&mut app);
    app.key(key(Esc)); // cancela el nombre, no el borrador
    assert!(app.popup.is_none() && app.builder.as_ref().is_some_and(|b| b.picked.len() == 1));
    assert_eq!(app.screen, app::Screen::PresetBuilder);
    sent(&mut app);
    app.key(key(Esc)); // con sonidos elegidos pregunta
    assert!(matches!(app.popup, Some(app::Popup::DiscardPreset { sel: 1 })));
    let t = screen_text(&app, 90, 30);
    assert!(t.contains("Sí, descartar") && t.contains("> Seguir armando"), "{t}");
    press(&mut app, "xsyn"); // las letras no hacen nada
    assert!(matches!(app.popup, Some(app::Popup::DiscardPreset { .. })));
    app.key(key(Esc)); // Esc sigue armando
    assert!(app.popup.is_none() && app.builder.is_some());
    ks(&mut app, "esc enter"); // Enter sobre la fila marcada también sigue armando
    assert!(app.popup.is_none() && app.builder.is_some());
    sent(&mut app);
    ks(&mut app, "esc up enter"); // «Sí, descartar»
    assert_eq!(app.screen, app::Screen::Presets);
    assert!(app.builder.is_none());
    assert!(!sent(&mut app).iter().any(|c| matches!(c, Command::StorePreset { .. })));
}

#[test]
fn esc_in_an_empty_builder_leaves_without_asking() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    app.key(key(Esc));
    assert!(app.popup.is_none() && app.builder.is_none());
    assert_eq!(app.screen, app::Screen::Presets);
}

fn with_focus_playing(ids: &[&str], paused: bool) -> App {
    let mut app = app_with_sounds_and_presets();
    let mut st = app.state.clone();
    st.active = ids.iter().map(|s| s.to_string()).collect();
    st.paused = paused;
    app.set_state(st);
    app
}

fn active_ids(cmds: &[Command]) -> Vec<(String, bool)> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::SetActive { id, active } => Some((id.clone(), *active)),
            _ => None,
        })
        .collect()
}

#[test]
fn entering_the_builder_silences_what_played_in_focus() {
    let mut app = with_focus_playing(&["lluvia"], false);
    sent(&mut app);
    open_builder(&mut app);
    assert_eq!(active_ids(&sent(&mut app)), [("lluvia".to_string(), false)]);
}

#[test]
fn picked_sounds_play_live_with_their_volume() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    sent(&mut app);
    ks(&mut app, "enter"); // Templo Jedi, 50 %
    let cmds = sent(&mut app);
    assert!(matches!(&cmds[0], Command::SetVolume { id, volume } if id == "templo" && (*volume - 0.5).abs() < 1e-4));
    assert_eq!(active_ids(&cmds), [("templo".to_string(), true)]);
    ks(&mut app, "right");
    assert!(
        matches!(sent(&mut app).pop(), Some(Command::SetVolume { id, volume }) if id == "templo" && (volume - 0.55).abs() < 1e-4)
    );
    ks(&mut app, "down right"); // Lluvia, sin elegir: el volumen no suena
    assert!(sent(&mut app).is_empty());
    ks(&mut app, "up enter");
    assert_eq!(active_ids(&sent(&mut app)), [("templo".to_string(), false)]);
}

#[test]
fn leaving_the_builder_with_esc_restores_what_played_before() {
    let mut app = with_focus_playing(&["lluvia"], false);
    open_builder(&mut app);
    ks(&mut app, "enter");
    sent(&mut app);
    ks(&mut app, "esc up enter"); // confirma el descarte
    let cmds = sent(&mut app);
    assert_eq!(
        active_ids(&cmds),
        [("templo".to_string(), false), ("lluvia".to_string(), true)],
        "apaga el preview y vuelve lo de antes"
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, Command::SetVolume { id, volume } if id == "lluvia" && (*volume - 0.7).abs() < 1e-4))
    );
    assert!(!cmds.iter().any(|c| matches!(c, Command::PauseAll)));
}

#[test]
fn ctrl_c_from_the_builder_restores_the_audio_like_esc() {
    let mut app = with_focus_playing(&["lluvia"], false);
    open_builder(&mut app);
    ks(&mut app, "enter");
    sent(&mut app);
    ctrl(&mut app, "c");
    assert!(app.exit.is_some());
    assert_eq!(active_ids(&sent(&mut app)), [("templo".to_string(), false), ("lluvia".to_string(), true)]);
    assert!(app.builder.is_none());
}

#[test]
fn saving_restores_too_and_keeps_a_pause_and_silence_when_nothing_played() {
    let mut app = with_focus_playing(&["lluvia"], true);
    open_builder(&mut app);
    ks(&mut app, "enter");
    builder_save(&mut app);
    type_text(&mut app, "Nuevo");
    sent(&mut app);
    app.key(key(Enter));
    let cmds = sent(&mut app);
    assert!(matches!(cmds.first(), Some(Command::SetActive { id, active: false }) if id == "templo"));
    assert!(cmds.iter().any(|c| matches!(c, Command::PauseAll)), "estaba en pausa: {cmds:?}");
    assert!(cmds.iter().any(|c| matches!(c, Command::StorePreset { .. })));

    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    ks(&mut app, "enter");
    sent(&mut app);
    ks(&mut app, "esc up enter"); // confirma el descarte
    assert_eq!(active_ids(&sent(&mut app)), [("templo".to_string(), false)], "queda en silencio");
}

#[test]
fn the_preset_needs_a_name_and_it_cannot_repeat_one() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    ks(&mut app, "enter");
    builder_save(&mut app);
    sent(&mut app);
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
    ks(&mut app, "enter");
    builder_save(&mut app);
    app.paste("Mi mezcla\n");
    match &app.popup {
        Some(app::Popup::SavePreset { name }) => assert_eq!(name, "Mi mezcla"),
        _ => panic!("no hay popup"),
    }
}

#[test]
fn the_save_popup_types_every_character() {
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    ks(&mut app, "enter");
    builder_save(&mut app);
    type_text(&mut app, "q? 0 t");
    match &app.popup {
        Some(app::Popup::SavePreset { name }) => assert_eq!(name, "q? 0 t"),
        _ => panic!("no hay popup"),
    }
    assert!(app.help.is_none() && app.exit.is_none());
}

// ------------------------------------------------------------ Focus

fn focus_app() -> App {
    let mut app = app_with_sounds_and_presets();
    home(&mut app, 2);
    app.key(key(Esc)); // saltea la ventanita
    app.outbox.clear();
    app
}

fn with_timer(app: &mut App, remaining_secs: u64) {
    let mut s = app.state.clone();
    s.timer = Some(crate::protocol::TimerState { total_secs: 3000, remaining_secs, paused: false });
    app.set_state(s);
}

fn without_timer(app: &mut App) {
    let mut s = app.state.clone();
    s.timer = None;
    app.set_state(s);
}

fn paused_timer(app: &mut App, remaining_secs: u64) {
    let mut s = app.state.clone();
    s.timer = Some(crate::protocol::TimerState { total_secs: 3000, remaining_secs, paused: true });
    app.set_state(s);
}

fn pause_cmds(app: &mut App) -> usize {
    app.outbox.drain(..).filter(|c| matches!(c, Command::ToggleTimerPause)).count()
}

/// Prende el modo deep work por Tab › Modo deep work.
fn deep_on(app: &mut App) {
    ks(app, "tab down enter");
    assert_eq!(app.deep, app::Deep::On);
    sent(app);
}

#[test]
fn focus_has_no_presets_panel_and_no_removed_hints() {
    let mut app = app_with_sounds_and_presets();
    home(&mut app, 2);
    app.key(key(Esc));
    for (w, h) in [(80, 24), (120, 34), (200, 50)] {
        let t = screen_text(&app, w, h);
        assert!(!t.contains("PRESETS") && !t.contains("Lluvia y fuego"), "{w}x{h}\n{t}");
        for gone in ["guardar", "agregar", "borrar", "cargar", "timer", "reloj", "salir", "pausa"] {
            assert!(!t.contains(gone), "{w}x{h}: no tendría que decir «{gone}»:\n{t}");
        }
    }
    let t = screen_text(&app, 120, 34);
    assert!(t.contains("Enter sonido  ←→ volumen  Tab opciones  Esc inicio  ? ayuda"), "{t}");
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("Tab opciones") && t.contains("? ayuda"), "{t}");
}

#[test]
fn tab_in_focus_opens_options_and_shift_tab_does_nothing() {
    let mut app = focus_app();
    ks(&mut app, "backtab");
    assert!(app.popup.is_none());
    ks(&mut app, "tab");
    assert!(matches!(app.popup, Some(app::Popup::Options { sel: 0 })));
    ks(&mut app, "esc");
    assert!(app.popup.is_none());
}

#[test]
fn tab_does_nothing_outside_focus() {
    let mut app = app_with_routines();
    ks(&mut app, "tab");
    assert!(app.popup.is_none() && app.screen == app::Screen::Home);
    let mut app = app_with_presets();
    home(&mut app, 2);
    ks(&mut app, "tab");
    assert!(matches!(app.popup, Some(app::Popup::Intro(_))), "dentro de «Antes de arrancar» tampoco");
}

#[test]
fn options_menu_lists_its_rows_in_order_with_the_volume_bar() {
    let mut app = focus_app();
    app.state.master = 0.7;
    ks(&mut app, "tab");
    let t = screen_text(&app, 80, 24);
    let rows = [
        "Timer ›",
        "Modo deep work",
        "Cargar preset ›",
        "Volumen general",
        "Pausar todo",
        "Apagar todo",
        "Volver al inicio",
        "Salir",
    ];
    let mut last = 0;
    for r in rows {
        let at = t.find(r).unwrap_or_else(|| panic!("falta «{r}» en:\n{t}"));
        assert!(at >= last, "«{r}» fuera de orden:\n{t}");
        last = at;
    }
    assert!(t.contains("opciones") && t.contains("> Timer ›") && t.contains("70 %"), "{t}");
    assert!(t.contains("↑↓ mover  Enter elegir  Esc cerrar"), "{t}");
    for gone in ["borrar", "guardar", "crear", "agregar", "reinici", "pausar / seguir el timer"] {
        assert!(!t.contains(gone), "Opciones no modifica nada ni lista atajos ocultos: «{gone}»\n{t}");
    }
}

#[test]
fn options_rows_change_label_with_the_state() {
    let mut app = focus_app();
    app.state.paused = true;
    app.state.active = vec!["noise-brown".into()];
    ks(&mut app, "tab");
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("Seguir todo") && !t.contains("Pausar todo"), "{t}");
    ks(&mut app, "esc");
    app.deep = app::Deep::On;
    ks(&mut app, "tab");
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("Salir de deep work") && !t.contains("Modo deep work"), "{t}");
}

#[test]
fn options_timer_opens_the_timer_list_without_digits() {
    let mut app = focus_app();
    ks(&mut app, "tab enter");
    assert!(matches!(app.popup, Some(app::Popup::Timer { sel: 0, custom: None })));
    let t = screen_text(&app, 80, 24);
    for s in ["25 min", "50 min", "90 min", "personalizado…"] {
        assert!(t.contains(s), "falta «{s}»:\n{t}");
    }
    assert!(!t.contains("1  25") && !t.contains("2  50") && !t.contains("3  90"), "{t}");
    ks(&mut app, "down enter");
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Timer { minutes: Some(50) })));
    assert!(app.popup.is_none());
}

#[test]
fn esc_in_the_timer_goes_back_to_options_with_timer_selected() {
    let mut app = focus_app();
    ks(&mut app, "tab enter");
    assert!(matches!(app.popup, Some(app::Popup::Timer { custom: None, .. })));
    ks(&mut app, "esc");
    assert!(matches!(app.popup, Some(app::Popup::Options { sel: 0 })));
    assert!(screen_text(&app, 80, 24).contains("> Timer ›"));
    assert!(sent(&mut app).is_empty());
    // Un segundo Esc cierra Opciones.
    ks(&mut app, "esc");
    assert!(app.popup.is_none());
    // Elegir un tiempo sigue cerrando todo.
    ks(&mut app, "tab enter down enter");
    assert!(app.popup.is_none());
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Timer { minutes: Some(50) })));
}

#[test]
fn esc_in_the_custom_minutes_goes_back_to_the_list_and_then_to_options() {
    let mut app = focus_app();
    ks(&mut app, "tab enter down*3 enter"); // «personalizado…»
    assert!(matches!(app.popup, Some(app::Popup::Timer { custom: Some(_), .. })));
    press(&mut app, "12");
    ks(&mut app, "esc");
    assert!(matches!(app.popup, Some(app::Popup::Timer { custom: None, .. })));
    ks(&mut app, "esc");
    assert!(matches!(app.popup, Some(app::Popup::Options { sel: 0 })));
    assert!(sent(&mut app).is_empty());
}

#[test]
fn digits_one_to_three_no_longer_pick_a_timer() {
    let mut app = focus_app();
    ks(&mut app, "tab enter");
    press(&mut app, "123");
    assert!(sent(&mut app).is_empty());
    assert!(matches!(app.popup, Some(app::Popup::Timer { sel: 0, custom: None })));
    press(&mut app, "jk");
    assert!(matches!(app.popup, Some(app::Popup::Timer { sel: 0, .. })));
}

#[test]
fn the_timer_list_offers_cancel_when_running_and_custom_types_minutes() {
    let mut app = focus_app();
    with_timer(&mut app, 600);
    ks(&mut app, "tab enter down*4 enter");
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Timer { minutes: None })));
    ks(&mut app, "tab enter down*3 enter");
    press(&mut app, "4 5");
    ks(&mut app, "enter");
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Timer { minutes: Some(45) })));
}

#[test]
fn options_deep_work_toggles_the_mode_and_closes() {
    let mut app = focus_app();
    ks(&mut app, "tab down enter");
    assert_eq!(app.deep, app::Deep::On);
    assert!(app.popup.is_none());
    assert!(app.outbox.is_empty(), "el deep work no manda nada al daemon");
    ks(&mut app, "tab down enter");
    assert_eq!(app.deep, app::Deep::Off);
}

#[test]
fn options_load_preset_lists_presets_and_loads_the_chosen_one() {
    let mut app = focus_app();
    ks(&mut app, "tab down*2 enter");
    assert!(matches!(app.popup, Some(app::Popup::LoadPreset { sel: 0 })));
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("cargar preset") && t.contains("> Templo Jedi") && t.contains("Lluvia y fuego"), "{t}");
    assert!(!t.contains("borrar") && !t.contains("guardar") && !t.contains("nuevo"), "solo se elige:\n{t}");
    ks(&mut app, "down enter");
    assert!(app.popup.is_none());
    let out = sent(&mut app);
    assert!(matches!(out.as_slice(), [Command::LoadPreset { name }] if name == "Lluvia y fuego"), "{out:?}");
    // Esc no carga nada.
    ks(&mut app, "tab down*2 enter esc");
    assert!(app.popup.is_none() && sent(&mut app).is_empty());
}

#[test]
fn load_preset_without_presets_says_so_and_esc_closes() {
    let mut app = focus_app();
    let mut st = app.state.clone();
    st.presets.clear();
    app.set_state(st);
    ks(&mut app, "tab down*2 enter");
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("Todavía no hay presets"), "{t}");
    ks(&mut app, "enter");
    assert!(app.popup.is_none() && sent(&mut app).is_empty());
}

#[test]
fn options_volume_changes_with_left_right_without_closing() {
    let mut app = focus_app();
    app.state.master = 0.5;
    ks(&mut app, "tab down*3");
    ks(&mut app, "right");
    assert!(matches!(app.popup, Some(app::Popup::Options { sel: 3 })), "no se cierra");
    assert!((app.state.master - 0.55).abs() < 1e-4);
    assert!(matches!(sent(&mut app).pop(), Some(Command::SetMaster { volume }) if (volume - 0.55).abs() < 1e-4));
    ks(&mut app, "left*3");
    assert!((app.state.master - 0.4).abs() < 1e-4);
    assert!(app.popup.is_some());
    assert!(screen_text(&app, 80, 24).contains("40 %"));
    ks(&mut app, "left*30");
    assert_eq!(app.state.master, 0.0);
    ks(&mut app, "right*30");
    assert_eq!(app.state.master, 1.0);
    sent(&mut app);
    ks(&mut app, "enter");
    assert!(app.popup.is_some() && sent(&mut app).is_empty(), "Enter sobre el volumen no hace nada");
}

#[test]
fn left_right_on_other_options_rows_do_nothing() {
    let mut app = focus_app();
    app.state.master = 0.5;
    ks(&mut app, "tab left right down right");
    assert!(sent(&mut app).is_empty() && app.state.master == 0.5);
    assert!(matches!(app.popup, Some(app::Popup::Options { sel: 1 })));
}

#[test]
fn options_pause_stop_home_and_quit_rows_work() {
    let mut app = focus_app();
    ks(&mut app, "tab down*4 enter");
    assert!(matches!(sent(&mut app).as_slice(), [Command::PauseAll]));
    assert!(app.popup.is_none());
    ks(&mut app, "tab down*5 enter");
    assert!(matches!(sent(&mut app).as_slice(), [Command::StopAll]));
    ks(&mut app, "tab down*6 enter");
    assert_eq!(app.screen, app::Screen::Home);
    assert!(app.popup.is_none());

    // Salir en silencio: la ventanita de siempre, con «No» marcada.
    let mut app = focus_app();
    ks(&mut app, "tab down*7 enter");
    assert!(matches!(app.popup, Some(app::Popup::ConfirmQuit { sel: 1 })));
    assert!(app.exit.is_none());
    ks(&mut app, "up enter");
    assert_eq!(app.exit, Some(app::Exit::Quit));

    // Con algo sonando: seguir de fondo / apagar todo y salir / volver.
    let mut app = focus_app();
    app.state.active = vec!["noise-brown".into()];
    ks(&mut app, "tab down*7 enter");
    assert!(matches!(app.popup, Some(app::Popup::Quit { sel: 2 })));
    ks(&mut app, "up*2 enter");
    assert_eq!(app.exit, Some(app::Exit::Detach));
}

#[test]
fn space_works_as_enter_in_options() {
    let mut app = focus_app();
    ks(&mut app, "tab");
    press(&mut app, " ");
    assert!(matches!(app.popup, Some(app::Popup::Timer { .. })));
}

#[test]
fn old_focus_keys_do_nothing() {
    let mut app = focus_app();
    app.state.active = vec!["noise-brown".into()];
    app.state.master = 0.5;
    for c in "qpx+-=dhjklraAsgnemy1234".chars() {
        press(&mut app, &c.to_string());
    }
    ks(&mut app, "home end del backtab");
    ctrl(&mut app, "qgjk");
    assert!(app.popup.is_none() && app.exit.is_none() && app.help.is_none());
    assert!(sent(&mut app).is_empty());
    assert_eq!(app.state.master, 0.5);
    assert_eq!(app.cursor, 0);
    assert_eq!(app.deep, app::Deep::Off);
    assert_eq!(app.screen, app::Screen::Focus);
}

#[test]
fn focus_keeps_up_down_left_right_and_enter_on_sounds() {
    let mut app = focus_app();
    ks(&mut app, "down*2");
    assert_eq!(app.cursor, 2);
    ks(&mut app, "up");
    assert_eq!(app.cursor, 1);
    let id = app.selected_sound().unwrap().id.clone();
    ks(&mut app, "enter");
    assert!(matches!(sent(&mut app).pop(), Some(Command::Toggle { id: i }) if i == id));
    ks(&mut app, "right");
    assert!(matches!(sent(&mut app).pop(), Some(Command::SetVolume { id: i, volume }) if i == id && volume > 0.5));
    ks(&mut app, "left");
    assert!(matches!(sent(&mut app).pop(), Some(Command::SetVolume { .. })));
}

// ---------------------------------------------------- atajos ocultos: 0 y t

#[test]
fn zero_restarts_sounds_only_on_the_focus_main_view() {
    let mut app = focus_app();
    press(&mut app, "0");
    assert!(matches!(sent(&mut app).as_slice(), [Command::RestartSounds]));
    // Sigue andando en deep work.
    deep_on(&mut app);
    press(&mut app, "0");
    assert!(matches!(sent(&mut app).as_slice(), [Command::RestartSounds]));
    // En el menú de inicio no es de Focus.
    app.go(app::Screen::Home);
    app.outbox.clear();
    press(&mut app, "0");
    assert!(app.outbox.iter().all(|c| !matches!(c, Command::RestartSounds)));
}

#[test]
fn zero_inside_options_and_the_custom_minutes_does_not_restart() {
    let mut app = focus_app();
    ks(&mut app, "tab");
    press(&mut app, "0");
    assert!(sent(&mut app).is_empty());
    let mut app = focus_app();
    app.popup = Some(app::Popup::Timer { sel: 0, custom: Some(String::new()) });
    press(&mut app, "50");
    assert!(matches!(&app.popup, Some(app::Popup::Timer { custom: Some(s), .. }) if s == "50"));
    assert!(app.outbox.iter().all(|c| !matches!(c, Command::RestartSounds)));
}

#[test]
fn t_toggles_the_timer_pause_and_leaves_the_sound_alone() {
    let mut app = focus_app();
    with_timer(&mut app, 1200);
    press(&mut app, "t");
    assert_eq!(pause_cmds(&mut app), 1);
    press(&mut app, "T");
    assert_eq!(pause_cmds(&mut app), 1);
    deep_on(&mut app);
    press(&mut app, "t");
    assert_eq!(pause_cmds(&mut app), 1, "también en deep work");
    assert!(app.outbox.iter().all(|c| !matches!(c, Command::PauseAll | Command::StopAll)));
    assert!(app.popup.is_none(), "t ya no abre el timer");
}

#[test]
fn t_without_a_timer_only_shows_a_notice() {
    let mut app = focus_app();
    press(&mut app, "t");
    assert_eq!(pause_cmds(&mut app), 0);
    assert_eq!(app.notice.as_ref().map(|(n, _)| n.as_str()), Some("no hay timer"));
}

#[test]
fn r_does_nothing_in_focus_any_more() {
    let mut app = focus_app();
    with_timer(&mut app, 1200);
    press(&mut app, "rR");
    assert!(sent(&mut app).is_empty() && app.notice.is_none());
}

#[test]
fn t_types_normally_in_text_fields_and_does_nothing_in_popups() {
    let mut app = focus_app();
    with_timer(&mut app, 1200);
    ks(&mut app, "tab enter");
    press(&mut app, "t"); // en la lista de tiempos no hace nada
    assert_eq!(pause_cmds(&mut app), 0);
    app.popup = Some(app::Popup::Timer { sel: 0, custom: Some(String::new()) });
    press(&mut app, "tT");
    assert_eq!(pause_cmds(&mut app), 0);
    let mut app = app_with_routines();
    config(&mut app, 0);
    ks(&mut app, "enter");
    press(&mut app, "tT0");
    assert_eq!(pause_cmds(&mut app), 0);
    assert!(sent(&mut app).is_empty());
}

#[test]
fn hidden_shortcuts_appear_nowhere() {
    let mut app = focus_app();
    with_timer(&mut app, 1200);
    for (w, h) in [(80, 24), (120, 34)] {
        let t = screen_text(&app, w, h);
        for gone in ["reinicia", "restart", "pausar / seguir el timer"] {
            assert!(!t.contains(gone), "«{gone}» no va en el pie:\n{t}");
        }
        let footer = t.lines().last().unwrap();
        assert!(footer.split_whitespace().all(|w| w != "0" && w != "t"), "{t}");
    }
    ks(&mut app, "?");
    let help = screen_text(&app, 80, 24);
    for gone in ["reinicia", "pausar / seguir el timer"] {
        assert!(!help.contains(gone), "«{gone}» no va en la ayuda:\n{help}");
    }
    assert!(help.lines().all(|l| !l.split_whitespace().any(|w| w == "0" || w == "t")), "{help}");
    ks(&mut app, "esc tab");
    let opt = screen_text(&app, 80, 24);
    for gone in ["reinicia", "pausar / seguir el timer"] {
        assert!(!opt.contains(gone), "«{gone}» no va en Opciones:\n{opt}");
    }
}

// ---------------------------------------------------------- modo deep work

#[test]
fn d_no_longer_toggles_deep_work() {
    let mut app = focus_app();
    press(&mut app, "dD");
    assert_eq!(app.deep, app::Deep::Off);
    assert!(app.outbox.is_empty());
}

#[test]
fn tab_does_nothing_in_the_intro_popup() {
    let mut app = app_with_presets();
    home(&mut app, 2);
    ks(&mut app, "tab");
    assert_eq!(app.deep, app::Deep::Off);
    assert!(intro(&app).is_some());
}

#[test]
fn deep_work_keeps_the_sound_keys_and_options() {
    let mut app = focus_app();
    deep_on(&mut app);
    ks(&mut app, "down enter right");
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Toggle { .. })));
    ks(&mut app, "tab down*4 enter");
    assert!(matches!(sent(&mut app).as_slice(), [Command::PauseAll]));
    ks(&mut app, "tab down*7 enter");
    assert!(matches!(app.popup, Some(app::Popup::ConfirmQuit { .. })));
    ks(&mut app, "esc esc");
    assert_eq!(app.screen, app::Screen::Home);
    assert_eq!(app.deep, app::Deep::Off, "al salir de Focus se sale del modo");
}

#[test]
fn reaching_zero_in_deep_work_ends_the_session_and_enter_goes_back() {
    let mut app = focus_app();
    with_timer(&mut app, 5);
    deep_on(&mut app);
    with_timer(&mut app, 1);
    without_timer(&mut app);
    assert_eq!(app.deep, app::Deep::Done);
    sent(&mut app);
    ks(&mut app, "enter");
    assert_eq!(app.deep, app::Deep::Off);
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(sent(&mut app).is_empty(), "Enter solo vuelve: no enciende ningún sonido");
}

#[test]
fn space_also_goes_back_from_the_final_screen_and_d_does_not() {
    let mut app = focus_app();
    app.deep = app::Deep::Done;
    press(&mut app, "d");
    assert_eq!(app.deep, app::Deep::Done);
    press(&mut app, " ");
    assert_eq!(app.deep, app::Deep::Off);
}

#[test]
fn reaching_zero_outside_deep_work_changes_nothing() {
    let mut app = focus_app();
    with_timer(&mut app, 1);
    without_timer(&mut app);
    assert_eq!(app.deep, app::Deep::Off);
}

#[test]
fn cancelling_the_timer_in_deep_work_does_not_end_the_session() {
    let mut app = focus_app();
    with_timer(&mut app, 1200);
    deep_on(&mut app);
    without_timer(&mut app);
    assert_eq!(app.deep, app::Deep::On);
}

#[test]
fn deep_work_paused_label_leaves_a_blank_row_under_the_digits() {
    let mut app = focus_app();
    paused_timer(&mut app, 1200);
    deep_on(&mut app);
    for (w, h) in [(80, 24), (120, 40), (40, 14)] {
        let t = screen_text(&app, w, h);
        let lines: Vec<&str> = t.lines().collect();
        let i = lines.iter().position(|l| l.contains("en pausa")).unwrap_or_else(|| panic!("{w}x{h}: {t}"));
        assert!(i > 0, "{t}");
        let start = lines[i].find("⏸").unwrap();
        let above: String = lines[i - 1].chars().skip(lines[i][..start].chars().count()).take(11).collect();
        assert!(above.trim().is_empty(), "{w}x{h}: la fila de arriba no está vacía: {above:?}\n{t}");
    }
}

#[test]
fn a_paused_timer_is_shown_as_paused_in_focus_and_deep_work() {
    let mut app = focus_app();
    with_timer(&mut app, 1200);
    assert!(!screen_text(&app, 80, 24).contains("en pausa"));
    paused_timer(&mut app, 1200);
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("⏸  timer en pausa") && !t.contains("r para seguir"), "{t}");
    deep_on(&mut app);
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("⏸  en pausa"), "{t}");
    let t = screen_text(&app, 30, 10);
    assert!(t.contains("en pausa"), "{t}");
}

#[test]
fn cancelling_a_paused_timer_near_zero_does_not_end_deep_work() {
    let mut app = focus_app();
    paused_timer(&mut app, 1);
    deep_on(&mut app);
    without_timer(&mut app);
    assert_eq!(app.deep, app::Deep::On);
}

#[test]
fn a_new_timer_on_the_final_screen_resumes_the_session() {
    let mut app = focus_app();
    app.deep = app::Deep::Done;
    with_timer(&mut app, 600);
    assert_eq!(app.deep, app::Deep::On);
}

#[test]
fn deep_work_shows_only_the_big_clock_and_the_help_hint() {
    let mut app = focus_app();
    with_timer(&mut app, 1873);
    deep_on(&mut app);
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("? ayuda"), "{t}");
    for s in ["sonido", "volumen", "timer", "presets", "salir", "vol ", "opciones"] {
        assert!(!t.contains(s), "no debería verse «{s}» en:\n{t}");
    }
    // 31:13 con los dígitos del reloj normal, agrandados: la primera fila del 3 es «██████» (ancho 6 o más).
    assert!(t.contains("██████"), "{t}");
    assert!(!t.contains("31:13"));
}

#[test]
fn deep_work_without_a_timer_shows_the_big_focus_word() {
    let mut app = focus_app();
    deep_on(&mut app);
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("██████"), "{t}");
    assert!(t.contains("? ayuda"));
}

#[test]
fn deep_work_clock_never_grows_past_twice_the_normal_one() {
    let mut app = focus_app();
    deep_on(&mut app);
    let rows = |w, h| {
        screen_text(&app, w, h).lines().filter(|l| l.contains('█') || l.contains('▀') || l.contains('▄')).count()
    };
    // Escala 2 de una fuente de 3 filas: como mucho 6 filas, aunque la pantalla sea enorme.
    assert!(rows(80, 24) <= 6);
    assert_eq!(rows(300, 100), rows(80, 24));
}

#[test]
fn the_final_screen_says_well_done_and_how_to_go_back() {
    let mut app = focus_app();
    app.deep = app::Deep::Done;
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("¡Buen trabajo!"), "{t}");
    assert!(t.contains("Enter para volver") && !t.contains("presioná d"), "{t}");
}

// ---------------------------------------------------- Esc y Configuración

#[test]
fn esc_walks_back_one_step_at_a_time() {
    let mut app = App::new();
    ks(&mut app, "down*3 enter");
    assert_eq!(app.screen, app::Screen::Config);
    ks(&mut app, "down enter");
    assert_eq!(app.screen, app::Screen::Meditations);
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Config);
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Home);
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Home, "Esc en el inicio no sale");
    assert!(app.exit.is_none() && app.popup.is_none());
}

#[test]
fn config_lists_its_entries() {
    let mut app = App::new();
    home(&mut app, 3);
    let t = screen_text(&app, 80, 20);
    for s in ["Rutinas", "Meditaciones", "Sonidos ambientales", "Presets", "Sincronización", "↑↓ mover   Enter elegir"]
    {
        assert!(t.contains(s), "falta «{s}»");
    }
    ks(&mut app, "down*3 enter");
    assert_eq!(app.screen, app::Screen::Presets);
}

// ------------------------------------------------------------- Meditaciones

#[test]
fn meditations_screen_groups_by_duration_with_the_plus_row_last() {
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    let t = screen_text(&app, 80, 24);
    let pos = |s: &str| t.find(s).unwrap_or_else(|| panic!("falta «{s}» en:\n{t}"));
    assert!(pos("5 MINUTOS") < pos("Respiración 4-7-8"));
    assert!(pos("Respiración 4-7-8") < pos("10 MINUTOS"));
    assert!(pos("10 MINUTOS") < pos("Calma guiada"));
    assert!(!t.contains("20 MINUTOS"), "las categorías vacías no se muestran");
    assert!(pos("Calma guiada") < pos("+ Agregar desde YouTube"), "la fila «+» va al final");
    assert!(t.contains(">   Respiración 4-7-8"), "el cursor arranca en la primera meditación:\n{t}");
}

#[test]
fn an_empty_meditations_list_starts_on_the_plus_row_and_the_cursor_stays_valid() {
    let mut app = App::new();
    app.screen = app::Screen::Meditations;
    assert_eq!(app.med_cursor, 0);
    assert!(screen_text(&app, 80, 24).contains("> + Agregar desde YouTube"));
    ks(&mut app, "down*3");
    assert_eq!(app.med_cursor, 0);
    ks(&mut app, "enter");
    assert!(matches!(app.med_popup, Some(app::MedPopup::Add { .. })));
}

#[test]
fn the_cursor_stays_valid_when_the_last_item_of_each_list_disappears() {
    let mut app = app_with_meditations();
    let n = app.state.meditations.len();
    app.med_cursor = n; // sobre «+»
    let mut st = app.state.clone();
    st.meditations.pop();
    app.set_state(st);
    assert_eq!(app.med_cursor, n - 1, "ahora «+» es la fila n-1");
    app.med_cursor = n - 2; // sobre la última meditación
    let mut st = app.state.clone();
    st.meditations.pop();
    app.set_state(st);
    assert!(app.med_cursor <= app.state.meditations.len());

    let mut app = app_with_sounds_and_presets();
    app.preset_list_cursor = app.state.presets.len() - 1;
    let mut st = app.state.clone();
    st.presets.pop();
    app.set_state(st);
    assert_eq!(app.preset_list_cursor, app.state.presets.len(), "cae sobre la fila «+»");
}

#[test]
fn enter_on_a_meditation_opens_its_menu_with_listen_preselected() {
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    ks(&mut app, "enter");
    assert!(matches!(app.med_popup, Some(app::MedPopup::Menu { sel: 0, .. })));
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("Respiración 4-7-8") && t.contains("> Escuchar") && t.contains("Borrar"), "{t}");
    assert!(app.outbox.is_empty());
    ks(&mut app, "enter");
    assert!(app.med_popup.is_none());
    assert!(matches!(app.outbox.pop(), Some(Command::PlayMeditation { id }) if id == "resp"));
    app.state.meditation = Some(crate::protocol::MeditationPlayback {
        id: "resp".into(),
        position_secs: 192,
        total_secs: 300,
        paused: false,
    });
    assert!(screen_text(&app, 80, 24).contains("▶ 03:12 / 05:00"));
    ks(&mut app, "enter");
    assert!(screen_text(&app, 80, 24).contains("> Pausar"));
    ks(&mut app, "enter");
    assert!(matches!(app.outbox.pop(), Some(Command::ToggleMeditation)));
    app.state.meditation.as_mut().unwrap().paused = true;
    assert!(screen_text(&app, 80, 24).contains("⏸ 03:12 / 05:00"));
    ks(&mut app, "enter");
    assert!(screen_text(&app, 80, 24).contains("> Escuchar"), "en pausa vuelve a «Escuchar»");
    ks(&mut app, "esc");
    assert!(app.med_popup.is_none());
}

#[test]
fn the_plus_row_asks_for_the_link_then_the_category_with_up_down_only() {
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    ks(&mut app, "down*40 enter");
    ks(&mut app, "enter"); // sin link no avanza
    assert!(matches!(app.med_popup, Some(app::MedPopup::Add { category: None, .. })));
    app.paste("https://youtu.be/abc");
    ks(&mut app, "enter");
    assert!(screen_text(&app, 80, 24).contains("¿De cuántos minutos es?"));
    ks(&mut app, "right right"); // ← → ya no hacen falta
    assert!(matches!(app.med_popup, Some(app::MedPopup::Add { category: Some(0), .. })));
    ks(&mut app, "down*2 enter"); // 5 -> 10 -> 15
    assert!(app.med_popup.is_none());
    match app.outbox.pop() {
        Some(Command::AddMeditation(r)) => assert_eq!((r.url.as_str(), r.minutes), ("https://youtu.be/abc", 15)),
        other => panic!("salió {other:?}"),
    }
}

#[test]
fn deleting_a_meditation_goes_through_its_menu_and_a_confirmation_with_no_preselected() {
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    ks(&mut app, "enter down enter");
    assert!(matches!(app.med_popup, Some(app::MedPopup::Delete { sel: 1, .. })));
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("¿Borrar «Respiración 4-7-8»?") && t.contains("Sí, borrar") && t.contains("> No"), "{t}");
    ks(&mut app, "esc");
    assert!(app.med_popup.is_none() && app.outbox.is_empty(), "Esc = no");
    ks(&mut app, "enter down enter enter");
    assert!(app.med_popup.is_none() && app.outbox.is_empty(), "Enter sobre «No» no borra");
    ks(&mut app, "enter down enter up enter");
    assert!(matches!(app.outbox.pop(), Some(Command::DeleteMeditation { id }) if id == "resp"));
}

#[test]
fn menu_screens_are_still_so_the_tui_idles() {
    let mut app = App::new();
    app.state.active = vec!["noise-brown".into()];
    assert!(!app.animating(), "con sonido en background, el menú no anima");
}

// ------------------------------------------------------------ rutinas

/// Abre el editor de la rutina `i` desde Configuración > Rutinas (Enter sobre «Editar»).
fn edit_routine_n(app: &mut App, i: usize) {
    config(app, 0);
    ks(app, &format!("down*{i} enter enter"));
    assert_eq!(app.screen, app::Screen::RoutineEditor);
}

fn edit_first(app: &mut App) {
    edit_routine_n(app, 0);
}

fn editor(app: &App) -> &routines::Editor {
    app.editor.as_ref().unwrap()
}

/// Baja hasta el final del editor («✓ Guardar») y lo elige.
fn ed_save(app: &mut App) {
    ks(app, "down*30 enter");
}

/// Fila de la meditación en el editor de la rutina de ejemplo (nombre, 3 tareas, «+», meditación).
const MED_ROW: usize = 5;

#[test]
fn home_continues_the_last_routine_or_offers_to_create_the_first() {
    let mut app = App::new();
    assert!(screen_text(&app, 80, 20).contains("Crear mi primera rutina"));
    home(&mut app, 0);
    assert_eq!(app.screen, app::Screen::RoutineEditor, "sin rutinas, la primera opción crea");
    let mut app = app_with_routines();
    assert!(screen_text(&app, 80, 20).contains("Continuar \"Mañana de estudio\""));
    app.last_routine = Some("Tarde".into());
    assert_eq!(app.home_items()[0], "Continuar \"Tarde\"");
    home(&mut app, 0);
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
fn list_has_a_plus_row_last_summaries_and_esc_goes_back_to_where_it_came_from() {
    let mut app = app_with_routines();
    home(&mut app, 1);
    assert_eq!(app.screen, app::Screen::Routines);
    assert_eq!(app.list_cursor, 0);
    let t = screen_text(&app, 80, 20);
    for s in ["> Mañana de estudio", "+ Nueva rutina", "4 pasos · 1 meditación", "Enter elegir"] {
        assert!(t.contains(s), "falta «{s}» en:\n{t}");
    }
    assert!(t.find("Mañana de estudio").unwrap() < t.find("+ Nueva rutina").unwrap());
    for gone in ["nueva  ", "editar  ", "borrar  "] {
        assert!(!t.contains(gone), "{t}");
    }
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Home);
    config(&mut app, 0);
    assert_eq!(app.screen, app::Screen::Routines);
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Config);
}

#[test]
fn the_plus_row_creates_a_routine() {
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "down*40");
    assert_eq!(app.list_cursor, 2);
    assert!(screen_text(&app, 80, 20).contains("> + Nueva rutina"));
    ks(&mut app, "enter");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(editor(&app).index.is_none());
}

#[test]
fn with_no_routines_the_list_starts_on_the_plus_row() {
    let mut app = App::new();
    home(&mut app, 1);
    assert_eq!(app.screen, app::Screen::Routines);
    assert!(screen_text(&app, 80, 20).contains("> + Nueva rutina"));
    ks(&mut app, "down*3");
    assert_eq!(app.list_cursor, 0);
    ks(&mut app, "enter");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
}

#[test]
fn enter_on_a_routine_from_home_opens_empezar_editar_borrar_with_empezar_preselected() {
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "enter");
    assert!(matches!(app.list_popup, Some(routines::ListPopup::Menu { index: 0, sel: 0 })));
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("> Empezar") && t.contains("Editar") && t.contains("Borrar"), "{t}");
    ks(&mut app, "esc");
    assert!(app.list_popup.is_none() && app.screen == app::Screen::Routines);
    ks(&mut app, "enter enter");
    assert_eq!(app.screen, app::Screen::RoutineRun);
    assert_eq!(app.run.as_ref().unwrap().routine.name, "Mañana de estudio");
    // Editar es la segunda fila.
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "down enter down enter");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert_eq!(editor(&app).routine.name, "Tarde");
}

#[test]
fn enter_on_a_routine_from_config_opens_editar_borrar_with_editar_preselected() {
    let mut app = app_with_routines();
    config(&mut app, 0);
    ks(&mut app, "enter");
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("> Editar") && t.contains("Borrar") && !t.contains("Empezar"), "{t}");
    ks(&mut app, "enter");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
}

#[test]
fn deleting_a_routine_goes_through_the_menu_and_asks_first_with_no_preselected() {
    let mut app = app_with_routines();
    config(&mut app, 0);
    ks(&mut app, "enter down enter");
    assert!(matches!(app.list_popup, Some(routines::ListPopup::Delete { sel: 1, .. })));
    let t = screen_text(&app, 80, 24);
    assert!(
        t.contains("¿Borrar la rutina «Mañana de estudio»?") && t.contains("Sí, borrar") && t.contains("> No"),
        "{t}"
    );
    ks(&mut app, "esc");
    assert_eq!(app.routines.len(), 2);
    assert!(!app.routines_dirty);
    ks(&mut app, "enter down enter enter");
    assert_eq!(app.routines.len(), 2, "Enter sobre «No» no borra");
    ks(&mut app, "enter down enter up enter");
    assert_eq!(app.routines.len(), 1);
    assert_eq!(app.routines[0].name, "Tarde");
    assert!(app.routines_dirty);
}

#[test]
fn from_home_the_delete_row_is_the_third_one() {
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "enter down*2 enter");
    assert!(matches!(app.list_popup, Some(routines::ListPopup::Delete { .. })));
}

#[test]
fn a_broken_routines_file_is_never_overwritten() {
    let mut app = app_with_routines();
    app.routines_locked = Some("toml roto".into());
    home(&mut app, 1);
    ks(&mut app, "enter down*2 enter up enter");
    assert_eq!(app.routines.len(), 2);
    assert!(!app.routines_dirty);
    ks(&mut app, "down*40 enter");
    assert_eq!(app.screen, app::Screen::Routines, "tampoco crea");
    ks(&mut app, "up*40 enter down enter");
    assert_eq!(app.screen, app::Screen::Routines, "ni edita");
}

#[test]
fn new_routine_asks_the_name_then_collects_tasks_and_saves_from_the_save_row() {
    let mut app = app_with_meditations();
    home(&mut app, 1);
    ks(&mut app, "enter");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(screen_text(&app, 80, 24).contains("Nombre de la rutina"));
    app.key(key(Enter)); // vacío: no avanza
    assert!(editor(&app).popup.is_some());
    type_text(&mut app, "Tarde");
    app.key(key(Enter));
    assert!(editor(&app).popup.is_none());
    ed_save(&mut app);
    assert_eq!(app.screen, app::Screen::RoutineEditor, "sin pasos no se guarda");
    assert!(app.notice.is_some());
    ks(&mut app, "up*30 down*1"); // «+ Agregar tarea» (no hay tareas todavía)
    assert_eq!(editor(&app).row(), routines::EdRow::Add);
    ks(&mut app, "enter");
    assert!(screen_text(&app, 80, 24).contains("¿Qué hay que hacer?"));
    type_text(&mut app, "Preparar mate");
    app.key(key(Enter));
    assert_eq!(editor(&app).row(), routines::EdRow::Task(0), "el cursor queda en la tarea nueva");
    assert!(screen_text(&app, 80, 24).contains("Meditación · no"));
    ks(&mut app, "down*2"); // «+ Agregar tarea», después la meditación
    ks(&mut app, "enter");
    assert_eq!(app.screen, app::Screen::RoutineStep);
    ks(&mut app, "right down*2 right enter"); // sí; 10 -> 15
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    ed_save(&mut app);
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
    home(&mut app, 1);
    ks(&mut app, "enter");
    type_text(&mut app, "Sola");
    ks(&mut app, "enter");
    ed_save(&mut app);
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    ks(&mut app, "up*30 down*2 enter right enter"); // la meditación, sí
    ed_save(&mut app);
    assert_eq!(app.screen, app::Screen::Routines);
    assert_eq!(app.routines[0].meditation, setting(Some(10)));
    assert!(app.routines[0].tasks.is_empty());
}

#[test]
fn cancelling_the_name_of_a_new_routine_leaves_the_editor() {
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "enter esc");
    assert_eq!(app.screen, app::Screen::Routines);
    assert!(app.editor.is_none());
}

#[test]
fn names_cannot_repeat_and_the_name_row_edits_it() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    assert_eq!(editor(&app).row(), routines::EdRow::Name);
    ks(&mut app, "enter");
    ks(&mut app, "bs*30");
    type_text(&mut app, "tarde");
    ks(&mut app, "enter");
    assert!(app.notice.as_ref().unwrap().0.contains("ya hay otra rutina"));
    assert!(editor(&app).popup.is_some());
    ks(&mut app, "bs*30");
    type_text(&mut app, "Nueva");
    ks(&mut app, "enter");
    assert!(editor(&app).popup.is_none());
    assert_eq!(editor(&app).routine.name, "Nueva");
    assert!(screen_text(&app, 80, 24).contains("Nombre: Nueva"));
}

#[test]
fn editor_lists_name_tasks_plus_row_the_meditation_apart_and_the_save_row() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    let t = screen_text(&app, 80, 30);
    assert!(t.contains("Nombre: Mañana de estudio") && t.contains("3  Abrir el material"), "{t}");
    assert!(t.contains("+ Agregar tarea") && t.contains("Meditación · 10 min fija") && t.contains("✓ Guardar"), "{t}");
    let pos = |s: &str| t.find(s).unwrap();
    assert!(
        pos("Nombre:") < pos("1  Ordenar")
            && pos("3  Abrir") < pos("+ Agregar tarea")
            && pos("+ Agregar tarea") < pos("Meditación ·")
            && pos("Meditación ·") < pos("✓ Guardar"),
        "{t}"
    );
    assert!(!t.contains("4  "), "la meditación no se numera como paso:\n{t}");
    let footer = t.lines().last().unwrap();
    for gone in ["tarea", "nombre", "guardar", "K/J", "borrar"] {
        assert!(!footer.contains(gone), "{t}");
    }
}

#[test]
fn enter_on_a_task_opens_editar_texto_mover_borrar_with_editar_texto_preselected() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*2 enter");
    assert!(matches!(editor(&app).popup, Some(routines::EdPopup::TaskMenu { at: 1, sel: 0 })));
    let t = screen_text(&app, 80, 30);
    assert!(t.contains("> Editar texto") && t.contains("Mover") && t.contains("Borrar"), "{t}");
    ks(&mut app, "enter");
    assert!(
        matches!(&editor(&app).popup, Some(routines::EdPopup::Task { text, at: Some(1) }) if text == "Preparar mate")
    );
    type_text(&mut app, "!");
    ks(&mut app, "enter");
    assert_eq!(editor(&app).routine.tasks[1], "Preparar mate!");
    assert!(editor(&app).dirty());
}

#[test]
fn mover_keeps_the_task_highlighted_and_arrows_carry_it_until_enter_or_esc_drops_it() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*2 enter down enter"); // tarea 2 › Mover
    assert!(matches!(editor(&app).popup, Some(routines::EdPopup::Move)));
    let t = screen_text(&app, 80, 30);
    assert!(t.contains("↕ Preparar mate") && t.contains("Enter soltar"), "{t}");
    ks(&mut app, "up");
    assert_eq!(editor(&app).routine.tasks[0], "Preparar mate");
    assert_eq!(editor(&app).cursor, 1, "el cursor acompaña a la tarea");
    ks(&mut app, "up"); // ya es la primera
    assert_eq!(editor(&app).routine.tasks[0], "Preparar mate");
    ks(&mut app, "down*5");
    assert_eq!(editor(&app).routine.tasks[2], "Preparar mate");
    assert_eq!(editor(&app).cursor, 3, "no sale de la lista de tareas");
    assert!(matches!(editor(&app).popup, Some(routines::EdPopup::Move)), "sigue llevándola");
    ks(&mut app, "enter");
    assert!(editor(&app).popup.is_none());
    ks(&mut app, "up");
    assert_eq!(editor(&app).cursor, 2, "ya suelta, ↑↓ solo mueven el cursor");
    assert_eq!(editor(&app).routine.tasks[2], "Preparar mate");
    assert!(!app.routines_dirty, "hasta guardar no se toca la lista");
    // Esc también la suelta, sin cerrar el editor.
    ks(&mut app, "enter down enter up esc");
    assert!(editor(&app).popup.is_none() && app.screen == app::Screen::RoutineEditor);
    assert_eq!(editor(&app).routine.tasks[0], "Abrir el material");
}

#[test]
fn borrar_a_task_asks_with_no_preselected() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*2 enter down*2 enter");
    assert!(matches!(editor(&app).popup, Some(routines::EdPopup::DeleteStep { at: 1, sel: 1 })));
    let t = screen_text(&app, 80, 30);
    assert!(t.contains("¿Borrar la tarea «Preparar mate»?") && t.contains("Sí, borrar") && t.contains("> No"), "{t}");
    ks(&mut app, "esc");
    assert_eq!(editor(&app).routine.tasks.len(), 3);
    ks(&mut app, "enter down*2 enter enter");
    assert_eq!(editor(&app).routine.tasks.len(), 3, "Enter sobre «No» no borra");
    ks(&mut app, "enter down*2 enter up enter");
    assert_eq!(editor(&app).routine.tasks.len(), 2);
    assert!(!app.routines_dirty);
}

#[test]
fn other_editor_rows_have_no_task_menu() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "enter");
    assert!(matches!(editor(&app).popup, Some(routines::EdPopup::Name { .. })), "nombre");
    ks(&mut app, "esc down*4 enter");
    assert!(matches!(editor(&app).popup, Some(routines::EdPopup::Task { at: None, .. })), "+ Agregar tarea");
    ks(&mut app, "esc down enter");
    assert_eq!(app.screen, app::Screen::RoutineStep, "meditación");
}

#[test]
fn keys_that_used_to_edit_do_nothing_in_the_editor() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down");
    press(&mut app, "adrmgjkq");
    ctrl(&mut app, "jkqg");
    ks(&mut app, "del home end");
    assert!(editor(&app).popup.is_none() && !editor(&app).dirty());
    assert_eq!(editor(&app).cursor, 1);
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(app.exit.is_none() && app.popup.is_none());
}

#[test]
fn m_does_not_open_the_setting_and_typing_a_task_keeps_every_letter() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*4 enter");
    type_text(&mut app, "mm?q 0t");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(matches!(&editor(&app).popup, Some(routines::EdPopup::Task { text, .. }) if text == "mm?q 0t"));
    assert!(app.help.is_none() && app.exit.is_none());
}

#[test]
fn esc_with_unsaved_changes_asks_with_keep_editing_preselected() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Routines);
    edit_first_again(&mut app);
    ks(&mut app, "down enter down*2 enter up enter"); // borra la primera tarea
    ks(&mut app, "esc");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(matches!(editor(&app).popup, Some(routines::EdPopup::Unsaved { sel: 2 })));
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("Guardar") && t.contains("Descartar") && t.contains("> Seguir editando"), "{t}");
    assert!(t.contains("cambios sin guardar") || t.contains("Hay cambios sin guardar"), "{t}");
    ks(&mut app, "esc"); // Esc = seguir editando
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(editor(&app).popup.is_none());
    ks(&mut app, "esc enter"); // Enter de entrada también sigue editando
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(editor(&app).popup.is_none());
    ks(&mut app, "esc up enter"); // descartar
    assert_eq!(app.screen, app::Screen::Routines);
    assert_eq!(app.routines[0].tasks.len(), 3);
    edit_first_again(&mut app);
    ks(&mut app, "down enter down*2 enter up enter esc up*2 enter"); // guardar
    assert_eq!(app.screen, app::Screen::Routines);
    assert_eq!(app.routines[0].tasks.len(), 2);
    assert!(app.routines_dirty);
}

/// Desde la lista de rutinas (ya abierta desde Configuración), edita la primera.
fn edit_first_again(app: &mut App) {
    assert_eq!(app.screen, app::Screen::Routines);
    ks(app, "up*10 enter enter");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
}

#[test]
fn renaming_the_last_routine_keeps_it_as_the_last() {
    let mut app = app_with_routines();
    app.last_routine = Some("Mañana de estudio".into());
    edit_first(&mut app);
    ks(&mut app, "enter");
    type_text(&mut app, "!");
    ks(&mut app, "enter");
    ed_save(&mut app);
    assert_eq!(app.last_routine.as_deref(), Some("Mañana de estudio!"));
    assert!(app.last_dirty);
}

#[test]
fn the_meditation_row_opens_the_setting_and_keeps_the_cursor() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*5 enter");
    assert_eq!(app.screen, app::Screen::RoutineStep);
    let d = editor(&app).step.as_ref().unwrap();
    assert!(d.enabled && !d.free, "parte de lo que ya tiene la rutina");
    ks(&mut app, "down right enter");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert_eq!(editor(&app).routine.meditation, setting(None));
    assert_eq!(editor(&app).cursor, MED_ROW, "el cursor no se mueve");
}

#[test]
fn meditation_screen_switches_mode_and_duration() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*5 enter");
    assert_eq!(app.screen, app::Screen::RoutineStep);
    let t = screen_text(&app, 80, 20);
    assert!(t.contains("Duración fija") && t.contains("10 minutos"), "{t}");
    ks(&mut app, "down*2 left up right"); // 10 -> 5, modo libre
    let t = screen_text(&app, 80, 20);
    assert!(t.contains("se elige en el momento"), "{t}");
    assert!(t.contains("↑↓ campo   ←→ cambiar   Enter listo"), "{t}");
    ks(&mut app, "enter");
    assert_eq!(editor(&app).routine.meditation, setting(None));
    ed_save(&mut app);
    assert_eq!(app.routines[0].meditation, setting(None));
    assert_eq!(app.routines[0].tasks.len(), 3);
}

#[test]
fn meditation_can_be_turned_off_and_on_again() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*5 enter left"); // no
    let t = screen_text(&app, 80, 20);
    assert!(!t.contains("Duración fija"), "sin meditación no hay modo ni duración:\n{t}");
    ks(&mut app, "down right enter"); // sin efecto estando en «no»
    assert_eq!(editor(&app).routine.meditation, None);
    assert!(screen_text(&app, 80, 24).contains("Meditación · no"));
    ed_save(&mut app);
    assert_eq!(app.routines[0].summary(), "3 pasos");
    edit_first_again(&mut app);
    ks(&mut app, "down*5 enter right enter"); // sí: arranca en fija de 10
    assert_eq!(editor(&app).routine.meditation, setting(Some(10)));
}

#[test]
fn esc_on_the_meditation_screen_discards_the_change() {
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*5 enter down right esc");
    assert_eq!(app.screen, app::Screen::RoutineEditor);
    assert!(!editor(&app).dirty());
}

// ---------------------------------------------------------- rutina en curso

#[test]
fn running_a_routine_silences_ambient_sounds_and_gives_them_back() {
    let mut app = app_with_routines();
    app.state.active = vec!["noise-brown".into()];
    home(&mut app, 0);
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
    home(&mut app, 0);
    app.key(key(Esc));
    assert!(sent(&mut app).is_empty());
}

#[test]
fn marks_go_in_any_order_and_count() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    assert!(screen_text(&app, 80, 24).contains("0 de 4 listos"));
    ks(&mut app, "down*2"); // última tarea primero
    press(&mut app, " ");
    ks(&mut app, "up*2");
    press(&mut app, " "); // marca la 0
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("2 de 4 listos"), "{t}");
    assert!(app.popup.is_none() && app.med_popup.is_none());
}

#[test]
fn space_unmarks_done_steps_and_keeps_the_cursor() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    press(&mut app, "  "); // 0 y 1 hechos; el cursor queda en el 2
    assert_eq!(app.run.as_ref().unwrap().done_count(), 2);
    app.key(key(Up)); // sobre el 1, hecho
    press(&mut app, " ");
    let run = app.run.as_ref().unwrap();
    assert_eq!(run.marks[1], routines::Mark::Pending);
    assert_eq!(run.cursor, 1, "desmarcar no avanza");
    app.key(key(Up)); // sobre el 0, hecho
    press(&mut app, " ");
    let run = app.run.as_ref().unwrap();
    assert_eq!(run.marks[0], routines::Mark::Pending);
    assert_eq!(run.cursor, 0);
    assert!(screen_text(&app, 80, 24).contains("0 de 4 listos"));
}

#[test]
fn letters_do_nothing_in_a_running_routine() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    press(&mut app, " ");
    app.key(key(Up));
    press(&mut app, "usjkqhl");
    let run = app.run.as_ref().unwrap();
    assert_eq!(run.done_count(), 1);
    assert_eq!(run.marks[0], routines::Mark::Done);
    assert_eq!(run.cursor, 0);
    assert!(app.notice.is_none() && app.popup.is_none() && app.exit.is_none());
    let t = screen_text(&app, 80, 24);
    assert!(!t.contains("deshac") && !t.contains("saltear"));
}

#[test]
fn finishing_every_step_enters_focus_and_gives_back_the_ambient() {
    let mut app = app_with_routines();
    app.state.active = vec!["noise-brown".into()];
    home(&mut app, 0);
    sent(&mut app);
    app.state.paused = true;
    press(&mut app, "   "); // las tres tareas
    assert_eq!(app.screen, app::Screen::RoutineRun, "falta la meditación");
    let id = start_meditation(&mut app);
    sent(&mut app);
    end_meditation(&mut app, &id);
    assert_eq!(app.screen, app::Screen::Focus);
    assert!(app.run.is_none());
    let out = sent(&mut app);
    assert!(out.iter().any(|c| matches!(c, Command::PauseAll)));
    assert!(out.iter().any(|c| matches!(c, Command::Watch { levels: true })));
}

/// Va al paso de meditación (el último), la arranca con espacio y simula que suena.
fn start_meditation(app: &mut App) -> String {
    app.run.as_mut().unwrap().cursor = 3;
    press(app, " ");
    let id = match sent(app).pop() {
        Some(Command::PlayMeditation { id }) => id,
        other => panic!("salió {other:?}"),
    };
    app.state.meditation =
        Some(crate::protocol::MeditationPlayback { id: id.clone(), position_secs: 10, total_secs: 600, paused: false });
    id
}

/// El daemon suelta la meditación; con `entera` también avisa que terminó (contador e id).
fn release_meditation(app: &mut App, id: &str, entera: bool) {
    let mut s = app.state.clone();
    s.meditation = None;
    if entera {
        s.meditations_finished += 1;
        s.last_meditation_finished = Some(id.into());
    }
    app.set_state(s);
}

fn end_meditation(app: &mut App, id: &str) {
    release_meditation(app, id, true);
}

#[test]
fn fixed_meditation_gets_a_random_pick_and_space_plays_then_toggles() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    ks(&mut app, "down*3");
    let t = screen_text(&app, 90, 24);
    assert!(t.contains("Meditación 10 min · ") && t.contains("(al azar)"), "{t}");
    press(&mut app, " ");
    let id = match sent(&mut app).pop() {
        Some(Command::PlayMeditation { id }) => id,
        other => panic!("salió {other:?}"),
    };
    assert!(id == "calma" || id == "scan", "tiene que ser de la categoría de 10: {id}");
    app.state.meditation =
        Some(crate::protocol::MeditationPlayback { id, position_secs: 192, total_secs: 600, paused: false });
    assert!(screen_text(&app, 90, 24).contains("▶ 03:12 / 10:00"));
    press(&mut app, " ");
    assert!(matches!(sent(&mut app).pop(), Some(Command::ToggleMeditation)));
    assert_eq!(app.run.as_ref().unwrap().marks[3], routines::Mark::Pending);
    app.state.meditation.as_mut().unwrap().paused = true;
    assert!(screen_text(&app, 90, 24).contains("⏸ 03:12 / 10:00"));
}

#[test]
fn the_audio_ending_marks_the_step_and_it_cannot_be_unmarked() {
    use routines::Mark;
    let mut app = app_with_routines();
    home(&mut app, 0);
    let id = start_meditation(&mut app);
    sent(&mut app);
    end_meditation(&mut app, &id);
    // Con las tareas pendientes la rutina sigue.
    assert_eq!(app.screen, app::Screen::RoutineRun);
    let run = app.run.as_ref().unwrap();
    assert_eq!(run.marks[3], Mark::Done);
    assert_eq!(run.cursor, 3);
    press(&mut app, " ");
    assert_eq!(app.run.as_ref().unwrap().marks[3], Mark::Done, "no se desmarca");
    assert!(sent(&mut app).is_empty(), "ni vuelve a sonar");
}

#[test]
fn a_meditation_cut_midway_does_not_mark_the_step() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    let id = start_meditation(&mut app);
    // Aunque la última posición vista estuviera pegada al final: sin señal no se marca.
    app.state.meditation.as_mut().unwrap().position_secs = 599;
    release_meditation(&mut app, &id, false);
    assert_eq!(app.run.as_ref().unwrap().marks[3], routines::Mark::Pending);
}

#[test]
fn a_finish_signal_from_before_starting_or_for_another_meditation_does_not_mark() {
    use routines::Mark;
    let mut app = app_with_routines();
    // Ya había terminado una antes de entrar a la rutina.
    app.state.meditations_finished = 3;
    app.state.last_meditation_finished = Some("scan".into());
    home(&mut app, 0);
    let id = start_meditation(&mut app);
    let mut s = app.state.clone();
    s.meditations_finished += 1;
    s.last_meditation_finished = Some("otra".into());
    app.set_state(s);
    assert_eq!(app.run.as_ref().unwrap().marks[3], Mark::Pending);
    end_meditation(&mut app, &id);
    assert_eq!(app.run.as_ref().unwrap().marks[3], Mark::Done);
}

#[test]
fn enter_on_a_task_step_toggles_the_mark() {
    use routines::Mark;
    let mut app = app_with_routines();
    home(&mut app, 0);
    ks(&mut app, "enter");
    assert_eq!(app.run.as_ref().unwrap().marks[0], Mark::Done);
    ks(&mut app, "up enter");
    assert_eq!(app.run.as_ref().unwrap().marks[0], Mark::Pending);
}

#[test]
fn enter_on_the_meditation_step_starts_and_pauses_but_never_marks() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    ks(&mut app, "down*3 enter");
    let id = match sent(&mut app).pop() {
        Some(Command::PlayMeditation { id }) => id,
        other => panic!("salió {other:?}"),
    };
    assert_eq!(app.run.as_ref().unwrap().done_count(), 0);
    app.state.meditation =
        Some(crate::protocol::MeditationPlayback { id, position_secs: 10, total_secs: 600, paused: false });
    ks(&mut app, "enter");
    assert!(matches!(sent(&mut app).pop(), Some(Command::ToggleMeditation)));
    assert_eq!(app.run.as_ref().unwrap().done_count(), 0);
}

#[test]
fn space_on_the_meditation_step_never_marks_it() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    ks(&mut app, "down*3");
    press(&mut app, " ");
    let run = app.run.as_ref().unwrap();
    assert_eq!(run.done_count(), 0);
    assert!(matches!(sent(&mut app).pop(), Some(Command::PlayMeditation { .. })));
}

#[test]
fn fixed_step_without_meditations_warns_and_lets_pick_another_duration() {
    let mut app = app_with_routines();
    app.routines[0].meditation = setting(Some(20));
    home(&mut app, 0);
    ks(&mut app, "down*3");
    let t = screen_text(&app, 90, 24);
    assert!(t.contains("No hay meditaciones de 20 minutos") && t.contains("Enter elige otra duración"), "{t}");
    press(&mut app, " ");
    assert!(screen_text(&app, 90, 24).contains("¿cuánto tiempo?"));
    ks(&mut app, "down up enter"); // 5 minutos
    assert_eq!(app.run.as_ref().unwrap().picks[3].as_ref().unwrap().id, "resp");
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
fn leaving_a_routine_stops_the_meditation_it_started() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    ks(&mut app, "down*3");
    press(&mut app, " ");
    sent(&mut app);
    app.key(key(Esc));
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::StopMeditation)));
}

#[test]
fn routine_screens_do_not_animate() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    assert!(!app.animating());
}

// -------------------------------------------- teclas sueltas que se sacaron

/// Todo lo que cambia una tecla, en una sola cadena para comparar antes y después.
fn state_print(app: &App) -> String {
    let (ed_popup, ed_cursor) = match &app.editor {
        Some(e) => (e.popup.is_some(), e.cursor),
        None => (false, 0),
    };
    format!(
        "{:?}|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{:?}",
        app.screen,
        app.home_cursor,
        app.config_cursor,
        app.med_cursor,
        app.sound_cursor,
        app.preset_list_cursor,
        app.sync_cursor,
        app.list_cursor,
        app.deep,
        app.cursor,
        app.popup.is_some(),
        app.med_popup.is_some(),
        app.list_popup.is_some(),
        ed_popup,
        ed_cursor,
        app.builder.as_ref().map_or(0, |b| b.cursor + b.picked.len()),
        app.run.as_ref().map_or(0, |r| r.cursor + r.done_count()),
        app.exit,
        app.help.is_some(),
        app.outbox,
    )
}

/// Cantidad de pantallas de `inert_screen`.
const INERT_SCREENS: usize = 14;

/// Una pantalla donde las teclas sueltas ya no hacen nada, lista para probar.
fn inert_screen(i: usize) -> (&'static str, App) {
    match i {
        0 => ("inicio", app_with_routines()),
        1 => {
            let mut a = app_with_routines();
            home(&mut a, 3);
            ("configuración", a)
        }
        2 => {
            let mut a = app_with_routines();
            a.go(app::Screen::Meditations);
            ("meditaciones", a)
        }
        3 => {
            let mut a = app_with_sounds_and_presets();
            open_sounds(&mut a);
            ("sonidos", a)
        }
        4 => {
            let mut a = app_with_sounds_and_presets();
            open_presets(&mut a);
            ("presets", a)
        }
        5 => {
            let mut a = app_with_sounds_and_presets();
            open_builder(&mut a);
            sent(&mut a);
            ("nuevo preset", a)
        }
        6 => {
            let mut a = app_with_routines();
            config(&mut a, 4);
            ("sincronización", a)
        }
        7 => {
            let mut a = app_with_routines();
            home(&mut a, 1);
            ("rutinas (inicio)", a)
        }
        8 => {
            let mut a = app_with_routines();
            config(&mut a, 0);
            ("rutinas (config)", a)
        }
        9 => {
            let mut a = app_with_routines();
            edit_first(&mut a);
            ("editor", a)
        }
        10 => {
            let mut a = app_with_routines();
            edit_first(&mut a);
            ks(&mut a, "down*5 enter");
            ("ajuste de meditación", a)
        }
        11 => {
            let mut a = app_with_routines();
            home(&mut a, 0);
            ("rutina en curso", a)
        }
        12 => {
            let mut a = focus_app();
            a.state.active = vec!["noise-brown".into()];
            a.state.master = 0.5;
            ("focus", a)
        }
        _ => {
            let mut a = focus_app();
            deep_on(&mut a);
            ("focus (deep work)", a)
        }
    }
}

#[test]
fn removed_keys_do_nothing_on_any_screen() {
    // Letras de atajos viejos, dígitos del inicio y del timer, +/-, y las teclas de edición viejas.
    let chars = "qjklhasyndemrgpxuQJKLHASYNDEMRGPX+-=1234";
    for i in 0..INERT_SCREENS {
        let (name, base) = inert_screen(i);
        let before = state_print(&base);
        for c in chars.chars() {
            let (_, mut app) = inert_screen(i);
            press(&mut app, &c.to_string());
            assert_eq!(state_print(&app), before, "{name}: la tecla «{c}» cambió algo");
        }
        for code in [Home, End, Delete, BackTab] {
            let (_, mut app) = inert_screen(i);
            app.key(key(code));
            assert_eq!(state_print(&app), before, "{name}: {code:?} cambió algo");
        }
        for c in "qgjkh".chars() {
            let (_, mut app) = inert_screen(i);
            ctrl(&mut app, &c.to_string());
            assert_eq!(state_print(&app), before, "{name}: Ctrl+{c} cambió algo");
        }
    }
}

#[test]
fn only_ctrl_c_is_a_shortcut_and_it_stays_hidden_everywhere() {
    for i in 0..INERT_SCREENS {
        let (name, mut app) = inert_screen(i);
        ctrl(&mut app, "c");
        assert!(matches!(app.exit, Some(app::Exit::Detach)), "{name}");
    }
    // Nunca aparece en pantalla ni en las ayudas.
    let app = app_with_routines();
    assert!(!screen_text(&app, 100, 24).contains("Ctrl"));
    // En un campo de texto, Ctrl+letra no escribe nada ni sale (salvo Ctrl+C).
    let mut app = app_with_routines();
    config(&mut app, 0);
    ks(&mut app, "down*40 enter");
    ctrl(&mut app, "q");
    assert!(app.exit.is_none());
    match app.editor.as_ref().and_then(|e| e.popup.as_ref()) {
        Some(routines::EdPopup::Name { text, .. }) => assert!(text.is_empty()),
        _ => panic!("popup de nombre"),
    }
}

#[test]
fn caps_lock_and_shift_still_count_as_lowercase_where_letters_are_keys() {
    let mut app = focus_app();
    with_timer(&mut app, 1200);
    press(&mut app, "T");
    assert_eq!(pause_cmds(&mut app), 1);
    // Shift+letra llega igual: mayúscula con el modificador SHIFT.
    app.key(KeyEvent::new(Char('T'), KeyModifiers::SHIFT));
    assert_eq!(pause_cmds(&mut app), 1);
    press(&mut app, "?");
    assert!(app.help.is_some());
}

#[test]
fn text_fields_keep_their_capitals_and_every_character() {
    use app::{Popup, Screen};
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    ks(&mut app, "down*40 enter");
    press(&mut app, "HTTP://X?0 t");
    match &app.popup {
        Some(Popup::Add(f)) => assert_eq!(f.url, "HTTP://X?0 t"),
        _ => panic!("popup de agregar"),
    }
    assert!(app.help.is_none());
    let mut app = app_with_meditations();
    app.go(Screen::Meditations);
    ks(&mut app, "down*40 enter");
    press(&mut app, "HTTP://Q?");
    match &app.med_popup {
        Some(app::MedPopup::Add { url, .. }) => assert_eq!(url, "HTTP://Q?"),
        _ => panic!("popup de meditación"),
    }
    let mut app = app_with_routines();
    config(&mut app, 0);
    ks(&mut app, "down*40 enter");
    press(&mut app, "Mañana QGJK ?0");
    match app.editor.as_ref().and_then(|e| e.popup.as_ref()) {
        Some(routines::EdPopup::Name { text, .. }) => assert_eq!(text, "Mañana QGJK ?0"),
        _ => panic!("popup de nombre"),
    }
    assert!(app.exit.is_none() && app.help.is_none());
}

// ------------------------------------------------------- rediseño v3

/// Like `screen_text`, but with the star band animated for a while, as the real loop does.
fn v3_text(app: &mut App, w: u16, h: u16) -> String {
    let band = menu::band_area(ratatui::layout::Rect::new(0, 0, w, h));
    for _ in 0..60 {
        app.animate(0.05, band.width, band.height);
    }
    screen_text(app, w, h)
}

/// The three rows of a title in block letters, as they must appear on screen.
fn title_rows(title: &str) -> Vec<String> {
    let (w, glyphs) = font::render(title, 1);
    (0..font::HEIGHT)
        .map(|r| {
            let mut row = vec![' '; w];
            for (x, rows) in &glyphs {
                for (i, c) in rows[r].chars().enumerate() {
                    row[x + i] = c;
                }
            }
            row.into_iter().collect::<String>().trim().to_string()
        })
        .collect()
}

fn has_block_title(t: &str, title: &str) -> bool {
    let rows = title_rows(title);
    let lines: Vec<&str> = t.lines().collect();
    lines.windows(3).any(|w| w.iter().zip(&rows).all(|(l, r)| l.contains(r.as_str())))
}

const TITLES: [&str; 9] =
    ["WORKHUB", "CONFIGURACIÓN", "RUTINAS", "EDITAR", "MEDITACIONES", "SONIDOS", "PRESETS", "NUEVO PRESET", "RUTINA"];

#[test]
fn every_title_renders_with_the_font() {
    for title in TITLES {
        for c in title.chars().filter(|c| *c != ' ') {
            let g = font::glyph(c);
            assert!(g.iter().any(|r| r.contains(['█', '▀', '▄'])), "falta la letra {c} de {title}");
            assert!(g.iter().all(|r| r.chars().count() == g[0].chars().count()), "{c} desparejo");
        }
        let (w, _) = font::render(title, 1);
        assert!(w <= 60, "{title} mide {w} columnas");
    }
}

#[test]
fn each_screen_shows_its_title_in_block_letters() {
    let mut shown: Vec<(&str, String)> = Vec::new();
    let mut at = |title, mut app: App| shown.push((title, v3_text(&mut app, 100, 30)));
    at("WORKHUB", app_with_routines());
    let mut app = app_with_routines();
    home(&mut app, 3);
    at("CONFIGURACIÓN", app);
    let mut app = app_with_routines();
    home(&mut app, 1);
    at("RUTINAS", app);
    let mut app = app_with_routines();
    edit_first(&mut app);
    at("EDITAR", app);
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*5 enter");
    assert_eq!(app.screen, app::Screen::RoutineStep);
    at("EDITAR", app);
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    at("MEDITACIONES", app);
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    at("SONIDOS", app);
    let mut app = app_with_sounds_and_presets();
    open_presets(&mut app);
    at("PRESETS", app);
    let mut app = app_with_sounds_and_presets();
    open_builder(&mut app);
    at("NUEVO PRESET", app);
    let mut app = app_with_routines();
    home(&mut app, 0);
    at("RUTINA", app);
    for (title, t) in shown {
        assert!(has_block_title(&t, title), "falta el título {title} en:\n{t}");
    }
}

#[test]
fn a_title_that_does_not_fit_is_plain_uppercase() {
    // 20 columnas: CONFIGURACIÓN en letras de bloque mide 51.
    let mut app = app_with_routines();
    home(&mut app, 3);
    let t = v3_text(&mut app, 24, 30);
    assert!(t.contains("CONFIGURACIÓN") && !has_block_title(&t, "CONFIGURACIÓN"), "{t}");
    // Y con el alto justo para el menú, el título grande cede el lugar a la lista.
    let t = v3_text(&mut app_with_routines(), 60, 11);
    assert!(t.contains("WORKHUB") && !has_block_title(&t, "WORKHUB"), "{t}");
}

#[test]
fn a_small_terminal_hides_the_band_but_keeps_the_block_title() {
    let stars = |t: &str| t.chars().filter(|c| "·∙✦✧".contains(*c)).count();
    let big = v3_text(&mut app_with_routines(), 100, 30);
    assert!(stars(&big) > 5, "la franja tiene que tener estrellas:\n{big}");
    let small = v3_text(&mut app_with_routines(), 100, 16);
    assert_eq!(stars(&small), 0, "sin franja no hay estrellas:\n{small}");
    assert!(has_block_title(&small, "WORKHUB"), "{small}");
    assert!(small.contains("Elegir otra rutina") && small.contains("Salir"), "{small}");
}

#[test]
fn lists_are_centered_as_a_block_keeping_their_columns() {
    let t = v3_text(&mut app_with_routines(), 100, 30);
    let col = |needle: &str| {
        let line = t.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("falta {needle}:\n{t}"));
        line[..line.find(needle).unwrap()].chars().count()
    };
    // «> Continuar…» y «  Elegir…»: el «>» y las etiquetas en columna.
    let (sel, second, third, last) = (col("> Continuar"), col("Elegir otra"), col("Ir a Focus"), col("Salir"));
    assert_eq!(sel + 2, second);
    assert_eq!(second, third);
    assert_eq!(second, last);
    // El bloque queda al medio: el margen izquierdo es parecido al derecho.
    let widest = t
        .lines()
        .filter(|l| l.contains(" Continuar") || l.contains("Configuración"))
        .map(|l| l.trim_end().chars().count())
        .max()
        .unwrap();
    let (left, right) = (sel, 100 - widest);
    assert!(left.abs_diff(right) <= 2 && left > 20, "izq {left} der {right}:\n{t}");
    // Los atajos van centrados abajo.
    let keys = t.lines().find(|l| l.contains("↑↓ mover   Enter elegir")).unwrap();
    let (l, r) = (keys.chars().take_while(|c| *c == ' ').count(), 100 - keys.trim_end().chars().count());
    assert!(l.abs_diff(r) <= 2, "{keys:?}");
}

#[test]
fn the_selected_row_keeps_its_mark_on_every_list() {
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    assert!(v3_text(&mut app, 100, 30).contains(">   Templo Jedi"), "sonidos");
    ks(&mut app, "down*40");
    assert!(v3_text(&mut app, 100, 30).contains("> + Agregar desde YouTube"), "sonidos");
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    assert!(v3_text(&mut app, 100, 30).contains(">   Respiración"));
    let mut app = app_with_routines();
    home(&mut app, 1);
    let t = v3_text(&mut app, 100, 30);
    assert!(t.lines().any(|l| l.contains("> Mañana de estudio")), "{t}");
    // Los resúmenes quedan en columna.
    let sum_col = |name: &str| t.lines().find(|l| l.contains(name)).map(|l| l.find("·").or(l.find("tarea"))).unwrap();
    assert!(sum_col("Tarde").is_some());
}

#[test]
fn subsections_are_uppercase_and_dim() {
    use ratatui::style::Color;
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    let band = menu::band_area(ratatui::layout::Rect::new(0, 0, 100, 30));
    app.animate(0.05, band.width, band.height);
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer().clone();
    let dim = theme::idle().dim.c();
    let found = (0..30).any(|y| {
        let line: String = (0..100).map(|x| buf[(x, y)].symbol().to_string()).collect();
        line.find("10 MINUTOS").is_some_and(|i| {
            let x = line[..i].chars().count() as u16;
            buf[(x, y)].fg == dim && buf[(x, y)].fg != Color::Reset
        })
    });
    assert!(found, "10 MINUTOS en mayúsculas y color apagado");
}

#[test]
fn popups_have_a_border_and_sit_in_the_middle() {
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "down enter");
    let t = v3_text(&mut app, 100, 30);
    let lines: Vec<&str> = t.lines().collect();
    let top = lines.iter().position(|l| l.contains('╭')).unwrap_or_else(|| panic!("sin borde:\n{t}"));
    let bottom = lines.iter().position(|l| l.contains('╰')).unwrap();
    assert!(lines[top].contains('╮') && lines[bottom].contains('╯'));
    assert!(lines[top + 1..bottom].iter().all(|l| l.contains('│')));
    let (l, r) = (lines[top].chars().take_while(|c| *c != '╭').count(), 100 - lines[top].trim_end().chars().count());
    assert!(l.abs_diff(r) <= 1, "{l} {r}");
    let (up, down) = (top, 30 - 1 - bottom);
    assert!(up.abs_diff(down) <= 1, "{up} {down}");
}

#[test]
fn the_options_popup_uses_the_same_bordered_box() {
    let mut app = focus_app();
    ks(&mut app, "tab");
    let t = screen_text(&app, 100, 30);
    let lines: Vec<&str> = t.lines().collect();
    let top = lines.iter().position(|l| l.contains('╭') && l.contains("opciones")).unwrap_or_else(|| panic!("{t}"));
    let bottom = lines.iter().position(|l| l.contains('╰')).unwrap();
    assert!(lines[top + 1..bottom].iter().all(|l| l.contains('│')), "{t}");
}

#[test]
fn the_running_routine_shows_rutina_its_name_and_the_meditation_with_its_bar() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    let t = v3_text(&mut app, 100, 30);
    assert!(has_block_title(&t, "RUTINA") && !has_block_title(&t, "WORKHUB"), "{t}");
    assert!(t.contains("Mañana de estudio") && t.contains("0 de 4 listos") && t.contains('✿'), "{t}");
    assert!(!t.contains('━'), "sin meditación sonando no hay barra");
    start_meditation(&mut app);
    app.state.meditation.as_mut().unwrap().position_secs = 192;
    let t = v3_text(&mut app, 100, 30);
    assert!(t.contains("╌") && t.contains('━') && t.contains("▶ 03:12 / 10:00"), "{t}");
    // La barra queda justo debajo del paso de meditación.
    let lines: Vec<&str> = t.lines().collect();
    let i = lines.iter().position(|l| l.contains("✿")).unwrap();
    assert!(lines[i + 1].contains('━'), "{t}");
}

#[test]
fn the_meditation_step_uses_the_second_accent() {
    let mut app = app_with_routines();
    home(&mut app, 0);
    let band = menu::band_area(ratatui::layout::Rect::new(0, 0, 100, 30));
    app.animate(0.05, band.width, band.height);
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer().clone();
    let pal = theme::idle();
    let hit = (0..30).flat_map(|y| (0..100).map(move |x| (x, y))).find(|&(x, y)| buf[(x, y)].symbol() == "✿").unwrap();
    assert_eq!(buf[hit].fg, pal.accent2.c());
}

#[test]
fn notices_and_keys_sit_in_their_rows_at_the_bottom() {
    let mut app = app_with_routines();
    app.notify("Meditación agregada");
    let t = v3_text(&mut app, 100, 30);
    let lines: Vec<&str> = t.lines().collect();
    assert!(lines[28].contains("Meditación agregada"), "{t}");
    assert!(lines[29].contains("↑↓ mover"), "{t}");
}

#[test]
fn focus_keeps_its_own_look() {
    let t = render(100, 30, |_| {});
    assert!(has_block_title(&t, "FOCUS") && !has_block_title(&t, "WORKHUB"), "{t}");
    assert!(!t.contains('✦') && !t.contains('✧') && !t.contains('╌'), "la franja calma es solo de los menús:\n{t}");
    let deep = render(100, 30, |a| a.deep = app::Deep::On);
    assert!(deep.contains("? ayuda") && !deep.contains('✦') && !deep.contains('╌'), "{deep}");
}

#[test]
fn the_band_costs_nothing_extra_while_idle() {
    let mut app = app_with_routines();
    let band = menu::band_area(ratatui::layout::Rect::new(0, 0, 100, 30));
    for _ in 0..100 {
        app.animate(0.25, band.width, band.height);
    }
    assert!(!app.band.busy() && !app.animating(), "la franja no pide cuadros rápidos");
}

// ------------------------------------------------ espacio = Enter

/// Todo lo que se ve o se manda después de una tecla.
fn fingerprint(app: &mut App) -> String {
    let out = sent(app);
    let exit = (app.exit == Some(app::Exit::Quit), app.exit == Some(app::Exit::Detach));
    format!("{:?}|{out:?}|{exit:?}|{}", app.screen, screen_text(app, 80, 24))
}

/// Prepara dos apps iguales, aprieta Enter en una y espacio en la otra y compara. También
/// exige que Enter haga algo, para que el caso no pase por no hacer nada.
fn same_as_enter(name: &str, setup: impl Fn(&mut App)) {
    let build = || {
        let mut app = app_with_routines();
        app.routines[0].meditation = setting(Some(5)); // una sola, sin azar
        let mut st = app.state.clone();
        st.presets = vec![Preset { name: "Rivendell".into(), layers: [("noise-brown".to_string(), 0.3)].into() }];
        st.sounds = library::all_sounds().unwrap();
        app.set_state(st);
        setup(&mut app);
        sent(&mut app);
        app
    };
    let (mut a, mut b) = (build(), build());
    let before = fingerprint(&mut build());
    a.key(key(Enter));
    press(&mut b, " ");
    let (fa, fb) = (fingerprint(&mut a), fingerprint(&mut b));
    assert_ne!(fa, before, "{name}: Enter no hizo nada");
    assert_eq!(fa, fb, "{name}: espacio y Enter difieren");
}

#[test]
fn space_and_enter_do_the_same_everywhere_outside_text_fields() {
    use app::Popup;
    same_as_enter("inicio", |_| {});
    same_as_enter("configuración", |a| ks(a, "down*3"));
    same_as_enter("inicio: salir", |a| ks(a, "down*4"));
    same_as_enter("antes de arrancar: tiempo", |a| a.enter_focus());
    same_as_enter("antes de arrancar: sonidos", |a| {
        a.enter_focus();
        a.key(key(Enter));
    });
    same_as_enter("salir (sonando)", |a| a.popup = Some(Popup::Quit { sel: 0 }));
    same_as_enter("salir (en silencio)", |a| a.popup = Some(Popup::ConfirmQuit { sel: 0 }));
    same_as_enter("borrar sonido", |a| {
        a.popup = Some(Popup::DeleteSound { id: "lluvia".into(), name: "Lluvia".into(), sel: 0 })
    });
    same_as_enter("borrar preset", |a| a.popup = Some(Popup::DeletePreset { name: "Rivendell".into(), sel: 0 }));
    same_as_enter("descartar preset", |a| a.popup = Some(Popup::DiscardPreset { sel: 0 }));
    same_as_enter("sonidos: agregar", |a| {
        a.go(app::Screen::Sounds);
    });
    same_as_enter("presets: nuevo", |a| {
        a.go(app::Screen::Presets);
    });
    same_as_enter("rutinas: nueva", |a| ks(a, "down enter"));
    same_as_enter("rutinas: menú", |a| ks(a, "down enter down"));
    same_as_enter("rutinas: empezar", |a| ks(a, "down enter down enter"));
    same_as_enter("rutinas: borrar", |a| ks(a, "down enter down enter down*2"));
    same_as_enter("rutinas: sí, borrar", |a| {
        ks(a, "down enter down enter down*2 enter up");
    });
    same_as_enter("editor: nombre", edit_first);
    same_as_enter("editor: menú de tarea", |a| {
        edit_first(a);
        ks(a, "down");
    });
    same_as_enter("editor: editar texto", |a| {
        edit_first(a);
        ks(a, "down enter");
    });
    same_as_enter("editor: mover", |a| {
        edit_first(a);
        ks(a, "down enter down");
    });
    same_as_enter("editor: soltar", |a| {
        edit_first(a);
        ks(a, "down enter down enter");
    });
    same_as_enter("editor: borrar tarea", |a| {
        edit_first(a);
        ks(a, "down enter down*2");
    });
    same_as_enter("editor: ajuste de meditación", |a| {
        edit_first(a);
        ks(a, "down*5");
    });
    same_as_enter("editor: guardar", |a| {
        edit_first(a);
        ks(a, "down*30");
    });
    same_as_enter("editor: sin guardar", |a| {
        edit_first(a);
        ks(a, "down enter down*2 enter up enter esc up*2");
    });
    same_as_enter("ajuste de meditación", |a| {
        edit_first(a);
        ks(a, "down*5 enter");
    });
    same_as_enter("rutina en curso: elegir duración", |a| {
        a.routines[0].meditation = setting(Some(20));
        home(a, 0);
        ks(a, "down*3 enter");
    });
    same_as_enter("rutina en curso: tarea", |a| home(a, 0));
    same_as_enter("rutina en curso: meditación", |a| {
        home(a, 0);
        ks(a, "down*3");
    });
    same_as_enter("meditaciones: agregar", |a| a.screen = app::Screen::Meditations);
    same_as_enter("meditaciones: menú", |a| {
        a.screen = app::Screen::Meditations;
        ks(a, "down");
    });
    same_as_enter("meditaciones: borrar", |a| {
        a.screen = app::Screen::Meditations;
        ks(a, "down enter down");
    });
    same_as_enter("meditaciones: sí, borrar", |a| {
        a.screen = app::Screen::Meditations;
        ks(a, "down enter down enter up");
    });
    same_as_enter("meditaciones: categoría", |a| {
        a.screen = app::Screen::Meditations;
        ks(a, "enter");
        a.paste("https://youtu.be/abc");
        a.key(key(Enter));
    });
    same_as_enter("armado: guardar", |a| {
        a.go(app::Screen::Presets);
        ks(a, "enter enter down*40");
    });
    same_as_enter("focus: encender sonido", |a| {
        a.screen = app::Screen::Focus;
    });
    same_as_enter("opciones: timer", |a| {
        a.screen = app::Screen::Focus;
        ks(a, "tab");
    });
    same_as_enter("opciones: deep work", |a| {
        a.screen = app::Screen::Focus;
        ks(a, "tab down");
    });
    same_as_enter("opciones: cargar preset", |a| {
        a.screen = app::Screen::Focus;
        ks(a, "tab down*2");
    });
    same_as_enter("opciones: pausar todo", |a| {
        a.screen = app::Screen::Focus;
        ks(a, "tab down*4");
    });
    same_as_enter("opciones: apagar todo", |a| {
        a.screen = app::Screen::Focus;
        ks(a, "tab down*5");
    });
    same_as_enter("opciones: volver al inicio", |a| {
        a.screen = app::Screen::Focus;
        ks(a, "tab down*6");
    });
    same_as_enter("opciones: salir", |a| {
        a.screen = app::Screen::Focus;
        ks(a, "tab down*7");
    });
    same_as_enter("timer: 25 minutos", |a| a.popup = Some(Popup::Timer { sel: 0, custom: None }));
    same_as_enter("cargar preset", |a| a.popup = Some(Popup::LoadPreset { sel: 0 }));
}

#[test]
fn space_saves_the_meditation_setting_like_enter() {
    // Dentro del ajuste de meditación Enter guarda y vuelve al editor; espacio igual.
    let mut a = app_with_routines();
    edit_first(&mut a);
    ks(&mut a, "down*5 enter");
    assert_eq!(a.screen, app::Screen::RoutineStep);
    press(&mut a, " ");
    assert_eq!(a.screen, app::Screen::RoutineEditor);
}

#[test]
fn space_still_types_in_every_text_field() {
    use app::Popup;
    use routines::EdPopup;
    // Nombre de la rutina.
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "enter");
    press(&mut app, "a b");
    match &app.editor.as_ref().unwrap().popup {
        Some(EdPopup::Name { text, .. }) => assert!(text.ends_with("a b"), "{text}"),
        _ => panic!("falta el nombre"),
    }
    // Texto de la tarea.
    let mut app = app_with_routines();
    edit_first(&mut app);
    ks(&mut app, "down*4 enter");
    press(&mut app, "a b");
    match &app.editor.as_ref().unwrap().popup {
        Some(EdPopup::Task { text, .. }) => assert_eq!(text, "a b"),
        _ => panic!("falta la tarea"),
    }
    // Nombre del preset.
    let mut app = app_with_routines();
    app.popup = Some(Popup::SavePreset { name: String::new() });
    press(&mut app, "a b");
    assert!(matches!(&app.popup, Some(Popup::SavePreset { name }) if name == "a b"));
    // Link de la meditación.
    let mut app = app_with_meditations();
    app.screen = app::Screen::Meditations;
    ks(&mut app, "down*40 enter");
    press(&mut app, "a b");
    assert!(matches!(&app.med_popup, Some(app::MedPopup::Add { url, category: None }) if url == "a b"));
}

#[test]
fn space_is_still_ignored_in_the_custom_timer_minutes() {
    use app::Popup;
    let mut app = app_with_routines();
    app.popup = Some(Popup::Timer { sel: 0, custom: Some(String::new()) });
    press(&mut app, "5 5");
    assert!(matches!(&app.popup, Some(Popup::Timer { custom: Some(s), .. }) if s == "55"));
    assert!(sent(&mut app).is_empty(), "espacio no confirma");
}

// ------------------------------------------------------- sincronización

use crate::progress::Out;
use std::sync::{Arc, Mutex, mpsc};
use syncmenu::SyncKind;

/// Un runner falso: avisa qué le pidieron, cuenta dos cosas y espera la señal para terminar.
/// Devuelve la señal (manda `Ok`/`Err` con el texto del error) y los pedidos que recibió.
type Finish = mpsc::Sender<Result<(), String>>;

fn gated_runner(app: &mut App) -> (Finish, Arc<Mutex<Vec<SyncKind>>>) {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let rx = Mutex::new(rx);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&asked);
    app.sync_runner = Arc::new(move |kind, out: &mut Out| {
        seen.lock().unwrap().push(kind);
        out.line("primera línea");
        out.line("segunda línea");
        let r = rx.lock().unwrap().recv().unwrap();
        r.map_err(|e| anyhow::anyhow!(e))
    });
    (tx, asked)
}

fn open_sync(app: &mut App) {
    config(app, 4);
    assert_eq!(app.screen, app::Screen::Sync);
}

/// Espera (hasta 5 s) a que la ventana cumpla `cond`, trayendo lo que llegue.
fn wait_for(app: &mut App, cond: impl Fn(&App) -> bool) {
    for _ in 0..500 {
        app.poll_sync();
        if cond(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("la ventana de sincronización no llegó al estado esperado");
}

#[test]
fn sync_menu_shows_its_three_options_and_the_big_title_fits() {
    let mut app = App::new();
    open_sync(&mut app);
    let t = screen_text(&app, 80, 24);
    for s in ["Conectar Google Drive", "Sincronizar", "Verificar archivos", "↑↓", "Esc"] {
        assert!(t.contains(s), "falta «{s}»");
    }
    assert!(font::render("SINCRONIZACIÓN", 1).0 + 2 <= 80, "las letras de bloque entran en 80 columnas");
    assert!(t.contains('█'), "el título va en letras de bloque");
}

#[test]
fn sync_menu_moves_stays_inside_the_list_and_esc_goes_back_to_config() {
    let mut app = App::new();
    open_sync(&mut app);
    ks(&mut app, "up*2");
    assert_eq!(app.sync_cursor, 0);
    ks(&mut app, "down*5");
    assert_eq!(app.sync_cursor, 2);
    app.key(key(Up));
    assert_eq!(app.sync_cursor, 1);
    app.key(key(Esc));
    assert_eq!(app.screen, app::Screen::Config);
}

#[test]
fn each_option_runs_its_own_task() {
    for (moves, kind) in [("", SyncKind::Setup), ("down", SyncKind::Sync), ("down*2", SyncKind::Verify)] {
        let mut app = App::new();
        let (done, asked) = gated_runner(&mut app);
        open_sync(&mut app);
        ks(&mut app, moves);
        press(&mut app, " "); // ␣ = Enter
        wait_for(&mut app, |a| a.sync.as_ref().is_some_and(|j| j.lines.len() == 2));
        assert_eq!(*asked.lock().unwrap(), [kind]);
        done.send(Ok(())).unwrap();
        wait_for(&mut app, |a| !a.sync_running());
    }
}

#[test]
fn while_it_runs_the_window_blocks_every_key_and_shows_the_progress() {
    let mut app = App::new();
    let (done, _) = gated_runner(&mut app);
    open_sync(&mut app);
    sent(&mut app);
    ks(&mut app, "down enter");
    wait_for(&mut app, |a| a.sync.as_ref().is_some_and(|j| j.lines.len() == 2));
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("primera línea") && t.contains("segunda línea"), "{t}");
    assert!(t.contains("Trabajando"), "dice que está en curso");

    for c in ['q', 'j', 'k', 'n', 's', 'a', 'd', ' ', '?', '\n'] {
        press(&mut app, &c.to_string());
    }
    for code in [Esc, Enter, Up, Down, Left, Right] {
        app.key(key(code));
    }
    ctrl(&mut app, "q");
    assert!(app.sync_running(), "nada la cierra");
    assert!(app.popup.is_none() && app.exit.is_none(), "ni q ni Ctrl+Q salen");
    assert_eq!(app.screen, app::Screen::Sync);
    assert_eq!(app.sync_cursor, 1, "el cursor de atrás no se movió");

    // Ctrl+C sigue siendo la salida de emergencia.
    ctrl(&mut app, "c");
    assert!(app.exit == Some(app::Exit::Detach));
    done.send(Ok(())).unwrap();
    wait_for(&mut app, |a| !a.sync_running());
}

#[test]
fn enter_space_and_esc_close_it_once_finished() {
    for close in ["enter", "space", "esc"] {
        let mut app = App::new();
        let (done, _) = gated_runner(&mut app);
        open_sync(&mut app);
        app.key(key(Enter));
        done.send(Ok(())).unwrap();
        wait_for(&mut app, |a| !a.sync_running());
        assert!(screen_text(&app, 80, 24).contains("Listo"));
        match close {
            "enter" => app.key(key(Enter)),
            "space" => press(&mut app, " "),
            _ => app.key(key(Esc)),
        }
        assert!(app.sync.is_none(), "{close} cierra");
        assert_eq!(app.screen, app::Screen::Sync, "y se queda en el submenú");
    }
}

#[test]
fn a_finished_window_ignores_other_keys() {
    let mut app = App::new();
    let (done, _) = gated_runner(&mut app);
    open_sync(&mut app);
    app.key(key(Enter));
    done.send(Ok(())).unwrap();
    wait_for(&mut app, |a| !a.sync_running());
    press(&mut app, "qjk");
    assert!(app.sync.is_some() && app.popup.is_none() && app.exit.is_none());
    assert_eq!(app.sync_cursor, 0);
}

#[test]
fn an_error_is_shown_with_its_message() {
    let mut app = App::new();
    let (done, _) = gated_runner(&mut app);
    open_sync(&mut app);
    app.key(key(Enter));
    sent(&mut app);
    done.send(Err("rclone falló: sin permiso".into())).unwrap();
    wait_for(&mut app, |a| !a.sync_running());
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("✗") && t.contains("rclone falló: sin permiso"), "{t}");
    assert!(sent(&mut app).is_empty(), "si falló no hace falta releer nada");
    assert!(!app.routines_stale);
}

#[test]
fn success_asks_the_daemon_to_reload_and_the_routines_to_be_reread() {
    let mut app = App::new();
    let (done, _) = gated_runner(&mut app);
    open_sync(&mut app);
    app.key(key(Enter));
    sent(&mut app);
    done.send(Ok(())).unwrap();
    wait_for(&mut app, |a| !a.sync_running());
    assert!(sent(&mut app).iter().any(|c| matches!(c, Command::Reload)));
    assert!(app.routines_stale);
    app.poll_sync();
    assert!(sent(&mut app).is_empty(), "una sola vez");
}

#[cfg(unix)]
#[test]
fn a_live_status_replaces_itself_and_a_line_clears_it() {
    let mut app = App::new();
    app.sync_runner = Arc::new(|_, out: &mut Out| {
        out.run(std::process::Command::new("sh").args(["-c", "printf 'a\\rb\\r'; sleep 0.3; echo fin"]))?;
        Ok(())
    });
    open_sync(&mut app);
    app.key(key(Enter));
    wait_for(&mut app, |a| a.sync.as_ref().is_some_and(|j| j.live.as_deref() == Some("a")));
    wait_for(&mut app, |a| !a.sync_running());
    let job = app.sync.as_ref().unwrap();
    assert_eq!(job.lines, ["fin"]);
    assert_eq!(job.live, None, "la línea final borra el estado");
}

#[test]
fn a_panicking_task_does_not_leave_the_window_stuck() {
    let mut app = App::new();
    app.sync_runner = Arc::new(|_, _: &mut Out| panic!("boom"));
    open_sync(&mut app);
    app.key(key(Enter));
    wait_for(&mut app, |a| !a.sync_running());
    assert!(matches!(app.sync.as_ref().unwrap().result, Some(Err(_))));
    app.key(key(Esc));
    assert!(app.sync.is_none());
}

#[test]
fn a_long_link_wraps_inside_the_window() {
    let mut app = App::new();
    app.sync_runner = Arc::new(|_, out: &mut Out| {
        out.line(format!("http://127.0.0.1:53682/auth?state={}", "x".repeat(60)));
        Ok(())
    });
    open_sync(&mut app);
    app.key(key(Enter));
    wait_for(&mut app, |a| !a.sync_running());
    let t = screen_text(&app, 80, 24);
    assert!(t.contains("http://127.0.0.1:53682/auth?state="));
    assert!(t.contains("xxxxxxxx"), "el final del link se ve en la línea siguiente");
}

// ------------------------------------------------------------- ayuda (?)

/// Las filas «tecla  descripción» de la ayuda que muestra la pantalla actual.
fn help_text(app: &mut App) -> String {
    press(app, "?");
    assert!(app.help.is_some(), "la ayuda no se abrió en {:?}", app.screen);
    let t = screen_text(app, 80, 24);
    assert!(t.contains("atajos"), "{t}");
    ks(app, "esc");
    assert!(app.help.is_none());
    t
}

#[test]
fn every_screens_help_lists_only_the_new_keys() {
    // (nombre, pantalla lista, lo que tiene que decir, lo que no)
    type Check = (&'static str, App, Vec<&'static str>, Vec<&'static str>);
    let mut cases: Vec<Check> = Vec::new();
    cases.push(("inicio", app_with_routines(), vec!["↑↓", "mover", "Enter/␣", "elegir"], vec!["Esc", "ir directo"]));
    let mut a = app_with_routines();
    home(&mut a, 3);
    cases.push(("configuración", a, vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"], vec![]));
    let mut a = app_with_routines();
    config(&mut a, 4);
    cases.push(("sincronización", a, vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"], vec!["ejecutar"]));
    let mut a = app_with_routines();
    a.go(app::Screen::Meditations);
    cases.push((
        "meditaciones",
        a,
        vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"],
        vec!["agregar", "borrar", "play"],
    ));
    let mut a = app_with_sounds_and_presets();
    open_sounds(&mut a);
    cases.push(("sonidos", a, vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"], vec!["agregar", "borrar"]));
    let mut a = app_with_sounds_and_presets();
    open_presets(&mut a);
    cases.push(("presets", a, vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"], vec!["nuevo", "borrar"]));
    let mut a = app_with_sounds_and_presets();
    open_builder(&mut a);
    cases.push((
        "nuevo preset",
        a,
        vec!["↑↓", "elegir / quitar", "← →", "volumen", "Esc", "cancelar"],
        vec!["guardar"],
    ));
    let mut a = app_with_routines();
    home(&mut a, 1);
    cases.push(("rutinas (inicio)", a, vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"], vec!["empezar", "nueva"]));
    let mut a = app_with_routines();
    config(&mut a, 0);
    cases.push(("rutinas (config)", a, vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"], vec!["editar", "borrar"]));
    let mut a = app_with_routines();
    edit_first(&mut a);
    cases.push((
        "editor",
        a,
        vec!["↑↓", "Enter/␣", "elegir", "Esc", "volver"],
        vec!["Ctrl", "tarea", "guardar", "nombre"],
    ));
    let mut a = app_with_routines();
    edit_first(&mut a);
    ks(&mut a, "down*5 enter");
    cases.push(("ajuste de meditación", a, vec!["↑↓", "campo", "← →", "cambiar", "listo", "Esc"], vec!["h l", "j k"]));
    let mut a = app_with_routines();
    home(&mut a, 0);
    cases.push((
        "rutina en curso",
        a,
        vec!["↑↓", "marcar / desmarcar", "play / pausa", "Esc", "volver al menú"],
        vec![],
    ));
    let mut a = app_with_presets();
    home(&mut a, 2);
    cases.push((
        "antes de arrancar: tiempo",
        a,
        vec!["↑↓", "±5 minutos", "0-9", "escribir minutos", "seguir", "saltear"],
        vec![],
    ));
    let mut a = app_with_presets();
    home(&mut a, 2);
    ks(&mut a, "enter");
    cases.push(("antes de arrancar: sonidos", a, vec!["↑↓", "elegir", "empezar", "saltear"], vec![]));
    let mut a = focus_app();
    a.state.timer = Some(crate::protocol::TimerState { total_secs: 600, remaining_secs: 300, paused: false });
    cases.push((
        "focus",
        a,
        vec![
            "↑↓",
            "moverse",
            "← →",
            "volumen del sonido",
            "Enter/␣",
            "encender / apagar",
            "Tab",
            "opciones",
            "Esc",
            "volver al inicio",
        ],
        vec!["presets", "reinicia", "timer", "pausar", "deep work", "apagar todo", "Ctrl+Q", "salir"],
    ));
    for (name, mut app, wanted, absent) in cases {
        let t = help_text(&mut app);
        for w in &wanted {
            assert!(t.contains(w), "{name}: falta «{w}» en:\n{t}");
        }
        // Las dos listas se leen sobre la tabla de teclas, sin el borde ni el título.
        let body: String = t.lines().filter(|l| l.contains('│')).collect::<Vec<_>>().join("\n");
        for gone in absent {
            assert!(!body.contains(gone), "{name}: «{gone}» no tendría que estar en:\n{t}");
        }
        for gone in ["j k", "h l", "Ctrl+Q", "Ctrl+J", "q  salir", "1-4"] {
            assert!(!body.contains(gone), "{name}: «{gone}» es una tecla vieja:\n{t}");
        }
    }
}

#[test]
fn deep_work_help_is_the_focus_one() {
    let mut app = focus_app();
    deep_on(&mut app);
    let t = help_text(&mut app);
    assert!(t.contains("Tab") && t.contains("opciones") && !t.contains("deep work"), "{t}");
    assert_eq!(app.deep, app::Deep::On);
}

#[test]
fn question_mark_works_in_the_before_you_start_window() {
    let mut app = app_with_presets();
    home(&mut app, 2);
    assert!(matches!(app.popup, Some(app::Popup::Intro(_))));
    press(&mut app, "?");
    assert!(screen_text(&app, 80, 24).contains("escribir minutos"));
    press(&mut app, "?");
    assert!(matches!(app.popup, Some(app::Popup::Intro(app::Intro::Time { .. }))));
    app.key(key(Enter));
    press(&mut app, "?");
    assert!(screen_text(&app, 80, 24).contains("empezar"));
    press(&mut app, "?");
}

#[test]
fn question_mark_types_in_text_fields_and_leaves_other_popups_alone() {
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "down*40 enter");
    type_text(&mut app, "a?");
    assert!(app.help.is_none());
    assert_eq!(
        match &app.editor.as_ref().unwrap().popup {
            Some(routines::EdPopup::Name { text, .. }) => text.clone(),
            _ => String::new(),
        },
        "a?"
    );
    let mut app = app_with_meditations();
    app.go(app::Screen::Meditations);
    ks(&mut app, "down*40 enter");
    press(&mut app, "?");
    assert!(app.help.is_none());
    assert!(matches!(&app.med_popup, Some(app::MedPopup::Add { url, .. }) if url == "?"));
    // Con otras ventanitas abiertas la ayuda no se abre.
    let mut app = focus_app();
    ks(&mut app, "tab");
    press(&mut app, "?");
    assert!(app.help.is_none() && matches!(app.popup, Some(app::Popup::Options { .. })));
    let mut app = app_with_routines();
    home(&mut app, 1);
    ks(&mut app, "enter");
    press(&mut app, "?");
    assert!(app.help.is_none() && app.list_popup.is_some());
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    ks(&mut app, "down*40 enter");
    press(&mut app, "?");
    assert!(app.help.is_none());
    let mut app = app_with_sounds();
    open_sounds(&mut app);
    ks(&mut app, "down enter");
    press(&mut app, "?");
    assert!(app.help.is_none() && matches!(app.popup, Some(app::Popup::DeleteSound { .. })));
}

#[test]
fn help_closes_with_any_key_and_swallows_it() {
    let mut app = app_with_routines();
    for code in [Esc, Enter, Char('?')] {
        press(&mut app, "?");
        assert!(app.help.is_some());
        app.key(key(code));
        assert!(app.help.is_none());
        assert_eq!(app.screen, app::Screen::Home);
    }
    press(&mut app, "?");
    ks(&mut app, "down");
    assert!(app.help.is_none() && app.home_cursor == 0, "la tecla que la cierra no hace otra cosa");
}

#[test]
fn every_help_fits_in_80_by_24() {
    use help::Topic::*;
    for topic in [
        Home,
        Config,
        Meditations,
        Sounds,
        Presets,
        Sync,
        Builder,
        Routines,
        Editor,
        Step,
        Run,
        IntroTime,
        IntroSounds,
        Focus,
    ] {
        let mut app = app_with_routines();
        app.help = Some(topic);
        for screen in [app::Screen::Home, app::Screen::Focus] {
            app.screen = screen;
            let t = screen_text(&app, 80, 24);
            let rows: Vec<&str> = t.lines().collect();
            let top = rows.iter().position(|r| r.contains("atajos")).expect("title");
            let bottom = (top..rows.len()).find(|&y| rows[y].contains('╰')).expect("bottom border");
            assert!(top >= 1 && bottom <= 22, "{topic:?}\n{t}");
            assert!(rows[top].contains('╭') && rows[top].contains('╮'), "{topic:?}\n{t}");
        }
    }
}

#[test]
fn every_popup_and_footer_fits_in_80_by_24() {
    // Las ventanitas nuevas entran enteras en una terminal chica.
    let mut cases: Vec<(&str, App)> = Vec::new();
    let mut a = focus_app();
    ks(&mut a, "tab");
    cases.push(("opciones", a));
    let mut a = focus_app();
    ks(&mut a, "tab down*2 enter");
    cases.push(("cargar preset", a));
    let mut a = focus_app();
    ks(&mut a, "tab enter");
    cases.push(("timer", a));
    let mut a = focus_app();
    a.state.active = vec!["noise-brown".into()];
    ks(&mut a, "tab down*7 enter");
    cases.push(("salir", a));
    let mut a = app_with_routines();
    edit_first(&mut a);
    ks(&mut a, "down enter");
    cases.push(("menú de tarea", a));
    for (name, app) in cases {
        let t = screen_text(&app, 80, 24);
        let rows: Vec<&str> = t.lines().collect();
        let top = rows.iter().position(|r| r.contains('╭')).unwrap_or_else(|| panic!("{name}:\n{t}"));
        let bottom = (top..rows.len()).find(|&y| rows[y].contains('╰')).unwrap_or_else(|| panic!("{name}:\n{t}"));
        assert!(top >= 1 && bottom <= 22, "{name}\n{t}");
        assert!(rows[top..=bottom].iter().all(|r| r.chars().count() <= 80), "{name}");
    }
    // Y el pie de Focus entra con el volumen incluido.
    let app = focus_app();
    let t = screen_text(&app, 80, 24);
    let footer = t.lines().last().unwrap().to_string() + t.lines().rev().nth(1).unwrap_or("");
    assert!(footer.contains("Tab opciones") && footer.contains("vol "), "{t}");
}
