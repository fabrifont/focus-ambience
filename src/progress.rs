//! How the long jobs (`setup`, `sync`, `verify`) say what they are doing: straight to the
//! terminal in the CLI, or update by update to a callback when the TUI runs them (anything
//! printed there would tear the screen). The code that does the work is the same in both.

use std::io::{self, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;

/// One thing worth showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    /// A finished line.
    Line(String),
    /// A status that the next `Live` replaces (rclone's transfer counter).
    Live(String),
}

pub struct Out<'a>(Option<&'a mut dyn FnMut(Update)>);

impl<'a> Out<'a> {
    pub fn console() -> Self {
        Out(None)
    }

    pub fn sink(f: &'a mut dyn FnMut(Update)) -> Self {
        Out(Some(f))
    }

    pub fn is_console(&self) -> bool {
        self.0.is_none()
    }

    pub fn line(&mut self, text: impl Into<String>) {
        match &mut self.0 {
            None => println!("{}", text.into()),
            Some(f) => f(Update::Line(text.into())),
        }
    }

    /// Runs `cmd` to the end. In the console it shares the terminal; in a sink its output comes
    /// back as updates and it never reads the keyboard.
    pub fn run(&mut self, cmd: &mut Command) -> io::Result<ExitStatus> {
        let Some(f) = &mut self.0 else { return cmd.status() };
        let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let (tx, rx) = mpsc::channel();
        if let Some(r) = child.stdout.take() {
            pump(r, tx.clone());
        }
        if let Some(r) = child.stderr.take() {
            pump(r, tx.clone());
        }
        drop(tx);
        // Ends when the child (and any grandchild) closes both pipes.
        for update in rx {
            f(update);
        }
        child.wait()
    }
}

fn pump(mut reader: impl Read + Send + 'static, tx: mpsc::Sender<Update>) {
    thread::spawn(move || {
        let mut split = Splitter::default();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            for &b in &chunk[..n] {
                split.push(b, &mut |u| {
                    let _ = tx.send(u);
                });
            }
        }
        split.finish(&mut |u| {
            let _ = tx.send(u);
        });
    });
}

/// Cuts a byte stream into updates: `\n` ends a line, `\r` ends a status that the next text
/// replaces (`\r\n` is a plain line end).
#[derive(Default)]
struct Splitter {
    buf: Vec<u8>,
    /// Text that ended in `\r`: a `Live` unless a `\n` comes right after.
    pending: Option<String>,
}

impl Splitter {
    fn push(&mut self, b: u8, out: &mut impl FnMut(Update)) {
        match b {
            b'\r' => {
                if !self.buf.is_empty() {
                    if let Some(p) = self.pending.take() {
                        out(Update::Live(p));
                    }
                    self.pending = Some(clean(&std::mem::take(&mut self.buf)));
                }
            }
            b'\n' => {
                let text = if self.buf.is_empty() { self.pending.take() } else { Some(clean(&self.buf)) };
                self.buf.clear();
                self.pending = None;
                if let Some(t) = text.filter(|t| !t.is_empty()) {
                    out(Update::Line(t));
                }
            }
            _ => {
                if let Some(p) = self.pending.take() {
                    out(Update::Live(p));
                }
                self.buf.push(b);
            }
        }
    }

    fn finish(&mut self, out: &mut impl FnMut(Update)) {
        let text = if self.buf.is_empty() { self.pending.take() } else { Some(clean(&self.buf)) };
        self.buf.clear();
        self.pending = None;
        if let Some(t) = text.filter(|t| !t.is_empty()) {
            out(Update::Line(t));
        }
    }
}

/// The text without terminal escape sequences (rclone colors and moves the cursor), trimmed.
fn clean(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            if !c.is_control() || c == '\t' {
                out.push(c);
            }
            continue;
        }
        if chars.next_if_eq(&'[').is_some() {
            // CSI: parameters, then one final byte in @..~
            for n in chars.by_ref() {
                if ('@'..='~').contains(&n) {
                    break;
                }
            }
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(input: &[u8]) -> Vec<Update> {
        let mut s = Splitter::default();
        let mut got = Vec::new();
        for &b in input {
            s.push(b, &mut |u| got.push(u));
        }
        s.finish(&mut |u| got.push(u));
        got
    }

    fn line(s: &str) -> Update {
        Update::Line(s.into())
    }

    fn live(s: &str) -> Update {
        Update::Live(s.into())
    }

    #[test]
    fn lines_end_at_newlines_and_empty_ones_are_dropped() {
        assert_eq!(split(b"uno\n\ndos\r\ntres"), [line("uno"), line("dos"), line("tres")]);
    }

    #[test]
    fn carriage_returns_make_a_status_that_gets_replaced() {
        assert_eq!(split(b"1 MiB\r2 MiB\r3 MiB\n"), [live("1 MiB"), live("2 MiB"), line("3 MiB")]);
        assert_eq!(split(b"1 MiB\r2 MiB"), [live("1 MiB"), line("2 MiB")]);
    }

    #[test]
    fn escape_sequences_are_removed() {
        assert_eq!(split(b"\x1b[2K\x1b[1;32mlisto\x1b[0m\n"), [line("listo")]);
        assert!(split(b"\x1b[2K\x1b[A\n").is_empty());
    }

    #[test]
    fn utf8_split_across_chunks_survives() {
        assert_eq!(split("✓ bajé ñandú\n".as_bytes()), [line("✓ bajé ñandú")]);
    }

    #[cfg(unix)]
    #[test]
    fn a_sink_receives_the_output_of_a_command() {
        let mut got = Vec::new();
        let mut f = |u| got.push(u);
        let status =
            Out::sink(&mut f).run(Command::new("sh").args(["-c", "echo hola; echo chau >&2; exit 3"])).unwrap();
        assert_eq!(status.code(), Some(3));
        got.sort_by_key(|u| format!("{u:?}"));
        assert_eq!(got, [line("chau"), line("hola")]);
    }
}
