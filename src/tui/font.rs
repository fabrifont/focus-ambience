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

/// Rows of the rendered text, one space between glyphs, every pixel (half a cell) drawn `scale` times
/// wide and tall so the shape and symmetry are exactly those of the 3-row font.
/// Each entry is (column offset, row strings); the text is `HEIGHT * scale` rows tall.
pub fn render(text: &str, scale: usize) -> (usize, Vec<(usize, Vec<String>)>) {
    let mut x = 0;
    let mut out = Vec::new();
    for c in text.chars() {
        let g = glyph(c);
        let w = g[0].chars().count();
        out.push((x, scale_glyph(&g, scale)));
        x += (w + 1) * scale;
    }
    (x.saturating_sub(scale), out)
}

/// Splits each cell into its two half-pixels, repeats them `scale` times in both directions
/// and packs them back into half blocks.
fn scale_glyph(g: &[&str; HEIGHT], scale: usize) -> Vec<String> {
    let pixels: Vec<Vec<bool>> = g
        .iter()
        .flat_map(|row| {
            let top = row.chars().map(|c| matches!(c, '█' | '▀')).collect::<Vec<_>>();
            let bottom = row.chars().map(|c| matches!(c, '█' | '▄')).collect::<Vec<_>>();
            [top, bottom]
        })
        .flat_map(|row| {
            let wide: Vec<bool> = row.iter().flat_map(|&p| std::iter::repeat_n(p, scale)).collect();
            std::iter::repeat_n(wide, scale)
        })
        .collect();
    pixels
        .chunks(2)
        .map(|pair| {
            pair[0]
                .iter()
                .zip(&pair[1])
                .map(|(&t, &b)| match (t, b) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_one_is_the_original_glyph() {
        for c in "0123456789:FOCUS".chars() {
            assert_eq!(scale_glyph(&glyph(c), 1), glyph(c).to_vec(), "{c}");
        }
    }

    #[test]
    fn scaling_keeps_the_shape_and_grows_uniformly() {
        // El 0 a escala 2: 6 filas de 6 columnas (la última queda vacía, como la mitad de abajo del original).
        let g = scale_glyph(&glyph('0'), 2);
        assert_eq!(g, vec!["██████", "██  ██", "██  ██", "██  ██", "██████", "      "]);
    }

    #[test]
    fn render_width_and_height_follow_the_scale() {
        let (w, glyphs) = render("1:00", 3);
        assert_eq!(w, (4 + 2 + 4 + 4) * 3 - 3);
        assert!(glyphs.iter().all(|(_, rows)| rows.len() == HEIGHT * 3));
    }
}
