use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use super::*;

#[derive(Default)]
struct Calls {
    /// Layers the fake currently "plays".
    layers: Vec<String>,
    master: f32,
    ended: HashSet<String>,
}

/// An audio output that only records what the player asks of it. It never opens the sources.
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Calls>>);

impl AudioOut for Fake {
    fn play(&mut self, id: &str, _volume: f32, _make: Box<dyn FnOnce() -> Result<Box<dyn Source>>>) -> Result<()> {
        let mut c = self.0.lock().unwrap();
        if !c.layers.iter().any(|l| l == id) {
            c.layers.push(id.to_string());
        }
        Ok(())
    }
    fn set_volume(&self, _id: &str, _volume: f32) {}
    fn stop(&mut self, id: &str) {
        self.0.lock().unwrap().layers.retain(|l| l != id);
    }
    fn has_ended(&self, id: &str) -> bool {
        self.0.lock().unwrap().ended.contains(id)
    }
    fn set_master(&self, v: f32) {
        self.0.lock().unwrap().master = v;
    }
    fn set_fade(&self, _v: f32) {}
    fn levels(&self) -> (f32, Vec<(String, f32)>) {
        (0.0, Vec::new())
    }
    fn positions(&self) -> HashMap<String, u64> {
        HashMap::new()
    }
    fn tick(&mut self) {}
    fn is_running(&self) -> bool {
        !self.0.lock().unwrap().layers.is_empty()
    }
}

struct Rig {
    player: Player,
    fake: Fake,
    events: Receiver<Event>,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn tmp_dir(name: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "focus-core-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rig_with(name: &str, setup: impl FnOnce(&std::path::Path)) -> Rig {
    let dir = tmp_dir(name);
    setup(&dir);
    let fake = Fake::default();
    let player =
        Player::with_options(dir.clone(), PlayerOptions { state_file: None, output: Some(Box::new(fake.clone())) })
            .unwrap();
    let (tx, events) = channel();
    let tx = Mutex::new(tx);
    player.set_event_listener(Box::new(move |e| {
        let _ = tx.lock().unwrap().send(e);
    }));
    Rig { player, fake, events, dir }
}

fn rig(name: &str) -> Rig {
    rig_with(name, |_| {})
}

/// A library with one downloaded sound (file present), one whose file is missing, and one
/// meditation with its (fake) audio.
fn library_with_files(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("sounds")).unwrap();
    std::fs::create_dir_all(dir.join("meditations")).unwrap();
    std::fs::write(dir.join("sounds/lluvia.opus"), b"x").unwrap();
    std::fs::write(dir.join("meditations/calma.opus"), b"x").unwrap();
    let sound = |id: &str| {
        format!(
            "[[sound]]\nid = \"{id}\"\nname = \"{id}\"\ntheme = \"nature\"\nurl = \"u\"\nfile = \"{id}.opus\"\nsha256 = \"s\"\nduration = 60\nbytes = 1\n\n"
        )
    };
    std::fs::write(dir.join("sounds.toml"), sound("lluvia") + &sound("fantasma")).unwrap();
    std::fs::write(
        dir.join("meditations.toml"),
        "[[meditation]]\nid = \"calma\"\nname = \"Calma\"\nminutes = 5\nurl = \"u\"\nfile = \"calma.opus\"\nsha256 = \"s\"\nduration = 300\nbytes = 1\n\n\
         [[meditation]]\nid = \"perdida\"\nname = \"Perdida\"\nminutes = 5\nurl = \"u\"\nfile = \"perdida.opus\"\nsha256 = \"s\"\nduration = 300\nbytes = 1\n",
    )
    .unwrap();
}

fn wait_for<T>(rx: &Receiver<Event>, mut pick: impl FnMut(Event) -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(e) => {
                if let Some(v) = pick(e) {
                    return Some(v);
                }
            }
            Err(_) => return None,
        }
    }
    None
}

fn set(id: &str, active: bool) -> Command {
    Command::SetActive { id: id.into(), active }
}

