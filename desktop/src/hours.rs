//! Horas trabajadas (#28): cada tramo con el timer corriendo en modo trabajo es una sesión que se
//! guarda en `horas.sqlite` (carpeta de datos local, sin red). Por sesión se guarda solo inicio,
//! fin, minutos trabajados y nombre de la compu. El daemon observa el timer una vez por segundo
//! (`Tracker`) y la TUI lee el archivo en solo lectura para el resumen (`load_summary`).

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use rusqlite::{Connection, OpenFlags, params};

use crate::paths;

/// Cada cuánto se escribe la sesión abierta, para no perder más que esto si el daemon muere.
const FLUSH_EVERY: f64 = 30.0;

/// Cómo está el timer en un instante (lo que el daemon ve en `State`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerObs {
    pub total: u64,
    pub remaining: u64,
    pub paused: bool,
    pub rest: bool,
}

/// La base de sesiones.
pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("no pude crear {}", dir.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("no pude abrir {}", path.display()))?;
        Store::init(conn)
    }

    #[cfg(test)]
    fn memory() -> Store {
        Store::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id         INTEGER PRIMARY KEY,
                started_at INTEGER NOT NULL,
                ended_at   INTEGER NOT NULL,
                minutes    REAL NOT NULL,
                host       TEXT NOT NULL
            );",
        )?;
        Ok(Store { conn })
    }

    /// Inserta la sesión (`id` en `None`) o actualiza la ya insertada; devuelve su id.
    fn save(&self, id: Option<i64>, start: i64, end: i64, minutes: f64, host: &str) -> Result<i64> {
        match id {
            Some(id) => {
                self.conn.execute(
                    "UPDATE sessions SET ended_at = ?1, minutes = ?2 WHERE id = ?3",
                    params![end, minutes, id],
                )?;
                Ok(id)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO sessions (started_at, ended_at, minutes, host) VALUES (?1, ?2, ?3, ?4)",
                    params![start, end, minutes, host],
                )?;
                Ok(self.conn.last_insert_rowid())
            }
        }
    }
}

/// Nombre de la compu: `COMPUTERNAME` (Windows), `HOSTNAME` o `/etc/hostname` (Linux).
pub fn hostname() -> String {
    let from_env = |k| std::env::var(k).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    from_env("COMPUTERNAME")
        .or_else(|| from_env("HOSTNAME"))
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "desconocido".to_string())
}

struct Open {
    id: Option<i64>,
    start: i64,
    /// Segundos trabajados hasta ahora.
    worked: f64,
    last_flush: f64,
}

struct Prev {
    at: f64,
    counting: bool,
    total: u64,
    remaining: u64,
}

/// Convierte lo que se ve del timer en sesiones. Una sesión es un tramo con el timer corriendo en
/// modo trabajo: la pausa no suma (pero no la cierra); cambiar a rest, cancelar, terminar o salir
/// la cierran, y volver a trabajo abre una nueva.
pub struct Tracker {
    store: Store,
    host: String,
    open: Option<Open>,
    prev: Option<Prev>,
}

impl Tracker {
    pub fn new(store: Store, host: String) -> Tracker {
        Tracker { store, host, open: None, prev: None }
    }

    /// `mono`: segundos de un reloj que no retrocede; `unix`: segundos desde 1970 (UTC);
    /// `timer`: el timer ahora, `None` si no hay.
    pub fn observe(&mut self, mono: f64, unix: i64, timer: Option<TimerObs>) {
        let replaced = match (&self.prev, &timer) {
            (Some(p), Some(t)) => t.total != p.total || t.remaining > p.remaining + 1,
            _ => false,
        };
        let continues = timer.is_some() && self.prev.is_some() && !replaced;

        // Lo trabajado desde la vez anterior; si el timer se acabó o cambió, no más de lo que le quedaba.
        if let (Some(p), Some(open)) = (&self.prev, &mut self.open)
            && p.counting
        {
            let dt = (mono - p.at).max(0.0);
            open.worked += if continues { dt } else { dt.min(p.remaining as f64) };
        }

        let working = timer.is_some_and(|t| !t.rest);
        if self.open.is_some() && !(continues && working) {
            self.close(unix);
        }
        if working && self.open.is_none() {
            self.open = Some(Open { id: None, start: unix, worked: 0.0, last_flush: mono });
        }
        if let Some(open) = &mut self.open
            && mono - open.last_flush >= FLUSH_EVERY
        {
            open.last_flush = mono;
            if open.worked > 0.0
                && let Ok(id) = self.store.save(open.id, open.start, unix, open.worked / 60.0, &self.host)
            {
                open.id = Some(id);
            }
        }

        self.prev =
            timer.map(|t| Prev { at: mono, counting: !t.paused && !t.rest, total: t.total, remaining: t.remaining });
    }

    /// Cierra la sesión abierta (el daemon se va).
    pub fn finish(&mut self, mono: f64, unix: i64) {
        self.observe(mono, unix, None);
    }

