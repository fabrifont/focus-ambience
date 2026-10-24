use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, Sample, SampleFormat, SizedSample, StreamConfig};
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

/// Tries 48 kHz/stereo/f32 first (unchanged behaviour on devices that already support it), then
/// falls back to whatever the device reports as its default config, adapting the sample format,
/// channel count and rate in the mixer instead of giving up with no sound.
fn open_stream(rx: Receiver<CbLayer>, shared: Arc<Shared>) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_output_device().context("no hay dispositivo de salida de audio")?;
    let exact = StreamConfig { channels: 2, sample_rate: SAMPLE_RATE, buffer_size: BufferSize::Fixed(DEVICE_FRAMES) };
    // The receiver sits in a slot the callback takes it from, so a failed attempt doesn't lose it.
    let slot = Arc::new(Mutex::new(Some(rx)));
    let mut tried = Vec::new();

    let attempt = |config: &StreamConfig, format: SampleFormat, tried: &mut Vec<String>| {
        let r = build_for_format(format, &device, config, slot.clone(), shared.clone());
        if let Err(e) = &r {
            tried.push(format!("{} Hz, {} canales, {format}: {e}", config.sample_rate, config.channels));
        }
        r
    };

    if let Ok(stream) = attempt(&exact, SampleFormat::F32, &mut tried) {
        stream.pause().ok();
        return Ok(stream);
    }
    // Some devices refuse a fixed buffer size; let them choose.
    let exact_default = StreamConfig { buffer_size: BufferSize::Default, ..exact };
    if let Ok(stream) = attempt(&exact_default, SampleFormat::F32, &mut tried) {
        stream.pause().ok();
        return Ok(stream);
    }

    let fallback = device.default_output_config().with_context(|| {
        format!(
            "48 kHz estéreo f32 no funcionó ({}) y el dispositivo no tiene una config por defecto",
            tried.join("; ")
        )
    })?;
    let format = fallback.sample_format();
    let mut config = fallback.config();
    config.buffer_size = BufferSize::Fixed(DEVICE_FRAMES);
    let stream = match attempt(&config, format, &mut tried) {
        Ok(s) => s,
        Err(_) => {
            config.buffer_size = BufferSize::Default;
            attempt(&config, format, &mut tried).map_err(|e| {
                anyhow::anyhow!("no se pudo abrir ningún dispositivo de audio. Se probó: {}; {e}", tried.join("; "))
            })?
        }
    };
    stream.pause().ok();
    Ok(stream)
}

type Slot = Arc<Mutex<Option<Receiver<CbLayer>>>>;

fn build_for_format(
    format: SampleFormat,
    device: &cpal::Device,
    config: &StreamConfig,
    slot: Slot,
    shared: Arc<Shared>,
) -> Result<cpal::Stream> {
    match format {
        SampleFormat::F32 => build::<f32>(device, config, slot, shared),
        SampleFormat::I16 => build::<i16>(device, config, slot, shared),
        SampleFormat::U16 => build::<u16>(device, config, slot, shared),
        SampleFormat::I32 => build::<i32>(device, config, slot, shared),
        SampleFormat::U32 => build::<u32>(device, config, slot, shared),
        _ => anyhow::bail!("formato de muestra no soportado: {format}"),
    }
}

fn build<T>(device: &cpal::Device, config: &StreamConfig, slot: Slot, shared: Arc<Shared>) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut mixer = Mixer::new(slot, shared, config.sample_rate);
    let stream = device.build_output_stream::<T, _, _>(
        *config,
        move |data: &mut [T], _| mixer.render(data, channels),
        |e| eprintln!("audio: {e}"),
        None,
    )?;
    Ok(stream)
}

/// Lives inside the audio callback: no locks, no allocation in steady state (the resampling
/// scratch buffer only grows if a callback ever needs more than `DEVICE_FRAMES` worth of 48 kHz
/// audio, which doesn't happen once the device settles on a buffer size).
struct Mixer {
    slot: Slot,
    rx: Option<Receiver<CbLayer>>,
    layers: Vec<CbLayer>,
    shared: Arc<Shared>,
    master: f32,
    mix: Vec<f32>,
    tmp: Vec<f32>,
    /// Scratch for the 48 kHz frames generated this callback, before resampling to device rate.
    mix48: Vec<f32>,
    /// SAMPLE_RATE / device_rate: how many 48 kHz source frames one device frame is worth.
    resample_step: f64,
    /// Device-rate frames rendered so far; together with `resample_step` gives the exact
    /// (drift-free) source position for every output frame.
    out_count: u64,
    /// 48 kHz source frames generated so far.
    src_count: u64,
    /// The most recently generated 48 kHz frame (abs index `src_count - 1`), kept as the
    /// interpolation anchor for the first output frames of the next callback.
    last_frame: [f32; 2],
}

