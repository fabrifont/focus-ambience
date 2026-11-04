//! Configuración > Sincronización: las tres tareas de Drive (las mismas de `focus setup`,
//! `focus sync` y `focus verify`) corriendo en un hilo, con una ventana que muestra lo que van
//! diciendo. Mientras corre la ventana bloquea todas las teclas.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::app::{App, Screen};
use super::menu::{draw_box_w, row};
use super::theme::Palette;
use crate::progress::{Out, Update};
use crate::protocol::Command;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncKind {
    /// `focus setup`.
    Setup,
    /// `focus sync` (sube y baja).
    Sync,
    /// `focus verify`.
    Verify,
}

pub const SYNC_ITEMS: [SyncKind; 3] = [SyncKind::Setup, SyncKind::Sync, SyncKind::Verify];

impl SyncKind {
    pub fn label(self) -> &'static str {
        match self {
            SyncKind::Setup => "Conectar Google Drive",
            SyncKind::Sync => "Sincronizar",
            SyncKind::Verify => "Verificar archivos",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SyncKind::Setup => "Instala rclone si falta y abre el navegador para entrar a tu cuenta",
            SyncKind::Sync => "Sube y baja sonidos, meditaciones y listas; gana lo más nuevo",
            SyncKind::Verify => "Revisa que los audios coincidan con las listas",
        }
    }

    fn title(self) -> &'static str {
        match self {
            SyncKind::Setup => " conectar Google Drive ",
            SyncKind::Sync => " sincronizar ",
            SyncKind::Verify => " verificar archivos ",
        }
    }
}

/// Lo que hace de verdad cada tarea; los tests lo reemplazan para no tocar Drive ni rclone.
pub type Runner = Arc<dyn Fn(SyncKind, &mut Out) -> anyhow::Result<()> + Send + Sync>;

pub fn real_runner() -> Runner {
    Arc::new(|kind, out| match kind {
        SyncKind::Setup => crate::setup::run(out),
        SyncKind::Sync => crate::sync::run(crate::sync::Direction::Both, out),
        SyncKind::Verify => crate::sync::verify(out),
    })
}

enum Msg {
    Update(Update),
    Done(Result<(), String>),
}

/// Líneas que guarda la ventana (las más viejas se tiran).
const KEEP_LINES: usize = 200;

/// Una tarea en curso o terminada, con lo que fue contando.
pub struct SyncJob {
    pub kind: SyncKind,
    pub lines: Vec<String>,
    /// Estado que la próxima actualización pisa (el contador de rclone).
    pub live: Option<String>,
    /// `None` mientras corre; después, el resultado.
    pub result: Option<Result<(), String>>,
    rx: Receiver<Msg>,
}

impl SyncJob {
    pub fn running(&self) -> bool {
        self.result.is_none()
    }
}

impl App {
    pub fn start_sync(&mut self, kind: SyncKind) {
        let (tx, rx) = mpsc::channel();
        let runner = Arc::clone(&self.sync_runner);
        thread::spawn(move || {
            let mut report = |u| {
                let _ = tx.send(Msg::Update(u));
            };
            let result = runner(kind, &mut Out::sink(&mut report));
            let _ = tx.send(Msg::Done(result.map_err(|e| format!("{e:#}"))));
        });
        self.sync = Some(SyncJob { kind, lines: Vec::new(), live: None, result: None, rx });
    }

    /// Trae lo que el hilo fue contando. Cada cuadro llama a esto.
    pub fn poll_sync(&mut self) {
        let Some(job) = &mut self.sync else { return };
        if !job.running() {
            return;
        }
        loop {
            match job.rx.try_recv() {
                Ok(Msg::Update(Update::Line(l))) => {
                    job.live = None;
                    job.lines.push(l);
                    if job.lines.len() > KEEP_LINES {
                        job.lines.remove(0);
                    }
                }
                Ok(Msg::Update(Update::Live(l))) => job.live = Some(l),
                Ok(Msg::Done(r)) => job.result = Some(r),
                // The thread ended without saying how (it panicked).
                Err(TryRecvError::Disconnected) => {
                    job.result = Some(Err("se cortó de golpe; mirá el log".to_string()));
                }
                Err(TryRecvError::Empty) => break,
            }
            if !job.running() {
                break;
            }
        }
        if let Some(Ok(())) = job.result {
            // Lo bajado cambia las listas: el daemon las relee y las rutinas se vuelven a leer.
            self.routines_stale = true;
            self.send(Command::Reload);
        }
    }

