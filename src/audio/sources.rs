use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ogg::PacketReader;

use crate::library::Generator;

pub const SAMPLE_RATE: u32 = 48_000;

/// Something that endlessly produces interleaved stereo f32 at 48 kHz.
pub trait Source: Send {
    fn fill(&mut self, out: &mut [f32]);
    /// Playback position in samples per channel, for sources worth resuming.
    fn position(&self) -> Option<u64> {
        None
    }
}

pub fn generator(g: Generator) -> Box<dyn Source> {
    match g {
        Generator::White => Box::new(Noise::new(NoiseColor::White)),
        Generator::Pink => Box::new(Noise::new(NoiseColor::Pink)),
        Generator::Brown => Box::new(Noise::new(NoiseColor::Brown)),
        Generator::Binaural40 => Box::new(Binaural::new(200.0, 40.0)),
    }
}

#[derive(Clone, Copy)]
enum NoiseColor {
    White,
    Pink,
    Brown,
}

/// Independent noise per channel so it feels wide instead of sitting in the middle of the head.
struct Noise {
    color: NoiseColor,
    rng: fastrand::Rng,
    pink: [[f32; 3]; 2],
    brown: [f32; 2],
}

impl Noise {
    fn new(color: NoiseColor) -> Self {
        Self { color, rng: fastrand::Rng::new(), pink: [[0.0; 3]; 2], brown: [0.0; 2] }
    }
}

impl Source for Noise {
    fn fill(&mut self, out: &mut [f32]) {
        for (i, s) in out.iter_mut().enumerate() {
            let ch = i & 1;
            let w = self.rng.f32() * 2.0 - 1.0;
            *s = match self.color {
                NoiseColor::White => w * 0.25,
                NoiseColor::Pink => {
                    // Paul Kellet's economy pink filter.
                    let b = &mut self.pink[ch];
                    b[0] = 0.99765 * b[0] + w * 0.099_046;
                    b[1] = 0.963 * b[1] + w * 0.296_516_4;
                    b[2] = 0.57 * b[2] + w * 1.052_691_3;
                    (b[0] + b[1] + b[2] + w * 0.1848) * 0.11
                }
                NoiseColor::Brown => {
                    let b = &mut self.brown[ch];
                    *b = (*b + 0.02 * w) / 1.02;
                    *b * 3.0
                }
            };
        }
    }
}

/// Carrier in the left ear, carrier + beat in the right ear.
struct Binaural {
    phase: [f64; 2],
    step: [f64; 2],
}

impl Binaural {
    fn new(carrier: f64, beat: f64) -> Self {
        let sr = SAMPLE_RATE as f64;
        Self { phase: [0.0; 2], step: [carrier / sr, (carrier + beat) / sr] }
    }
}

impl Source for Binaural {
    fn fill(&mut self, out: &mut [f32]) {
        for frame in out.as_chunks_mut::<2>().0 {
            for ((s, phase), step) in frame.iter_mut().zip(&mut self.phase).zip(self.step) {
                *s = (*phase * std::f64::consts::TAU).sin() as f32 * 0.2;
                *phase = (*phase + step).fract();
            }
        }
    }
}

/// Opus samples decoded and discarded after a seek so the decoder converges (RFC 7845 §4.6).
const PREROLL: u64 = 3840;
/// Largest Opus packet: 120 ms at 48 kHz.
const MAX_FRAME: usize = 5760;

/// Streams an Ogg Opus file from disk, looping forever.
pub struct OpusFile {
    path: PathBuf,
    reader: PacketReader<BufReader<File>>,
    decoder: opus::Decoder,
    channels: usize,
    pre_skip: u64,
    /// Samples (per channel) still to drop before output.
    discard: u64,
    decoded: Vec<f32>,
    pending: Vec<f32>,
    pending_pos: usize,
    position: u64,
    failed: bool,
}

impl OpusFile {
    pub fn open(path: &Path, start: u64) -> Result<Self> {
        let (reader, channels, pre_skip) = open_stream(path)?;
        let mut f = Self {
            path: path.to_path_buf(),
            reader,
            decoder: new_decoder(channels)?,
            channels,
            pre_skip,
            discard: pre_skip,
            decoded: vec![0.0; MAX_FRAME * channels],
            pending: Vec::with_capacity(MAX_FRAME * 2),
            pending_pos: 0,
            position: 0,
            failed: false,
        };
        if start > PREROLL && f.seek(start).is_err() {
            f.restart()?;
        }
        Ok(f)
    }

    fn seek(&mut self, target: u64) -> Result<()> {
        let goal = self.pre_skip + target - PREROLL;
        if !self.reader.seek_absgp(None, goal)? {
            bail!("seek fuera del archivo");
        }
        self.decoder.reset_state()?;
        self.discard = PREROLL;
        self.position = target;
        Ok(())
    }

    fn restart(&mut self) -> Result<()> {
        let (reader, _, _) = open_stream(&self.path)?;
        self.reader = reader;
        self.decoder.reset_state()?;
        self.discard = self.pre_skip;
        self.position = 0;
        Ok(())
    }

    /// Decode the next packet into `pending` (always stereo). Returns false on a hard failure.
    fn decode_next(&mut self) -> bool {
        let packet = match self.reader.read_packet() {
            Ok(Some(p)) => p,
            Ok(None) => return self.restart().is_ok(),
            Err(_) => return self.restart().is_ok(),
        };
        let n = match self.decoder.decode_float(&packet.data, &mut self.decoded, false) {
            Ok(n) => n,
            Err(_) => return true, // skip a damaged packet
        };
        let skip = (self.discard as usize).min(n);
        self.discard -= skip as u64;
        self.pending.clear();
        self.pending_pos = 0;
        let frames = &self.decoded[skip * self.channels..n * self.channels];
        if self.channels == 1 {
            self.pending.extend(frames.iter().flat_map(|&s| [s, s]));
        } else {
            self.pending.extend_from_slice(frames);
        }
        self.position += (n - skip) as u64;
        true
    }
}

impl Source for OpusFile {
    fn fill(&mut self, out: &mut [f32]) {
        let mut i = 0;
        while i < out.len() {
            if self.pending_pos >= self.pending.len() {
                if self.failed || !self.decode_next() {
                    self.failed = true;
                    out[i..].fill(0.0);
                    return;
                }
                continue;
            }
            let n = (self.pending.len() - self.pending_pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.pending[self.pending_pos..self.pending_pos + n]);
            self.pending_pos += n;
            i += n;
        }
    }

    fn position(&self) -> Option<u64> {
        Some(self.position)
    }
}

fn new_decoder(channels: usize) -> Result<opus::Decoder> {
    let ch = if channels == 1 { opus::Channels::Mono } else { opus::Channels::Stereo };
    Ok(opus::Decoder::new(SAMPLE_RATE, ch)?)
}

fn open_stream(path: &Path) -> Result<(PacketReader<BufReader<File>>, usize, u64)> {
    let file = File::open(path).with_context(|| format!("abriendo {}", path.display()))?;
    let mut reader = PacketReader::new(BufReader::with_capacity(64 * 1024, file));
    let head = reader.read_packet_expected()?;
    if head.data.len() < 19 || &head.data[..8] != b"OpusHead" {
        bail!("{} no es Ogg Opus", path.display());
    }
    let channels = head.data[9] as usize;
    if !(1..=2).contains(&channels) {
        bail!("{}: {channels} canales no soportados", path.display());
    }
    let pre_skip = u16::from_le_bytes([head.data[10], head.data[11]]) as u64;
    reader.read_packet_expected()?; // OpusTags
    Ok((reader, channels, pre_skip))
}
