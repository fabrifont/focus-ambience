use ratatui::style::Color;

use crate::library::Theme;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    pub const fn hex(h: u32) -> Self {
        Rgb(((h >> 16) & 0xff) as f32, ((h >> 8) & 0xff) as f32, (h & 0xff) as f32)
    }

    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        Rgb(self.0 + (o.0 - self.0) * t, self.1 + (o.1 - self.1) * t, self.2 + (o.2 - self.2) * t)
    }

    pub fn c(self) -> Color {
        let q = |v: f32| v.clamp(0.0, 255.0) as u8;
        Color::Rgb(q(self.0), q(self.1), q(self.2))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Screen background.
    pub bg: Rgb,
    /// Slightly lifted surface: selected row, popups.
    pub surface: Rgb,
    pub fg: Rgb,
    pub dim: Rgb,
    pub accent: Rgb,
    pub accent2: Rgb,
}

impl Palette {
    pub fn lerp(&self, o: &Palette, t: f32) -> Palette {
        Palette {
            bg: self.bg.lerp(o.bg, t),
            surface: self.surface.lerp(o.surface, t),
            fg: self.fg.lerp(o.fg, t),
            dim: self.dim.lerp(o.dim, t),
            accent: self.accent.lerp(o.accent, t),
            accent2: self.accent2.lerp(o.accent2, t),
        }
    }

    /// Weighted blend of several palettes.
    pub fn mix(items: &[(Palette, f32)]) -> Option<Palette> {
        let total: f32 = items.iter().map(|(_, w)| w).sum();
        if total <= 0.0 {
            return None;
        }
        let mut acc = items[0].0;
        let mut seen = items[0].1;
        for (p, w) in &items[1..] {
            seen += w;
            acc = acc.lerp(p, w / seen);
        }
        Some(acc)
    }
}

pub fn palette(t: Theme) -> Palette {
    match t {
        // Cold northern night, aurora blues.
        Theme::Skyrim => Palette {
            bg: Rgb::hex(0x0a1018),
            surface: Rgb::hex(0x141f2c),
            fg: Rgb::hex(0xdce8f5),
            dim: Rgb::hex(0x56677c),
            accent: Rgb::hex(0x8cc4ff),
            accent2: Rgb::hex(0x7fe0c8),
        },
        // Elven gold and Shire green.
        Theme::Lotr => Palette {
            bg: Rgb::hex(0x0e120b),
            surface: Rgb::hex(0x1b2216),
            fg: Rgb::hex(0xeee6cf),
            dim: Rgb::hex(0x6a6a52),
            accent: Rgb::hex(0xe9c46a),
            accent2: Rgb::hex(0x9cc97a),
        },
        // Opening-crawl yellow on deep space, a hint of saber blue.
        Theme::Starwars => Palette {
            bg: Rgb::hex(0x06060c),
            surface: Rgb::hex(0x13131f),
            fg: Rgb::hex(0xe4e4f0),
            dim: Rgb::hex(0x55556e),
            accent: Rgb::hex(0xffe14d),
            accent2: Rgb::hex(0x6fb4ff),
        },
        Theme::Nature => Palette {
            bg: Rgb::hex(0x08110e),
            surface: Rgb::hex(0x12211b),
            fg: Rgb::hex(0xdcefe5),
            dim: Rgb::hex(0x51695e),
            accent: Rgb::hex(0x6fd6b2),
            accent2: Rgb::hex(0x9ec6f0),
        },
        Theme::Generated => Palette {
            bg: Rgb::hex(0x0e0b15),
            surface: Rgb::hex(0x1b1626),
            fg: Rgb::hex(0xe6e0f5),
            dim: Rgb::hex(0x5f5778),
            accent: Rgb::hex(0xb69cff),
            accent2: Rgb::hex(0xff9cc8),
        },
        Theme::Other => Palette {
            bg: Rgb::hex(0x120e0c),
            surface: Rgb::hex(0x221b17),
            fg: Rgb::hex(0xf2e6de),
            dim: Rgb::hex(0x6e5f56),
            accent: Rgb::hex(0xf0a37a),
            accent2: Rgb::hex(0xe9c46a),
        },
    }
}

/// Neutral palette for when nothing plays: a quiet dusk.
pub fn idle() -> Palette {
    Palette {
        bg: Rgb::hex(0x0c0d12),
        surface: Rgb::hex(0x171922),
        fg: Rgb::hex(0xd8dae6),
        dim: Rgb::hex(0x585c70),
        accent: Rgb::hex(0x9aa6ff),
        accent2: Rgb::hex(0xc3a6ff),
    }
}

pub fn icon(t: Theme) -> &'static str {
    match t {
        Theme::Skyrim => "❄",
        Theme::Lotr => "✦",
        Theme::Starwars => "✧",
        Theme::Nature => "❀",
        Theme::Generated => "∿",
        Theme::Other => "♪",
    }
}
