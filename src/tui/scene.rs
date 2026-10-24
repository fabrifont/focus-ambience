//! The animated band at the top: particles driven by whatever is playing.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::theme::{Palette, Rgb};
use crate::library::Visual;

/// Something playing, as far as the scene cares.
pub struct Emitter {
    pub visual: Visual,
    /// ~0..1: volume times how loud it is right now.
    pub intensity: f32,
    pub palette: Palette,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Drop,
    Splash,
    Flake,
    Firefly,
    Star,
    Streak,
    Ember,
    Leaf,
    Static,
}

struct Particle {
    kind: Kind,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    seed: f32,
    color: Rgb,
}

struct BgStar {
    x: u16,
    y: u16,
    phase: f32,
    speed: f32,
    bright: f32,
}

pub struct Scene {
    particles: Vec<Particle>,
    stars: Vec<BgStar>,
    size: (u16, u16),
    t: f32,
    rng: fastrand::Rng,
    /// Fractional spawns carried over between frames, per visual.
    debt: [f32; 9],
}

const MAX_PARTICLES: usize = 600;

impl Scene {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            stars: Vec::new(),
            size: (0, 0),
            t: 0.0,
            rng: fastrand::Rng::new(),
            debt: [0.0; 9],
        }
    }

    /// Whether anything is still moving (decides the frame rate).
    pub fn busy(&self) -> bool {
        !self.particles.is_empty()
    }

    pub fn update(&mut self, dt: f32, w: u16, h: u16, emitters: &[Emitter]) {
        self.t += dt;
        if self.size != (w, h) {
            self.size = (w, h);
            self.reseed_stars();
        }
        let (wf, hf) = (w as f32, h as f32);
        let width_k = (wf / 80.0).max(0.3);

        for e in emitters {
            let rate = match e.visual {
                Visual::Rain => 70.0,
                Visual::Snow => 14.0,
                Visual::Fireflies => 3.0,
                Visual::Stars => 3.5,
                Visual::Embers => 9.0,
                Visual::Leaves => 2.2,
                Visual::Static => 45.0,
                Visual::Waves => 0.0,
            } * width_k
                * e.intensity;
            let slot = e.visual as usize;
            self.debt[slot] += rate * dt;
            while self.debt[slot] >= 1.0 {
                self.debt[slot] -= 1.0;
                if self.particles.len() < MAX_PARTICLES {
                    self.spawn(e.visual, &e.palette, wf, hf);
                }
            }
        }

        let t = self.t;
        let mut splashes = Vec::new();
        for p in &mut self.particles {
            p.age += dt;
            match p.kind {
                Kind::Flake => {
                    p.x += (p.vx + (t * 0.9 + p.seed * 7.0).sin() * 0.8) * dt;
                    p.y += p.vy * dt;
                }
                Kind::Firefly => {
                    // Wander: slowly rotate the heading.
                    let a = (t * 0.5 + p.seed * 13.0).sin() * 2.5 + p.seed * 6.0;
                    p.x += a.cos() * 1.4 * dt;
                    p.y += a.sin() * 0.5 * dt;
                }
                Kind::Ember => {
                    p.x += ((t * 3.0 + p.seed * 9.0).sin() * 1.2 + p.vx) * dt;
                    p.y += p.vy * dt;
                }
                Kind::Leaf => {
                    p.x += (p.vx + (t * 1.3 + p.seed * 5.0).sin() * 1.5) * dt;
                    p.y += (p.vy + (t * 2.1 + p.seed * 3.0).cos() * 0.4) * dt;
                }
                _ => {
                    p.x += p.vx * dt;
                    p.y += p.vy * dt;
                }
            }
            if p.kind == Kind::Drop && p.y >= hf - 1.0 {
                p.age = p.life;
                if p.seed < 0.5 {
                    splashes.push((p.x, hf - 1.0, p.color));
                }
            }
        }
        for (x, y, color) in splashes {
            self.particles.push(Particle {
                kind: Kind::Splash,
                x,
                y,
                vx: 0.0,
                vy: 0.0,
                age: 0.0,
                life: 0.18,
                seed: 0.0,
                color,
            });
        }
        self.particles.retain(|p| p.age < p.life && p.x > -16.0 && p.x < wf + 16.0 && p.y > -2.0 && p.y < hf + 1.0);
    }

    fn reseed_stars(&mut self) {
        let (w, h) = self.size;
        let n = (w as usize * h as usize) / 45;
        self.stars = (0..n)
            .map(|_| BgStar {
                x: self.rng.u16(0..w.max(1)),
                y: self.rng.u16(0..h.max(1)),
                phase: self.rng.f32() * std::f32::consts::TAU,
                speed: 0.4 + self.rng.f32() * 1.2,
                bright: 0.15 + self.rng.f32() * 0.35,
            })
            .collect();
    }

    fn spawn(&mut self, v: Visual, pal: &Palette, w: f32, h: f32) {
        let r = &mut self.rng;
        let seed = r.f32();
        let mut p =
            Particle { kind: Kind::Drop, x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, age: 0.0, life: 10.0, seed, color: pal.fg };
        match v {
            Visual::Rain => {
                p.kind = Kind::Drop;
                p.x = r.f32() * (w + 6.0);
                p.y = -1.0;
                p.vx = -2.0;
                p.vy = 16.0 + r.f32() * 12.0;
                p.color = pal.accent2.lerp(pal.fg, 0.25);
            }
            Visual::Snow => {
                p.kind = Kind::Flake;
                p.x = r.f32() * w;
                p.y = -1.0;
                p.vx = -0.3;
                p.vy = 0.9 + seed * 2.2;
                p.color = pal.fg;
            }
            Visual::Fireflies => {
                p.kind = Kind::Firefly;
                p.x = r.f32() * w;
                p.y = 1.0 + r.f32() * (h - 2.0).max(1.0);
                p.life = 3.5 + r.f32() * 4.0;
                p.color = pal.accent;
            }
            Visual::Stars => {
                if r.f32() < 0.04 {
                    p.kind = Kind::Streak;
                    p.x = w + 4.0;
                    p.vx = -(45.0 + r.f32() * 30.0);
                    p.color = pal.accent2.lerp(pal.fg, 0.5);
                } else {
                    p.kind = Kind::Star;
                    p.x = w + 1.0;
                    p.vx = -(0.6 + seed * 3.0);
                    p.color = pal.fg;
                }
                p.y = r.f32() * h;
                p.life = 60.0;
            }
            Visual::Embers => {
                p.kind = Kind::Ember;
                p.x = w * (0.2 + r.f32() * 0.6);
                p.y = h;
                p.vx = (r.f32() - 0.5) * 1.5;
                p.vy = -(1.8 + r.f32() * 3.0);
                p.life = 1.5 + r.f32() * 2.5;
                p.color = Rgb::hex(0xffc46a);
            }
            Visual::Leaves => {
                p.kind = Kind::Leaf;
                p.x = r.f32() * w * 0.8;
                p.y = -1.0;
                p.vx = 1.5 + r.f32() * 2.0;
                p.vy = 1.0 + r.f32() * 1.2;
                p.life = 30.0;
                p.color = if seed < 0.6 { pal.accent2 } else { pal.accent };
            }
            Visual::Static => {
                p.kind = Kind::Static;
                p.x = r.f32() * w;
                p.y = r.f32() * h;
                p.life = 0.06 + r.f32() * 0.1;
                p.color = pal.dim.lerp(pal.fg, r.f32() * 0.5);
            }
            Visual::Waves => return,
        }
        self.particles.push(p);
    }

    pub fn draw(&self, buf: &mut Buffer, area: Rect, pal: &Palette, emitters: &[Emitter]) {
        let (w, h) = (area.width as usize, area.height as usize);
        if w == 0 || h == 0 {
            return;
        }
        let t = self.t;

        // Background: vertical fade with a slow aurora-like shimmer near the top.
        let mut bgs = Vec::with_capacity(w * h);
        for y in 0..h {
            let fy = y as f32 / h as f32;
            let row = pal.bg.lerp(pal.surface, (1.0 - fy) * 0.55);
            for x in 0..w {
                let wave = ((x as f32 * 0.07 + t * 0.25 + fy * 2.0).sin() * 0.5 + 0.5) * (1.0 - fy).powi(2);
                let bg = row.lerp(pal.accent2, wave * 0.07);
                buf[(area.x + x as u16, area.y + y as u16)].set_char(' ').set_bg(bg.c());
                bgs.push(bg);
            }
        }

        // Strongest glyph per cell wins; alpha is how far it is from the background.
        let mut best = vec![0.0f32; w * h];
        let mut put = |buf: &mut Buffer, x: f32, y: f32, ch: char, color: Rgb, alpha: f32| {
            if x < 0.0 || y < 0.0 {
                return;
            }
            let (xi, yi) = (x as usize, y as usize);
            if xi >= w || yi >= h || alpha <= best[yi * w + xi] {
                return;
            }
            best[yi * w + xi] = alpha;
            let fg = bgs[yi * w + xi].lerp(color, alpha);
            buf[(area.x + xi as u16, area.y + yi as u16)].set_char(ch).set_fg(fg.c());
        };

        for s in &self.stars {
            if (s.x as usize) < w && (s.y as usize) < h {
                let tw = ((t * s.speed + s.phase).sin() * 0.5 + 0.5) * s.bright;
                let ch = if tw > 0.38 { '∙' } else { '·' };
                put(buf, s.x as f32, s.y as f32, ch, pal.fg, tw * 0.7);
            }
        }

        for e in emitters.iter().filter(|e| e.visual == Visual::Waves) {
            let amp = (h as f32 / 2.0 - 1.0) * (0.35 + 0.65 * e.intensity.min(1.0));
            let mid = h as f32 / 2.0;
            for x in 0..w {
                let fx = x as f32;
                let a = (fx * 0.21 - t * 2.0).sin();
                let b = (fx * 0.25 - t * 2.4).sin();
                // The beat: where both waves agree, the line glows.
                let beat = 1.0 - ((a - b).abs() / 2.0);
                let env = 0.25 + 0.5 * e.intensity.min(1.0);
                put(buf, fx, mid + a * amp * 0.9, '·', e.palette.accent, env * 0.8);
                put(buf, fx, mid + b * amp * 0.9, '·', e.palette.accent2, env * 0.8);
                if beat > 0.92 {
                    let y = mid + (a + b) / 2.0 * amp * 0.9;
                    put(buf, fx, y, '•', e.palette.accent.lerp(e.palette.fg, 0.5), env + 0.2);
                }
            }
        }

        for p in &self.particles {
            let life_t = p.age / p.life;
            match p.kind {
                Kind::Drop => {
                    let a = 0.35 + 0.35 * p.seed;
                    put(buf, p.x, p.y, if p.vy > 22.0 { '│' } else { '╎' }, p.color, a);
                }
                Kind::Splash => put(buf, p.x, p.y, if life_t < 0.5 { '˙' } else { '·' }, p.color, 0.5 * (1.0 - life_t)),
                Kind::Flake => {
                    let near = p.seed > 0.7;
                    let ch = if p.seed > 0.96 {
                        '❄'
                    } else if near {
                        '•'
                    } else {
                        '·'
                    };
                    put(buf, p.x, p.y, ch, p.color, if near { 0.85 } else { 0.5 });
                }
                Kind::Firefly => {
                    let pulse = (t * 2.3 + p.seed * 20.0).sin() * 0.5 + 0.5;
                    let fade = (life_t * 6.0).min(1.0).min((1.0 - life_t) * 4.0);
                    let a = (0.3 + 0.7 * pulse) * fade;
                    let ch = if a > 0.65 {
                        '•'
                    } else if a > 0.35 {
                        '∙'
                    } else {
                        '·'
                    };
                    put(buf, p.x, p.y, ch, p.color, a);
                }
                Kind::Star => {
                    let a = 0.25 + (-p.vx / 3.6) * 0.6;
                    put(
                        buf,
                        p.x,
                        p.y,
                        if a > 0.7 {
                            '✦'
                        } else if a > 0.45 {
                            '∙'
                        } else {
                            '·'
                        },
                        p.color,
                        a,
                    );
                }
                Kind::Streak => {
                    for i in 0..14 {
                        let k = 1.0 - i as f32 / 14.0;
                        put(buf, p.x + i as f32, p.y, if i == 0 { '━' } else { '─' }, p.color, 0.9 * k);
                    }
                }
                Kind::Ember => {
                    let color = Rgb::hex(0xffd27a).lerp(Rgb::hex(0xff5a1f), life_t);
                    let flick = 0.6 + 0.4 * (t * 17.0 + p.seed * 40.0).sin().abs();
                    let a = (1.0 - life_t) * flick;
                    put(
                        buf,
                        p.x,
                        p.y,
                        if a > 0.6 {
                            '•'
                        } else if a > 0.3 {
                            '∙'
                        } else {
                            '˙'
                        },
                        color,
                        a,
                    );
                }
                Kind::Leaf => {
                    const LEAF: [char; 4] = ['❧', '´', ',', '`'];
                    let spin = ((t * 1.7 + p.seed * 10.0) as usize) % 4;
                    let ch = if p.seed > 0.85 { '❦' } else { LEAF[spin] };
                    put(buf, p.x, p.y, ch, p.color, 0.75);
                }
                Kind::Static => put(buf, p.x, p.y, if p.seed > 0.7 { '░' } else { '·' }, p.color, 0.45),
            }
        }
    }
}