    /// Guarda lo trabajado de la sesión abierta; con cero trabajado no guarda nada.
    fn close(&mut self, unix: i64) {
        if let Some(open) = self.open.take()
            && open.worked > 0.0
        {
            let _ = self.store.save(open.id, open.start, unix, open.worked / 60.0, &self.host);
        }
    }
}

/// Minutos trabajados por período (las sesiones cuentan en el día en que empezaron).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    pub today: f64,
    pub week: f64,
    pub month: f64,
    pub total: f64,
}

/// Medianoche local de `day` como segundos unix (si ese día no tiene medianoche, la primera hora que sí).
fn midnight<Tz: TimeZone>(tz: &Tz, day: NaiveDate) -> i64 {
    let start = day.and_hms_opt(0, 0, 0).unwrap_or_default();
    (0..24)
        .find_map(|h| tz.from_local_datetime(&(start + Duration::hours(h))).earliest())
        .map_or_else(|| start.and_utc().timestamp(), |t| t.timestamp())
}

/// Inicio (unix) de hoy, de la semana (lunes) y del mes, en la hora local de `now`.
fn bounds<Tz: TimeZone>(now: &chrono::DateTime<Tz>) -> [i64; 3] {
    let today = now.date_naive();
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let first = today.with_day(1).unwrap_or(today);
    let tz = now.timezone();
    [midnight(&tz, today), midnight(&tz, monday), midnight(&tz, first)]
}

fn summarize<Tz: TimeZone>(conn: &Connection, now: &chrono::DateTime<Tz>) -> Result<Summary> {
    let since = |from: i64| -> Result<f64> {
        Ok(conn.query_row(
            "SELECT COALESCE(SUM(minutes), 0.0) FROM sessions WHERE started_at >= ?1",
            params![from],
            |r| r.get(0),
        )?)
    };
    let [today, week, month] = bounds(now);
    Ok(Summary { today: since(today)?, week: since(week)?, month: since(month)?, total: since(i64::MIN)? })
}

/// El resumen de ahora leyendo el archivo sin escribirlo; sin archivo (o sin tabla) es todo cero.
pub fn load_summary() -> Summary {
    let path = paths::hours_file();
    let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return Summary::default();
    };
    let _ = conn.busy_timeout(std::time::Duration::from_secs(1));
    summarize(&conn, &Local::now()).unwrap_or_default()
}

