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

/// Height of the big font used by deep work mode.
pub const BIG_HEIGHT: usize = 5;

pub fn big_glyph(c: char) -> [&'static str; BIG_HEIGHT] {
    match c {
        '0' | 'O' => ["█████", "█   █", "█   █", "█   █", "█████"],
        '1' => ["  █  ", " ██  ", "  █  ", "  █  ", " ███ "],
        '2' => ["█████", "    █", "█████", "█    ", "█████"],
        '3' => ["█████", "    █", "█████", "    █", "█████"],
        '4' => ["█   █", "█   █", "█████", "    █", "    █"],
        '5' | 'S' => ["█████", "█    ", "█████", "    █", "█████"],
        '6' => ["█████", "█    ", "█████", "█   █", "█████"],
        '7' => ["█████", "    █", "   █ ", "  █  ", "  █  "],
        '8' => ["█████", "█   █", "█████", "█   █", "█████"],
        '9' => ["█████", "█   █", "█████", "    █", "█████"],
        ':' => [" ", "█", " ", "█", " "],
        'F' => ["█████", "█    ", "████ ", "█    ", "█    "],
        'C' => ["█████", "█    ", "█    ", "█    ", "█████"],
        'U' => ["█   █", "█   █", "█   █", "█   █", "█████"],
        _ => ["     "; BIG_HEIGHT],
    }
}

/// Rows of the rendered text, one space between glyphs. Each entry is (column offset, row strings).
/// `big` picks the 5-row font instead of the 3-row one.
pub fn render(text: &str, big: bool) -> (usize, Vec<(usize, Vec<&'static str>)>) {
    let mut x = 0;
    let mut out = Vec::new();
    for c in text.chars() {
        let g: Vec<&'static str> = if big { big_glyph(c).to_vec() } else { glyph(c).to_vec() };
        let w = g[0].chars().count();
        out.push((x, g));
        x += w + 1;
    }
    (x.saturating_sub(1), out)
}