#[test]
fn player_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Player>();
}

#[test]
fn send_changes_the_state_and_drives_the_output() {
    let r = rig("send");
    r.player.send(set("noise-pink", true)).unwrap();
    r.player.send(Command::SetVolume { id: "noise-pink".into(), volume: 0.5 }).unwrap();
    r.player.send(Command::SetMaster { volume: 0.25 }).unwrap();
    let s = r.player.state();
    assert_eq!(s.active, ["noise-pink"]);
    assert!((s.volumes["noise-pink"] - 0.5).abs() < 1e-6);
    assert!((s.master - 0.25).abs() < 1e-6);
    assert!(s.is_playing());
    assert_eq!(r.fake.0.lock().unwrap().layers, ["noise-pink"]);
    r.player.send(Command::StopAll).unwrap();
    assert!(r.fake.0.lock().unwrap().layers.is_empty());
    assert!(!r.player.state().is_playing());
}

#[test]
fn state_events_reach_the_listener() {
    let r = rig("events");
    r.player.send(set("noise-white", true)).unwrap();
    let state = wait_for(&r.events, |e| match e {
        Event::State(s) if s.active == ["noise-white"] => Some(s),
        _ => None,
    });
    assert!(state.is_some(), "llegó un State con el sonido activo");
}

#[test]
fn pause_all_and_resume_all_are_idempotent() {
    let r = rig("pause");
    r.player.send(set("noise-brown", true)).unwrap();
    for _ in 0..2 {
        r.player.send(Command::PauseAll).unwrap();
        assert!(r.player.state().paused);
        assert!(r.fake.0.lock().unwrap().layers.is_empty());
    }
    for _ in 0..2 {
        r.player.send(Command::ResumeAll).unwrap();
        let s = r.player.state();
        assert!(!s.paused);
        assert_eq!(s.active, ["noise-brown"], "no se olvidó qué capas estaban");
        assert_eq!(r.fake.0.lock().unwrap().layers, ["noise-brown"]);
    }
    // Sin nada activo no hace nada.
    r.player.send(Command::StopAll).unwrap();
    r.player.send(Command::PauseAll).unwrap();
    assert!(!r.player.state().paused);
}

#[test]
fn toggle_pause_still_toggles() {
    let r = rig("toggle");
    r.player.send(set("noise-brown", true)).unwrap();
    r.player.send(Command::TogglePause).unwrap();
    assert!(r.player.state().paused);
    r.player.send(Command::TogglePause).unwrap();
    assert!(!r.player.state().paused);
}

