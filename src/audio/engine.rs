use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, Sample, SizedSample, StreamConfig};
use rtrb::RingBuffer;

use super::sources::{SAMPLE_RATE, Source};

/// Per-layer ring buffer: one second of stereo audio.
const RING: usize = SAMPLE_RATE as usize * 2;
/// The feeder only wakes this often, so the CPU can sleep between bursts of decoding.
const FEED_INTERVAL: Duration = Duration::from_millis(350);
/// Callback buffer; latency doesn't matter for background audio, fewer wakeups do.
const DEVICE_FRAMES: u32 = 4096;
const BLOCK: usize = 1024;

/// f32 stored in an AtomicU32.
#[derive(Default)]
pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed)
    }
}

/// Shared between daemon, feeder and audio callback.
struct LayerCtl {
    target: AtomicF32,
    /// Peak of the last processed block, after gain.
    level: AtomicF32,
    removing: AtomicBool,
    /// Set by the callback once the fade-out finished and it let go of the layer.
    done: AtomicBool,
}

struct Shared {
    master: AtomicF32,
    /// Extra multiplier used by the timer fade-out.
    fade: AtomicF32,
    level: AtomicF32,
    /// Sum of current gains, so the daemon knows when everything has faded to silence.
    loudness: AtomicF32,
}

struct FeedLayer {
    id: String,
    source: Box<dyn Source>,
    producer: rtrb::Producer<f32>,
    ctl: Arc<LayerCtl>,
}

struct CbLayer {
    consumer: rtrb::Consumer<f32>,
    ctl: Arc<LayerCtl>,
    gain: f32,
}

enum StreamCmd {
    Play,
    Pause,
}

pub struct Engine {
    layers: HashMap<String, Arc<LayerCtl>>,
    feeder: Sender<FeedLayer>,
    callback: Sender<CbLayer>,
    stream: Sender<StreamCmd>,
    shared: Arc<Shared>,
    positions: Arc<Mutex<HashMap<String, u64>>>,
    running: bool,
}