// Linear slews per frame: fading in is gentle, volume tweaks still feel immediate. These always
// run at the internal 48 kHz rate, regardless of the device's actual output rate.
const UP: f32 = 1.0 / (1.5 * SAMPLE_RATE as f32);
const DOWN: f32 = 1.0 / (0.8 * SAMPLE_RATE as f32);

impl Mixer {
    fn new(slot: Slot, shared: Arc<Shared>, device_rate: u32) -> Self {
        Self {
            slot,
            rx: None,
            layers: Vec::new(),
            shared,
            master: 0.0,
            mix: vec![0.0; BLOCK * 2],
            tmp: vec![0.0; BLOCK * 2],
            mix48: vec![0.0; DEVICE_FRAMES as usize * 2],
            resample_step: SAMPLE_RATE as f64 / device_rate as f64,
            out_count: 0,
            src_count: 0,
            last_frame: [0.0, 0.0],
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

        let out_frames = data.len() / channels;
        let new_count = resample_new_frames_needed(self.out_count, out_frames, self.src_count, self.resample_step);
        if self.mix48.len() < new_count * 2 {
            self.mix48.resize(new_count * 2, 0.0);
        }
        self.mix_48khz(new_count);

        let src = &self.mix48[..new_count * 2];
        for (j, out) in data.chunks_exact_mut(channels).enumerate() {
            let pos = (self.out_count + j as u64) as f64 * self.resample_step;
            let idx = pos.floor() as u64;
            let t = (pos - idx as f64) as f32;
            let a = sample_at(idx, self.src_count, self.last_frame, src);
            let b = sample_at(idx + 1, self.src_count, self.last_frame, src);
            let frame = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            write_channels::<T>(frame, out);
        }

        if new_count > 0 {
            self.last_frame = [src[(new_count - 1) * 2], src[(new_count - 1) * 2 + 1]];
        }
        self.src_count += new_count as u64;
        self.out_count += out_frames as u64;
    }

    /// Mixes all layers (gain slew, levels, soft-clipped master) into `self.mix48[..n*2]`, at the
    /// fixed internal 48 kHz rate. This is the part that was previously fused with writing to the
    /// device buffer directly; it's now separate so `render` can resample its output afterwards.
    fn mix_48khz(&mut self, n: usize) {
        let master_target = self.shared.master.get() * self.shared.fade.get();
        let mut peak = 0.0f32;
        let mut loudness = 0.0f32;
        let mut offset = 0;
        while offset < n {
            let frames = (n - offset).min(BLOCK);
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
            let out = &mut self.mix48[offset * 2..(offset + frames) * 2];
            for (o, m) in out.as_chunks_mut::<2>().0.iter_mut().zip(mix.as_chunks::<2>().0) {
                self.master = slew(self.master, master_target);
                let l = soft_clip(m[0] * self.master);
                let r = soft_clip(m[1] * self.master);
                peak = peak.max(l.abs()).max(r.abs());
                o[0] = l;
                o[1] = r;
            }
            offset += frames;
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

/// How many new 48 kHz source frames must be generated (on top of the `src_count` already made)
/// before `out_frames` more device-rate frames, starting at absolute output index `out_count`,
/// can be resampled. Always includes a 1-frame lookahead for the interpolation, so steady state
/// keeps exactly one 48 kHz frame buffered ahead of what's been consumed.
fn resample_new_frames_needed(out_count: u64, out_frames: usize, src_count: u64, step: f64) -> usize {
    if out_frames == 0 {
        return 0;
    }
    let pos_last = (out_count + out_frames as u64 - 1) as f64 * step;
    let max_idx = pos_last.floor() as u64 + 1;
    (max_idx + 1).saturating_sub(src_count) as usize
}

/// The 48 kHz stereo frame at absolute index `idx`. `last_frame` is the single frame generated
/// right before `new_src` (abs index `src_count - 1`); `new_src` holds abs indices
/// `src_count..`. Clamps to the last available frame instead of panicking if `idx` ever runs
/// past what was generated (only matters for the unused side of a zero-weight interpolation).
fn sample_at(idx: u64, src_count: u64, last_frame: [f32; 2], new_src: &[f32]) -> [f32; 2] {
    if idx < src_count {
        return last_frame;
    }
    let available = new_src.len() / 2;
    if available == 0 {
        return last_frame;
    }
    let local = ((idx - src_count) as usize).min(available - 1);
    [new_src[local * 2], new_src[local * 2 + 1]]
}

/// Writes a mixed stereo frame into a device output slice sized for `out.len()` channels:
/// passthrough for stereo, averaged down for mono, extra channels beyond the first two silenced.
fn write_channels<T: Sample + FromSample<f32>>(frame: [f32; 2], out: &mut [T]) {
    match out.len() {
        0 => {}
        1 => out[0] = T::from_sample_((frame[0] + frame[1]) * 0.5),
        _ => {
            out[0] = T::from_sample_(frame[0]);
            out[1] = T::from_sample_(frame[1]);
            for x in &mut out[2..] {
                *x = T::from_sample_(0.0);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_channels_stereo_passthrough_and_silences_extra() {
        let mut out = [1.0f32; 4]; // simulate a 4-channel device
        write_channels([0.3, -0.5], &mut out);
        assert_eq!(out, [0.3, -0.5, 0.0, 0.0]);
    }

    #[test]
    fn write_channels_mono_averages() {
        let mut out = [0.0f32; 1];
        write_channels([1.0, 0.5], &mut out);
        assert_eq!(out[0], 0.75);
    }

    #[test]
    fn write_channels_empty_does_not_panic() {
        let mut out: [f32; 0] = [];
        write_channels([1.0, 1.0], &mut out);
    }

    #[test]
    fn sample_at_before_new_src_returns_last_frame() {
        let new_src = [10.0, 20.0, 30.0, 40.0]; // two frames: [10,20], [30,40]
        let last = [1.0, 2.0];
        assert_eq!(sample_at(4, 5, last, &new_src), last);
    }

    #[test]
    fn sample_at_indexes_into_new_src() {
        let new_src = [10.0, 20.0, 30.0, 40.0];
        let last = [1.0, 2.0];
        assert_eq!(sample_at(5, 5, last, &new_src), [10.0, 20.0]);
        assert_eq!(sample_at(6, 5, last, &new_src), [30.0, 40.0]);
    }

    #[test]
    fn sample_at_clamps_past_the_end() {
        let new_src = [10.0, 20.0, 30.0, 40.0];
        let last = [1.0, 2.0];
        // idx 7 is one past what's generated; clamp to the last frame instead of panicking.
        assert_eq!(sample_at(7, 5, last, &new_src), [30.0, 40.0]);
    }

    #[test]
    fn resample_new_frames_needed_zero_out_frames() {
        assert_eq!(resample_new_frames_needed(0, 0, 0, 1.0), 0);
    }

    #[test]
    fn resample_passthrough_is_bit_exact_across_callbacks() {
        // step == 1.0: every output frame must equal the matching 48 kHz source frame exactly,
        // same as the un-resampled code path before this change.
        let step = 1.0;
        let mut src_count = 0u64;
        let mut out_count = 0u64;
        let mut last = [0.0f32, 0.0];
        let sizes = [1024usize, 1024, 37]; // irregular last callback
        let mut produced = Vec::new();
        for &out_frames in &sizes {
            let new_count = resample_new_frames_needed(out_count, out_frames, src_count, step);
            let new_src: Vec<f32> = (0..new_count)
                .flat_map(|i| [(src_count + i as u64) as f32, -((src_count + i as u64) as f32)])
                .collect();
            for j in 0..out_frames {
                let pos = (out_count + j as u64) as f64 * step;
                let idx = pos.floor() as u64;
                let t = (pos - idx as f64) as f32;
                assert_eq!(t, 0.0);
                let a = sample_at(idx, src_count, last, &new_src);
                produced.push(a);
            }
            if new_count > 0 {
                last = [new_src[(new_count - 1) * 2], new_src[(new_count - 1) * 2 + 1]];
            }
            src_count += new_count as u64;
            out_count += out_frames as u64;
        }
        for (n, frame) in produced.iter().enumerate() {
            assert_eq!(*frame, [n as f32, -(n as f32)]);
        }
    }

    #[test]
    fn resample_downsample_tracks_source_without_drift() {
        // 48000 -> 44100: step > 1. Over many irregular callbacks, the resampled ramp must keep
        // tracking `j * step` without accumulating drift or losing/duplicating source frames.
        let step = 48_000.0_f64 / 44_100.0;
        let mut src_count = 0u64;
        let mut out_count = 0u64;
        let mut last = [0.0f32, 0.0];
        let sizes = [4096usize, 4096, 4096, 123, 4096];
        let mut produced = Vec::new();
        for &out_frames in &sizes {
            let new_count = resample_new_frames_needed(out_count, out_frames, src_count, step);
            let new_src: Vec<f32> = (0..new_count).flat_map(|i| [(src_count + i as u64) as f32, 0.0]).collect();
            for j in 0..out_frames {
                let pos = (out_count + j as u64) as f64 * step;
                let idx = pos.floor() as u64;
                let t = (pos - idx as f64) as f32;
                let a = sample_at(idx, src_count, last, &new_src);
                let b = sample_at(idx + 1, src_count, last, &new_src);
                produced.push(a[0] + (b[0] - a[0]) * t);
            }
            if new_count > 0 {
                last = [new_src[(new_count - 1) * 2], new_src[(new_count - 1) * 2 + 1]];
            }
            src_count += new_count as u64;
            out_count += out_frames as u64;
        }
        for (n, v) in produced.iter().enumerate() {
            let expected = n as f32 * step as f32;
            assert!((v - expected).abs() < 1.0, "frame {n}: got {v}, expected ~{expected}");
        }
    }
}