#[test]
fn toggle_pause_keeps_its_wire_name() {
    assert_eq!(serde_json::to_string(&Command::TogglePause).unwrap(), r#"{"cmd":"pause_all"}"#);
    assert_eq!(serde_json::to_string(&Command::PauseAll).unwrap(), r#"{"cmd":"pause"}"#);
    assert_eq!(serde_json::to_string(&Command::ResumeAll).unwrap(), r#"{"cmd":"resume"}"#);
    let c: Command = serde_json::from_str(r#"{"cmd":"pause_all"}"#).unwrap();
    assert!(matches!(c, Command::TogglePause));
}

#[test]
fn pause_all_also_pauses_the_meditation_and_resume_all_brings_it_back() {
    let r = rig_with("med-pause", library_with_files);
    r.player.send(set("noise-white", true)).unwrap();
    r.player.send(Command::PlayMeditation { id: "calma".into() }).unwrap();
    let s = r.player.state();
    assert!(s.paused, "la meditación tiene el ambiente en pausa");
    assert!(s.meditation.as_ref().is_some_and(|m| !m.paused));

    r.player.send(Command::PauseAll).unwrap();
    let s = r.player.state();
    assert!(s.paused && s.meditation.as_ref().is_some_and(|m| m.paused));
    assert!(r.fake.0.lock().unwrap().layers.is_empty(), "no suena nada");

    r.player.send(Command::ResumeAll).unwrap();
    let s = r.player.state();
    assert!(s.meditation.as_ref().is_some_and(|m| !m.paused), "la meditación vuelve");
    assert_eq!(r.fake.0.lock().unwrap().layers.len(), 1, "solo la voz: el ambiente sigue retenido");
}

#[test]
fn resume_all_does_not_resume_a_meditation_the_user_had_paused() {
    let r = rig_with("med-user", library_with_files);
    r.player.send(Command::PlayMeditation { id: "calma".into() }).unwrap();
    r.player.send(Command::ToggleMeditation).unwrap();
    r.player.send(Command::PauseAll).unwrap();
    r.player.send(Command::ResumeAll).unwrap();
    assert!(r.player.state().meditation.is_some_and(|m| m.paused));
}

#[test]
fn the_timer_runs_on_its_own_thread_and_reports_the_end() {
    let r = rig("timer");
    r.player.send(set("noise-white", true)).unwrap();
    r.player.send(Command::Timer { minutes: Some(25) }).unwrap();
    let t = r.player.state().timer.expect("hay timer");
    assert_eq!(t.total_secs, 25 * 60);
    // Sin llamar a nada más, el hilo del player ve que se acabó el tiempo.
    r.player.shared.lock().timer.as_mut().unwrap().end = Instant::now();
    r.player.shared.wake.notify_all();
    let minutes = wait_for(&r.events, |e| match e {
        Event::TimerFinished { minutes } => Some(minutes),
        _ => None,
    });
    assert_eq!(minutes, Some(25));
    let s = r.player.state();
    assert!(s.timer.is_none() && s.paused, "el timer terminó y pausó el sonido");
    assert!(r.fake.0.lock().unwrap().layers.is_empty());
}

#[test]
fn a_paused_timer_does_not_finish() {
    let r = rig("timer-pause");
    r.player.send(Command::Timer { minutes: Some(10) }).unwrap();
    r.player.send(Command::ToggleTimerPause).unwrap();
    assert!(r.player.state().timer.unwrap().paused);
    r.player.send(Command::Timer { minutes: None }).unwrap();
    assert!(r.player.state().timer.is_none());
}

#[test]
fn a_missing_sound_is_reported_and_does_not_play() {
    let r = rig_with("missing", library_with_files);
    let s = r.player.state();
    let find = |s: &State, id: &str| s.sounds.iter().find(|x| x.id == id).map(|x| x.available);
    assert_eq!(find(&s, "lluvia"), Some(true));
    assert_eq!(find(&s, "fantasma"), Some(false), "el estado marca el archivo que falta");

    r.player.send(set("fantasma", true)).unwrap();
    assert!(r.player.state().active.is_empty());
    assert!(r.fake.0.lock().unwrap().layers.is_empty());
    let notice = wait_for(&r.events, |e| match e {
        Event::Notice { text } if text.contains("fantasma") => Some(text),
        _ => None,
    });
    assert!(notice.is_some(), "avisa que falta");

    // Un archivo que se borra después de cargar la biblioteca también se marca.
    std::fs::remove_file(r.dir.join("sounds/lluvia.opus")).unwrap();
    r.player.send(set("lluvia", true)).unwrap();
    let s = r.player.state();
    assert!(s.active.is_empty());
    assert_eq!(find(&s, "lluvia"), Some(false));
}

#[test]
fn a_missing_meditation_does_not_play() {
    let r = rig_with("missing-med", library_with_files);
    assert_eq!(r.player.state().meditations.iter().filter(|m| !m.available).count(), 1);
    r.player.send(Command::PlayMeditation { id: "perdida".into() }).unwrap();
    assert!(r.player.state().meditation.is_none());
    r.player.send(Command::PlayMeditation { id: "no-existe".into() }).unwrap();
    assert!(r.player.state().meditation.is_none());
    assert!(r.fake.0.lock().unwrap().layers.is_empty());
}

#[test]
fn a_finished_meditation_counts_once_and_lets_go() {
    let r = rig_with("med-end", library_with_files);
    r.player.send(Command::PlayMeditation { id: "calma".into() }).unwrap();
    r.fake.0.lock().unwrap().ended.insert("meditation:calma:1".into());
    {
        let mut inner = r.player.shared.lock();
        inner.playback.as_mut().unwrap().ended_at = Some(Instant::now() - MEDITATION_DRAIN);
    }
    r.player.shared.wake.notify_all();
    let deadline = Instant::now() + Duration::from_secs(5);
    while r.player.state().meditations_finished == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    let s = r.player.state();
    assert_eq!((s.meditations_finished, s.last_meditation_finished.as_deref()), (1, Some("calma")));
    assert!(s.meditation.is_none());
}

#[test]
fn add_and_quit_belong_to_the_host() {
    let r = rig("host");
    assert!(r.player.send(Command::Quit).is_err());
}

#[test]
fn downloads_show_up_in_the_state() {
    let r = rig("downloads");
    assert!(r.player.download_started("u", Some("Lluvia"), false));
    assert!(!r.player.download_started("u", None, false), "ya está bajando");
    r.player.download_progress("u", "bajando", 0.5);
    let s = r.player.state();
    assert_eq!((s.downloads[0].stage.as_str(), s.downloads[0].progress), ("bajando", 0.5));
    assert!(!r.player.is_idle());
    r.player.download_finished("u", Err("boom".into()));
    assert!(r.player.state().downloads[0].error.is_some());
    assert!(r.player.is_idle(), "una descarga fallida no mantiene vivo al daemon");
}

#[test]
fn volumes_survive_a_restart() {
    let dir = tmp_dir("persist");
    let make = || {
        Player::with_options(
            dir.clone(),
            PlayerOptions { state_file: Some(dir.join("estado.toml")), output: Some(Box::new(Fake::default())) },
        )
        .unwrap()
    };
    let p = make();
    p.send(Command::SetVolume { id: "noise-pink".into(), volume: 0.9 }).unwrap();
    p.send(Command::SetMaster { volume: 0.3 }).unwrap();
    p.shutdown();
    drop(p);
    let s = make().state();
    assert!((s.volumes["noise-pink"] - 0.9).abs() < 1e-6 && (s.master - 0.3).abs() < 1e-6);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn presets_load_and_store() {
    let r = rig("presets");
    r.player
        .send(Command::StorePreset { name: "calma".into(), layers: BTreeMap::from([("noise-brown".to_string(), 0.4)]) })
        .unwrap();
    r.player.send(Command::Reload).unwrap();
    assert_eq!(r.player.state().presets.len(), 1);
    r.player.send(Command::LoadPreset { name: "calma".into() }).unwrap();
    assert_eq!(r.player.state().active, ["noise-brown"]);
}

// --- moved from the daemon ---

fn playback(total: u64) -> Playback {
    Playback {
        id: "calma".into(),
        layer: "meditation:calma:1".into(),
        total: Duration::from_secs(total),
        played: Duration::ZERO,
        since: Some(Instant::now()),
        ended_at: None,
    }
}

#[test]
fn a_meditation_finishes_only_once_the_file_ran_out_and_the_buffer_drained() {
    let mut p = playback(600);
    assert!(!p.finished(false), "sigue sonando");
    assert!(!p.finished(true), "el decoder llegó al final pero queda buffer por vaciar");
    p.ended_at = Some(Instant::now() - MEDITATION_DRAIN);
    assert!(p.finished(false), "buffer vacío: terminó entera");
}

#[test]
fn a_meditation_far_from_its_end_does_not_finish_by_itself() {
    // Cortada, reemplazada o borrada: el player la saca sin pasar por `finished`.
    let mut p = playback(600);
    p.played = Duration::from_secs(595);
    assert!(!p.finished(false));
    // Si el decoder nunca avisa, el reloj la da por terminada pasado el final.
    let mut p = playback(1);
    p.since = Some(Instant::now() - Duration::from_secs(7));
    assert!(p.finished(false));
}

#[test]
fn a_paused_timer_does_not_count_down_and_resumes_from_what_was_left() {
    let mut t = Timer::new(Duration::from_secs(600));
    t.end = Instant::now() + Duration::from_secs(100);
    t.toggle_pause();
    assert!(t.paused());
    // Pasó el tiempo "real": si el reloj siguiera, ya estaría en cero.
    t.end = Instant::now() - Duration::from_secs(50);
    assert!((99..=100).contains(&t.left().as_secs()));
    t.toggle_pause();
    assert!(!t.paused());
    assert!((99..=100).contains(&t.left().as_secs()), "sigue desde lo que quedaba");
}

#[test]
fn a_paused_timer_is_reported_as_paused_and_a_new_one_is_not() {
    let mut t = Timer::new(Duration::from_secs(60));
    t.toggle_pause();
    let json =
        serde_json::to_string(&TimerState { total_secs: 60, remaining_secs: t.left().as_secs(), paused: t.paused() })
            .unwrap();
    assert!(json.contains("\"paused\":true"));
    // Cancelar y volver a arrancar crea un Timer nuevo: sin pausa.
    assert!(!Timer::new(Duration::from_secs(60)).paused());
}

#[test]
fn timer_state_from_an_older_daemon_parses_as_running() {
    let t: TimerState = serde_json::from_str(r#"{"total_secs":60,"remaining_secs":30}"#).unwrap();
    assert!(!t.paused);
    let c: Command = serde_json::from_str(r#"{"cmd":"restart_sounds"}"#).unwrap();
    assert!(matches!(c, Command::RestartSounds));
    let back = serde_json::to_string(&Command::RestartSounds).unwrap();
    assert_eq!(back, r#"{"cmd":"restart_sounds"}"#);
    let c: Command = serde_json::from_str(r#"{"cmd":"toggle_timer_pause"}"#).unwrap();
    assert!(matches!(c, Command::ToggleTimerPause));
}

#[test]
fn state_without_the_finished_signal_still_parses() {
    let json = r#"{"sounds":[],"presets":[],"volumes":{},"active":[],"paused":false,"master":1.0,"timer":null,"downloads":[]}"#;
    let s: State = serde_json::from_str(json).unwrap();
    assert_eq!((s.meditations_finished, s.last_meditation_finished), (0, None));
}

#[test]
fn meditation_pauses_the_ambient_and_gives_it_back() {
    let (mut hold, mut paused) = (AmbientHold::default(), false);
    assert!(hold.update(true, true, &mut paused) && paused, "al empezar se pausa");
    assert!(!hold.update(true, true, &mut paused) && paused, "mientras suena sigue pausado");
    assert!(hold.update(false, true, &mut paused) && !paused, "al terminar vuelve");
    assert!(!hold.update(false, true, &mut paused) && !paused);
}

#[test]
fn pausing_the_meditation_resumes_the_ambient_and_resuming_pauses_it_again() {
    let (mut hold, mut paused) = (AmbientHold::default(), false);
    hold.update(true, true, &mut paused);
    hold.update(false, true, &mut paused); // meditación en pausa
    assert!(!paused);
    hold.update(true, true, &mut paused); // reanuda
    assert!(paused);
}

#[test]
fn ambient_that_was_already_paused_or_absent_is_left_alone() {
    let (mut hold, mut paused) = (AmbientHold::default(), true);
    assert!(!hold.update(true, true, &mut paused) && paused);
    assert!(!hold.update(false, true, &mut paused) && paused, "no lo reanuda: no lo pausó él");

    let (mut hold, mut paused) = (AmbientHold::default(), false);
    assert!(!hold.update(true, false, &mut paused) && !paused, "sin ambiente no hay nada que pausar");
}

#[test]
fn ambient_resumed_by_hand_during_the_meditation_stays_on() {
    let (mut hold, mut paused) = (AmbientHold::default(), false);
    hold.update(true, true, &mut paused);
    paused = false; // el usuario lo reanuda
    assert!(!hold.update(true, true, &mut paused) && !paused);
    assert!(!hold.update(false, true, &mut paused) && !paused);
}