impl Engine {
    pub fn new() -> Result<Self> {
        let shared = Arc::new(Shared {
            master: AtomicF32::new(0.8),
            fade: AtomicF32::new(1.0),
            level: AtomicF32::new(0.0),
            loudness: AtomicF32::new(0.0),
        });
        let positions = Arc::new(Mutex::new(HashMap::new()));

        let (feed_tx, feed_rx) = mpsc::channel();
        let pos = positions.clone();
        thread::Builder::new().name("feeder".into()).spawn(move || feeder(feed_rx, pos))?;

        let (cb_tx, cb_rx) = mpsc::channel();
        let (stream_tx, stream_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let sh = shared.clone();
        // cpal streams aren't Send, so one thread owns it for the daemon's lifetime.
        thread::Builder::new().name("output".into()).spawn(move || match open_stream(cb_rx, sh) {
            Ok(stream) => {
                let _ = ready_tx.send(Ok(()));
                for cmd in stream_rx {
                    let _ = match cmd {
                        StreamCmd::Play => stream.play(),
                        StreamCmd::Pause => stream.pause(),
                    };
                }
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        })?;
        ready_rx.recv().context("el hilo de audio murió")??;

        Ok(Self {
            layers: HashMap::new(),
            feeder: feed_tx,
            callback: cb_tx,
            stream: stream_tx,
            shared,
            positions,
            running: false,
        })
    }

    /// Start (or re-target) a layer. `volume` is the slider value; the gain curve is applied here.
    pub fn play(&mut self, id: &str, volume: f32, make: impl FnOnce() -> Result<Box<dyn Source>>) -> Result<()> {
        if let Some(ctl) = self.layers.get(id)
            && !ctl.removing.load(Ordering::Relaxed)
        {
            ctl.target.set(curve(volume));
            return Ok(());
        }
        let source = make()?;
        let ctl = Arc::new(LayerCtl {
            target: AtomicF32::new(curve(volume)),
            level: AtomicF32::new(0.0),
            removing: AtomicBool::new(false),
            done: AtomicBool::new(false),
        });
        let (producer, consumer) = RingBuffer::new(RING);
        self.feeder
            .send(FeedLayer { id: id.into(), source, producer, ctl: ctl.clone() })
            .map_err(|_| anyhow::anyhow!("el hilo de decodificación murió"))?;
        self.callback
            .send(CbLayer { consumer, ctl: ctl.clone(), gain: 0.0 })
            .map_err(|_| anyhow::anyhow!("el hilo de audio murió"))?;
        self.layers.insert(id.into(), ctl);
        self.set_running(true);
        Ok(())
    }

    pub fn set_volume(&self, id: &str, volume: f32) {
        if let Some(ctl) = self.layers.get(id) {
            ctl.target.set(curve(volume));
        }
    }

    /// Fade out and release a layer.
    pub fn stop(&mut self, id: &str) {
        if let Some(ctl) = self.layers.get(id) {
            ctl.removing.store(true, Ordering::Relaxed);
            ctl.target.set(0.0);
        }
    }

    pub fn set_master(&self, v: f32) {
        self.shared.master.set(curve(v));
    }

    pub fn set_fade(&self, v: f32) {
        self.shared.fade.set(v.clamp(0.0, 1.0));
    }

    pub fn levels(&self) -> (f32, Vec<(String, f32)>) {
        let layers = self.layers.iter().map(|(id, c)| (id.clone(), c.level.get())).collect();
        (self.shared.level.get(), layers)
    }

    pub fn positions(&self) -> HashMap<String, u64> {
        self.positions.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Housekeeping: forget finished layers and pause the device once everything is silent.
    pub fn tick(&mut self) {
        self.layers.retain(|_, c| !c.done.load(Ordering::Relaxed));
        let silent = self.layers.is_empty() && self.shared.loudness.get() == 0.0;
        if silent {
            self.set_running(false);
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    fn set_running(&mut self, on: bool) {
        if on != self.running {
            self.running = on;
            let _ = self.stream.send(if on { StreamCmd::Play } else { StreamCmd::Pause });
        }
    }
}

/// Perceptual volume curve for sliders.
fn curve(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v
}

fn feeder(rx: Receiver<FeedLayer>, positions: Arc<Mutex<HashMap<String, u64>>>) {
    let mut layers: Vec<FeedLayer> = Vec::new();
    let mut scratch = vec![0.0f32; RING];
    loop {
        match rx.recv_timeout(FEED_INTERVAL) {
            Ok(l) => layers.push(l),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(l) = rx.try_recv() {
            layers.push(l);
        }
        layers.retain(|l| !l.ctl.done.load(Ordering::Relaxed));
        for l in &mut layers {
            // Keep stereo frames aligned.
            let free = l.producer.slots() & !1;
            if free < BLOCK * 2 {
                continue;
            }
            let buf = &mut scratch[..free];
            l.source.fill(buf);
            let _ = l.producer.push_partial_slice(buf);
            if let Some(p) = l.source.position()
                && let Ok(mut map) = positions.lock()
            {
                map.insert(l.id.clone(), p);
            }
        }
    }
}

fn open_stream(rx: Receiver<CbLayer>, shared: Arc<Shared>) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_output_device().context("no hay dispositivo de salida de audio")?;
    let mut config =
        StreamConfig { channels: 2, sample_rate: SAMPLE_RATE, buffer_size: BufferSize::Fixed(DEVICE_FRAMES) };
    // The receiver sits in a slot the callback takes it from, so a failed attempt doesn't lose it.
    let slot = Arc::new(Mutex::new(Some(rx)));
    let stream = build::<f32>(&device, &config, slot.clone(), shared.clone()).or_else(|_| {
        // Some devices refuse a fixed buffer size; let them choose.
        config.buffer_size = BufferSize::Default;
        build::<f32>(&device, &config, slot.clone(), shared.clone())
    })?;
    stream.pause().ok();
    Ok(stream)
}

type Slot = Arc<Mutex<Option<Receiver<CbLayer>>>>;

fn build<T>(device: &cpal::Device, config: &StreamConfig, slot: Slot, shared: Arc<Shared>) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut mixer = Mixer::new(slot, shared);
    let stream = device.build_output_stream::<T, _, _>(
        *config,
        move |data: &mut [T], _| mixer.render(data, channels),
        |e| eprintln!("audio: {e}"),
        None,
    )?;
    Ok(stream)
}

/// Lives inside the audio callback: no locks, no allocation except when layers come and go.
struct Mixer {
    slot: Slot,
    rx: Option<Receiver<CbLayer>>,
    layers: Vec<CbLayer>,
    shared: Arc<Shared>,
    master: f32,
    mix: Vec<f32>,
    tmp: Vec<f32>,
}

// Linear slews per frame: fading in is gentle, volume tweaks still feel immediate.
const UP: f32 = 1.0 / (1.5 * SAMPLE_RATE as f32);
const DOWN: f32 = 1.0 / (0.8 * SAMPLE_RATE as f32);

impl Mixer {
    fn new(slot: Slot, shared: Arc<Shared>) -> Self {
        Self {
            slot,
            rx: None,
            layers: Vec::new(),
            shared,
            master: 0.0,
            mix: vec![0.0; BLOCK * 2],
            tmp: vec![0.0; BLOCK * 2],
        }
    }

    fn render<T: Sample + FromSample<f32>>(&mut self, data: &mut [T], channels: usize) {
        if self.rx.is_none()
            && let Ok(mut s) = self.slot.try_lock()
        {
            self.rx = s.take();
        }
        if let Some(rx) = &self.rx {
            while let Ok(l) = rx.try_recv() {
                self.layers.push(l);
            }
        }
        let master_target = self.shared.master.get() * self.shared.fade.get();
        let mut peak = 0.0f32;
        let mut loudness = 0.0f32;

        for out in data.chunks_mut(BLOCK * channels) {
            let frames = out.len() / channels;
            let mix = &mut self.mix[..frames * 2];
            mix.fill(0.0);
            for l in &mut self.layers {
                let tmp = &mut self.tmp[..frames * 2];
                // An underrun just plays silence for that stretch.
                let (_, rest) = l.consumer.pop_partial_slice(tmp);
                rest.fill(0.0);
                let target = l.ctl.target.get();
                let mut lpeak = 0.0f32;
                for (f, m) in tmp.as_chunks::<2>().0.iter().zip(mix.as_chunks_mut::<2>().0) {
                    l.gain = slew(l.gain, target);
                    let a = f[0] * l.gain;
                    let b = f[1] * l.gain;
                    m[0] += a;
                    m[1] += b;
                    lpeak = lpeak.max(a.abs()).max(b.abs());
                }
                l.ctl.level.set(lpeak);
                loudness += l.gain;
            }
            for (o, m) in out.chunks_exact_mut(channels).zip(mix.as_chunks::<2>().0) {
                self.master = slew(self.master, master_target);
                let l = soft_clip(m[0] * self.master);
                let r = soft_clip(m[1] * self.master);
                peak = peak.max(l.abs()).max(r.abs());
                o[0] = T::from_sample_(l);
                if channels > 1 {
                    o[1] = T::from_sample_(r);
                    for x in &mut o[2..] {
                        *x = T::from_sample_(0.0);
                    }
                }
            }
        }

        self.layers.retain(|l| {
            let gone = l.ctl.removing.load(Ordering::Relaxed) && l.gain == 0.0;
            if gone {
                l.ctl.done.store(true, Ordering::Relaxed);
            }
            !gone
        });
        self.shared.level.set(peak);
        self.shared.loudness.set(loudness);
    }
}

#[inline]
fn slew(cur: f32, target: f32) -> f32 {
    if cur < target {
        (cur + UP).min(target)
    } else if cur > target {
        (cur - DOWN).max(target)
    } else {
        cur
    }
}

/// Transparent below 0.7, then bends smoothly towards ±1 instead of clipping.
#[inline]
fn soft_clip(x: f32) -> f32 {
    const K: f32 = 0.7;
    let a = x.abs();
    if a <= K {
        x
    } else {
        let over = (a - K) / (1.0 - K);
        (K + (1.0 - K) * over.tanh()).copysign(x)
    }
}