    /// Hay una tarea corriendo (la ventana no deja hacer nada más).
    pub fn sync_running(&self) -> bool {
        self.sync.as_ref().is_some_and(SyncJob::running)
    }

    /// Teclas de la ventana: mientras corre no hace nada; al terminar, Enter (o ␣) y Esc cierran.
    pub(super) fn sync_job_key(&mut self, k: KeyEvent) {
        if self.sync_running() {
            return;
        }
        if matches!(k.code, KeyCode::Enter | KeyCode::Esc) {
            self.sync = None;
        }
    }

    pub(super) fn sync_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up => self.sync_cursor = self.sync_cursor.saturating_sub(1),
            KeyCode::Down => self.sync_cursor = (self.sync_cursor + 1).min(SYNC_ITEMS.len() - 1),
            KeyCode::Enter => self.start_sync(SYNC_ITEMS[self.sync_cursor]),
            KeyCode::Esc => self.go(Screen::Config),
            _ => {}
        }
    }
}

/// Las opciones, cada una con su descripción abajo.
pub fn lines(app: &App, pal: &Palette) -> Vec<Line<'static>> {
    let dim = Style::new().fg(pal.dim.c());
    let mut v = Vec::new();
    for (i, kind) in SYNC_ITEMS.iter().enumerate() {
        if i > 0 {
            v.push(Line::raw(""));
        }
        v.push(row(i == app.sync_cursor, String::new(), kind.label(), pal));
        v.push(Line::styled(format!("    {}", kind.description()), dim));
    }
    v
}

const BOX_WIDTH: u16 = 64;

/// Hard-wraps `text` to `width` columns (links have no spaces to break at).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(width.max(1)).map(|c| c.iter().collect()).collect()
}

pub fn draw_popup(f: &mut Frame, app: &App, pal: &Palette) {
    let Some(job) = &app.sync else { return };
    let dim = Style::new().fg(pal.dim.c());
    let fg = Style::new().fg(pal.fg.c());
    let acc = Style::new().fg(pal.accent.c());
    let area = f.area();
    let inner = (BOX_WIDTH.min(area.width.saturating_sub(2)) as usize).saturating_sub(4);
    // Output rows: as many as fit, up to 8, so the box does not jump while lines arrive.
    let rows = (area.height.saturating_sub(9) as usize).clamp(3, 8);
    let mut wrapped: Vec<String> = job.lines.iter().chain(job.live.iter()).flat_map(|l| wrap(l, inner)).collect();
    let skip = wrapped.len().saturating_sub(rows);
    wrapped.drain(..skip);
    let mut out: Vec<Line> = wrapped.into_iter().map(|l| Line::styled(format!("  {l}"), fg)).collect();
    out.resize(rows, Line::raw(""));

    let mut lines = vec![Line::raw("")];
    lines.extend(out);
    lines.push(Line::raw(""));
    match &job.result {
        None => {
            const SPIN: [&str; 8] = ["⠋", "⠙", "⠸", "⠴", "⠦", "⠇", "⠏", "⠋"];
            let frame = (app.started.elapsed().as_millis() / 120) as usize % SPIN.len();
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", SPIN[frame]), acc),
                Span::styled("Trabajando… esperá a que termine", fg),
            ]));
            lines.push(Line::styled(
                if job.kind == SyncKind::Setup {
                    "  Terminá el login en el navegador; si no se abre, usá el link."
                } else {
                    "  Hasta que termine no se puede cerrar."
                },
                dim,
            ));
        }
        Some(Ok(())) => {
            lines.push(Line::styled("  ✓ Listo", acc.add_modifier(Modifier::BOLD)));
            lines.push(close_hint(acc, dim));
        }
        Some(Err(e)) => {
            let err = Style::new().fg(pal.accent2.c());
            for (i, l) in wrap(&format!("Falló: {e}"), inner.saturating_sub(2)).into_iter().take(3).enumerate() {
                lines.push(Line::styled(format!("  {}{l}", if i == 0 { "✗ " } else { "  " }), err));
            }
            lines.push(close_hint(acc, dim));
        }
    }
    draw_box_w(f, pal, job.kind.title(), BOX_WIDTH, lines);
}

fn close_hint(acc: Style, dim: Style) -> Line<'static> {
    Line::from(vec![Span::styled("  Enter/␣/Esc ", acc), Span::styled("cerrar", dim)])
}
