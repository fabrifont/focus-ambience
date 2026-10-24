//! A tiny 3-row block font for the clock and the wordmark.

pub const HEIGHT: usize = 3;

pub fn glyph(c: char) -> [&'static str; HEIGHT] {
    match c {
        '0' => ["█▀█", "█ █", "▀▀▀"],
        '1' => ["▀█ ", " █ ", "▀▀▀"],
        '2' => ["▀▀█", "█▀▀", "▀▀▀"],
        '3' => ["▀▀█", " ▀█", "▀▀▀"],
        '4' => ["█ █", "▀▀█", "  ▀"],
        '5' => ["█▀▀", "▀▀█", "▀▀▀"],
        '6' => ["█▀▀", "█▀█", "▀▀▀"],
        '7' => ["▀▀█", "  █", "  ▀"],
        '8' => ["█▀█", "█▀█", "▀▀▀"],
        '9' => ["█▀█", "▀▀█", "▀▀▀"],
        ':' => [" ", "▀", "▀"],
        'F' => ["█▀▀", "█▀ ", "▀  "],
        'O' => ["█▀█", "█ █", "▀▀▀"],
        'C' => ["█▀▀", "█  ", "▀▀▀"],
        'U' => ["█ █", "█ █", "▀▀▀"],
        'S' => ["█▀▀", "▀▀█", "▀▀▀"],
        _ => ["   ", "   ", "   "],
    }
}

/// Rows of the rendered text, one space between glyphs. Each entry is (column offset, row strings).
pub fn render(text: &str) -> (usize, Vec<(usize, [&'static str; HEIGHT])>) {
    let mut x = 0;
    let mut out = Vec::new();
    for c in text.chars() {
        let g = glyph(c);
        let w = g[0].chars().count();
        out.push((x, g));
        x += w + 1;
    }
    (x.saturating_sub(1), out)
}