/// «2 h 35 min»; menos de una hora, solo «35 min».
pub fn format_minutes(minutes: f64) -> String {
    let total = minutes.max(0.0).round() as u64;
    match (total / 60, total % 60) {
        (0, m) => format!("{m} min"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    fn tracker() -> Tracker {
        Tracker::new(Store::memory(), "compu".into())
    }

    fn obs(remaining: u64, paused: bool, rest: bool) -> Option<TimerObs> {
        Some(TimerObs { total: 3000, remaining, paused, rest })
    }

    fn rows(t: &Tracker) -> Vec<(i64, i64, f64)> {
        let mut st = t.store.conn.prepare("SELECT started_at, ended_at, minutes FROM sessions ORDER BY id").unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    }

    #[test]
    fn pause_does_not_count() {
        let mut t = tracker();
        t.observe(0.0, 1000, obs(3000, false, false));
        t.observe(60.0, 1060, obs(2940, false, false)); // 60 s trabajados
        t.observe(61.0, 1061, obs(2940, true, false)); // se pausa
        t.observe(121.0, 1121, obs(2940, true, false)); // 60 s en pausa
        t.observe(122.0, 1122, obs(2940, false, false)); // sigue
        t.observe(182.0, 1182, None); // cancelado: 60 s más
        let r = rows(&t);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].0, r[0].1), (1000, 1182));
        assert!((r[0].2 - 121.0 / 60.0).abs() < 1e-9, "{r:?}");
    }

    #[test]
    fn rest_closes_and_work_opens_a_new_session() {
        let mut t = tracker();
        t.observe(0.0, 1000, obs(3000, false, false));
        t.observe(120.0, 1120, obs(2880, false, false));
        t.observe(121.0, 1121, obs(2879, false, true)); // a rest: cierra
        t.observe(400.0, 1400, obs(2600, false, true)); // descanso: no se guarda
        t.observe(401.0, 1401, obs(2599, false, false)); // vuelve a trabajo: sesión nueva
        t.observe(461.0, 1461, obs(2539, false, false));
        t.finish(462.0, 1462);
        let r = rows(&t);
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!((r[0].0, r[0].1), (1000, 1121));
        assert!((r[0].2 - 121.0 / 60.0).abs() < 1e-9); // el último segundo todavía era trabajo
        assert_eq!((r[1].0, r[1].1), (1401, 1462));
        assert!((r[1].2 - 61.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn cancel_saves_partial_and_zero_is_dropped() {
        let mut t = tracker();
        t.observe(0.0, 1000, obs(3000, false, false));
        t.observe(30.0, 1030, None);
        assert_eq!(rows(&t).len(), 1);
        // Arranca y se cancela sin que corra nada, y uno en pausa desde el principio.
        t.observe(100.0, 1100, obs(3000, false, false));
        t.observe(100.0, 1100, None);
        t.observe(200.0, 1200, obs(3000, true, false));
        t.observe(300.0, 1300, None);
        assert_eq!(rows(&t).len(), 1);
    }

    #[test]
    fn natural_end_does_not_overcount() {
        let mut t = tracker();
        t.observe(0.0, 1000, obs(2, false, false));
        t.observe(5.0, 1005, None); // se acabó a los 2 s, el reloj lo vio 3 s tarde
        let r = rows(&t);
        assert!((r[0].2 - 2.0 / 60.0).abs() < 1e-9, "{r:?}");
    }

    #[test]
    fn new_timer_replaces_the_session_and_starts_in_work() {
        let mut t = tracker();
        t.observe(0.0, 1000, obs(3000, false, false));
        t.observe(60.0, 1060, obs(2940, false, true)); // rest
        t.observe(100.0, 1100, obs(2900, false, true));
        // El timer nuevo llega ya en trabajo (el core lo reinicia).
        t.observe(101.0, 1101, Some(TimerObs { total: 600, remaining: 600, paused: false, rest: false }));
        t.observe(161.0, 1161, Some(TimerObs { total: 600, remaining: 540, paused: false, rest: false }));
        t.finish(162.0, 1162);
        let r = rows(&t);
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(r[1].0, 1101);
        assert!((r[1].2 - 61.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn open_session_is_flushed_periodically() {
        let mut t = tracker();
        t.observe(0.0, 1000, obs(3000, false, false));
        for s in 1..=31 {
            t.observe(s as f64, 1000 + s, obs(3000 - s as u64, false, false));
        }
        // Sin cerrar (el daemon murió): ya hay una fila con lo trabajado hasta el volcado.
        let r = rows(&t);
        assert_eq!(r.len(), 1);
        assert!(r[0].2 > 29.0 / 60.0, "{r:?}");
        t.observe(40.0, 1040, None);
        let r = rows(&t);
        assert_eq!(r.len(), 1);
        assert!((r[0].2 - 40.0 / 60.0).abs() < 1e-9);
    }

    fn at(offset_h: i32, y: i32, m: u32, d: u32, h: u32) -> chrono::DateTime<FixedOffset> {
        FixedOffset::east_opt(offset_h * 3600).unwrap().with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    fn insert(store: &Store, start: chrono::DateTime<FixedOffset>, minutes: f64) {
        store.save(None, start.timestamp(), start.timestamp() + (minutes * 60.0) as i64, minutes, "h").unwrap();
    }

    #[test]
    fn summary_uses_local_days_and_monday_weeks() {
        let s = Store::memory();
        // 2026-11-18 es miércoles; zona UTC-3.
        let now = at(-3, 2026, 11, 18, 15);
        insert(&s, at(-3, 2026, 11, 18, 9), 60.0); // hoy
        insert(&s, at(-3, 2026, 11, 17, 23), 30.0); // martes, esta semana
        insert(&s, at(-3, 2026, 11, 16, 0), 20.0); // lunes 00:00 local: esta semana
        insert(&s, at(-3, 2026, 11, 15, 23), 10.0); // domingo anterior: este mes, no esta semana
        insert(&s, at(-3, 2026, 10, 31, 23), 5.0); // mes anterior
        let r = summarize(&s.conn, &now).unwrap();
        assert_eq!(r, Summary { today: 60.0, week: 110.0, month: 120.0, total: 125.0 });
    }

    #[test]
    fn midnight_crossing_session_counts_on_start_day() {
        let s = Store::memory();
        let now = at(-3, 2026, 11, 18, 10);
        // Empezó el 17 a las 23:30 y duró 90 min: todo para el 17, nada para hoy.
        insert(&s, at(-3, 2026, 11, 17, 23), 90.0);
        let r = summarize(&s.conn, &now).unwrap();
        assert_eq!((r.today, r.week, r.total), (0.0, 90.0, 90.0));
    }

    #[test]
    fn local_day_differs_from_utc_day() {
        let s = Store::memory();
        // 01:00 UTC del 18 es todavía el 17 en UTC-3.
        insert(&s, at(0, 2026, 11, 18, 1), 40.0);
        let r = summarize(&s.conn, &at(-3, 2026, 11, 18, 12)).unwrap();
        assert_eq!(r.today, 0.0);
        assert_eq!(r.week, 40.0);
    }

    #[test]
    fn empty_summary_is_zero_and_format() {
        let r = summarize(&Store::memory().conn, &at(0, 2026, 11, 18, 12)).unwrap();
        assert_eq!(r, Summary::default());
        assert_eq!(format_minutes(0.0), "0 min");
        assert_eq!(format_minutes(35.4), "35 min");
        assert_eq!(format_minutes(155.0), "2 h 35 min");
        assert_eq!(format_minutes(120.0), "2 h 0 min");
    }
}
