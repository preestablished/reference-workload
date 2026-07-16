//! Audio playback sink for interactive record mode (feature `interactive`).
//!
//! `refwork-emu`'s S-DSP synthesizes the full stereo audio stream every
//! frame already (it always did — see `Core::take_audio_samples`); this
//! module only taps that stream and plays it on the host via
//! [`cpal`](https://docs.rs/cpal) (CoreAudio on macOS, ALSA on Linux). It is
//! purely a host-frontend affordance: nothing here touches the pad word, the
//! padlog, or emulator state (see `ARCHITECTURE.md` §1 and the plan's
//! "host-side-only UX" constraint).
//!
//! # Design
//!
//! - [`AudioSink::open`] never fails: any device/stream/format error prints
//!   one stderr note and degrades to a no-op sink (mirrors the gamepad
//!   fallback style in `record.rs`/`gamepad_macos.rs`).
//! - The device's native sample rate is rarely
//!   [`refwork_emu::AUDIO_SAMPLE_RATE_HZ`] (32 kHz), so [`Resampler`]
//!   converts with **stateful linear interpolation**: it carries the
//!   fractional phase and the last input pair across `push` calls, so a
//!   chunk boundary never introduces a discontinuity (see the `continuity`
//!   test below).
//! - The shared queue (`Arc<Mutex<VecDeque<i16>>>`) always holds interleaved
//!   **stereo pairs** — the callback pops exactly 2 samples per device frame
//!   and zero-fills any further device channels — so every depth/watermark
//!   computation uses the pair layout (2 channels), never the device channel
//!   count. It is pre-allocated to the high watermark plus one push of
//!   headroom (extend runs before trim in the same critical section), so it
//!   does not regrow under the lock in practice. Resampling happens
//!   *outside* the lock; the lock is held only to read the depth, extend,
//!   and (rarely) trim. The realtime audio callback uses `try_lock` with a
//!   silence fallback so it never blocks the audio thread.
//! - **Closed-loop rate control**: the host frame limiter and the audio
//!   device run on unrelated clocks, so any fixed producer rate drifts —
//!   at ~60 fps production is ~0.17% short of the 32 kHz consumption, which
//!   with an empty queue would mean continuous underrun crackle. Instead the
//!   queue is primed with [`LOW_WATERMARK_MS`] of silence at open, and every
//!   `push` slews the resample ratio by up to [`MAX_RATE_SLEW`] (±0.5%,
//!   inaudible) toward that depth. Proportional-only control settles a bit
//!   below target under steady drift (~2/3 of it at the expected ~0.17%) —
//!   still an ample cushion. A fully drained queue (stalled frame loop,
//!   e.g. the blocking F5 dump prompt) is re-primed in one step rather than
//!   recovered through the slow slew. The producer-side watermark
//!   ([`apply_watermark`]) stays as a hard backstop for producer bursts the
//!   loop cannot absorb.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// High watermark: once the device-rate queue exceeds this many milliseconds
/// of audio, drop the oldest samples down to `LOW_WATERMARK_MS`.
const HIGH_WATERMARK_MS: u32 = 250;
/// Trim target when the high watermark is exceeded; also the priming depth
/// and the closed-loop rate-control target (see the module doc).
const LOW_WATERMARK_MS: u32 = 100;
/// Maximum multiplicative resample-ratio slew applied by the closed loop
/// (±0.5% ≈ ±8.6 cents of pitch — inaudible, and only ~0.17% is needed in
/// steady state to absorb the frame-limiter vs device-clock drift).
const MAX_RATE_SLEW: f64 = 0.005;
/// The queue's own channel layout: interleaved stereo pairs, always —
/// independent of the device channel count (extra device channels are
/// zero-filled by the callback).
const QUEUE_CHANNELS: usize = 2;

/// Owns the cpal output stream (if one could be opened) and the shared
/// playback queue. Safe to construct unconditionally: [`AudioSink::open`]
/// degrades to a no-op sink rather than failing.
pub struct AudioSink {
    resampler: Resampler,
    /// Reusable scratch buffer for resampled output, to avoid a fresh
    /// allocation on every `push` call.
    scratch: Vec<i16>,
    muted: bool,
    device: Option<DeviceState>,
}

/// State that only exists when a real output stream is running. Kept
/// separate from `AudioSink` so the muted/no-device state is representable
/// without touching cpal at all (this is what keeps the module's tests
/// device-free).
struct DeviceState {
    /// Kept alive for the lifetime of the sink; cpal streams stop producing
    /// audio once dropped. Never read directly after construction.
    _stream: cpal::Stream,
    queue: Arc<Mutex<VecDeque<i16>>>,
    mute_flag: Arc<AtomicBool>,
    high_watermark: usize,
    low_watermark: usize,
    /// Closed-loop rate-control target depth (queue samples); the ratio
    /// slew in `push` steers the queue toward this level.
    depth_target: usize,
    /// Count of watermark-trim events (not samples) since the sink opened.
    watermark_drops: u64,
}

impl AudioSink {
    /// Open the default output device and start playback. Never fails: any
    /// error along the way (no device, unsupported config, stream build
    /// failure, stream start failure) is reported as a single stderr note
    /// and yields a no-op sink that silently discards `push`.
    pub fn open() -> AudioSink {
        match try_open() {
            Ok(sink) => sink,
            Err(cause) => {
                eprintln!("interactive: audio unavailable ({cause}) — continuing silent");
                AudioSink::disabled()
            }
        }
    }

    /// A sink with no backing device: `push` is a no-op, `muted` still
    /// tracks the toggle state for the window title. Used directly by
    /// `--no-audio` (skip device construction entirely) and by tests
    /// (nothing here touches cpal).
    pub fn disabled() -> AudioSink {
        AudioSink {
            // The rates are irrelevant — `push` returns immediately when
            // `device` is `None`, before the resampler is ever touched.
            resampler: Resampler::new(1, 1),
            scratch: Vec::new(),
            muted: false,
            device: None,
        }
    }

    /// Resample `samples` (interleaved stereo i16 at
    /// [`refwork_emu::AUDIO_SAMPLE_RATE_HZ`]) to the device rate and enqueue
    /// them for playback. No-op when there is no backing device. Resampling
    /// runs outside any lock; the queue is locked once to extend it and
    /// (rarely) apply the watermark trim.
    pub fn push(&mut self, samples: &[i16]) {
        let Some(device) = self.device.as_mut() else {
            return;
        };

        // Closed-loop rate control: read the current depth (cheap lock, no
        // resampling under it) and slew the resample ratio toward the target
        // depth. Below target -> emit slightly more output per input; above
        // -> slightly less. See the module doc. A fully drained queue (the
        // live loop stalled, e.g. blocked on the F5 dump prompt while the
        // device kept consuming) is re-primed with silence in one step: the
        // ±0.5% slew authority would otherwise take many seconds to rebuild
        // the cushion, crackling the whole way.
        let (depth, reprimed) = {
            let mut queue = device
                .queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let reprimed = ensure_primed(&mut queue, device.depth_target);
            (queue.len(), reprimed)
        };
        if reprimed {
            eprintln!(
                "interactive: audio queue drained (stalled frame loop?) — re-primed with \
                 {}ms of silence",
                LOW_WATERMARK_MS
            );
        }
        let target = device.depth_target as f64;
        let error = (target - depth as f64) / target; // >0 when queue is low
        let scale = 1.0 - error.clamp(-1.0, 1.0) * MAX_RATE_SLEW;
        self.resampler.set_rate_scale(scale);

        self.scratch.clear();
        self.resampler.push(samples, &mut self.scratch);
        if self.scratch.is_empty() {
            return;
        }

        let dropped = {
            let mut queue = device
                .queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            queue.extend(self.scratch.iter().copied());
            apply_watermark(&mut queue, device.high_watermark, device.low_watermark)
        };
        if dropped > 0 {
            device.watermark_drops += 1;
        }
    }

    /// Toggle mute. Purely host-side: never touches the pad word, the
    /// padlog, or the core.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if let Some(device) = &self.device {
            device.mute_flag.store(muted, Ordering::Relaxed);
        }
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Count of watermark-trim events since the sink opened (0 if disabled).
    /// Intended for a shutdown diagnostic; see `record.rs::run_interactive`.
    pub fn watermark_drops(&self) -> u64 {
        self.device.as_ref().map_or(0, |d| d.watermark_drops)
    }
}

/// Build and start the real output stream. Any failure becomes a single
/// `String` cause for [`AudioSink::open`]'s stderr note.
fn try_open() -> Result<AudioSink, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no output device".to_owned())?;
    let supported_config = device
        .default_output_config()
        .map_err(|e| format!("no output config: {e}"))?;
    let sample_format = supported_config.sample_format();
    let stream_config: cpal::StreamConfig = supported_config.config();
    let device_rate = stream_config.sample_rate.0;
    let channels = stream_config.channels as usize;
    if channels < 2 {
        return Err(format!("device has {channels} channel(s), need at least 2"));
    }
    if device_rate < 1000 {
        // Also guards the rate-control arithmetic: a (physically implausible)
        // rate below 10 Hz would make the watermarks/depth target 0, and a
        // zero depth target would poison the slew math with NaN.
        return Err(format!("implausible device sample rate {device_rate} Hz"));
    }

    // Watermarks are in QUEUE units (stereo-pair interleaved — the queue
    // never stores device-channel frames), rounded down to whole pairs so a
    // trim can never break L/R alignment.
    let high_watermark = even_floor(ms_to_sample_count(
        HIGH_WATERMARK_MS,
        device_rate,
        QUEUE_CHANNELS,
    ));
    let low_watermark = even_floor(ms_to_sample_count(
        LOW_WATERMARK_MS,
        device_rate,
        QUEUE_CHANNELS,
    ));
    // Headroom above high: extend runs before trim in push(), so the queue
    // transiently exceeds `high` by up to one resampled push chunk. At very
    // high device rates a stall-recovery chunk can exceed this once and
    // regrow the deque under the lock — harmless: it happens at most once
    // (capacity sticks) and the callback side uses try_lock, never blocking.
    let mut initial: VecDeque<i16> = VecDeque::with_capacity(high_watermark + 8192);
    // Prime with the target depth of silence: gives the closed loop a
    // cushion so startup jitter doesn't underrun before control converges.
    initial.extend(std::iter::repeat_n(0i16, low_watermark));
    let queue = Arc::new(Mutex::new(initial));
    let mute_flag = Arc::new(AtomicBool::new(false));

    let stream = match sample_format {
        cpal::SampleFormat::I16 => build_i16_stream(
            &device,
            &stream_config,
            channels,
            Arc::clone(&queue),
            Arc::clone(&mute_flag),
        ),
        cpal::SampleFormat::F32 => build_f32_stream(
            &device,
            &stream_config,
            channels,
            Arc::clone(&queue),
            Arc::clone(&mute_flag),
        ),
        other => return Err(format!("unsupported sample format {other:?}")),
    }
    .map_err(|e| format!("cannot build stream: {e}"))?;

    stream
        .play()
        .map_err(|e| format!("cannot start stream: {e}"))?;

    Ok(AudioSink {
        resampler: Resampler::new(refwork_emu::AUDIO_SAMPLE_RATE_HZ, device_rate),
        scratch: Vec::new(),
        muted: false,
        device: Some(DeviceState {
            _stream: stream,
            queue,
            mute_flag,
            high_watermark,
            low_watermark,
            depth_target: low_watermark,
            watermark_drops: 0,
        }),
    })
}

fn build_i16_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    queue: Arc<Mutex<VecDeque<i16>>>,
    mute_flag: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_output_stream(
        config,
        move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
            fill_i16(data, channels, &queue, &mute_flag);
        },
        stream_error_callback,
        None,
    )
}

fn build_f32_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    queue: Arc<Mutex<VecDeque<i16>>>,
    mute_flag: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_output_stream(
        config,
        move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
            fill_f32(data, channels, &queue, &mute_flag);
        },
        stream_error_callback,
        None,
    )
}

fn stream_error_callback(err: cpal::StreamError) {
    eprintln!("interactive: audio stream error: {err}");
}

/// Realtime audio callback body for `i16` device output. Kept thin: on lock
/// contention, output silence for the whole buffer rather than blocking.
fn fill_i16(data: &mut [i16], channels: usize, queue: &Mutex<VecDeque<i16>>, mute_flag: &AtomicBool) {
    let muted = mute_flag.load(Ordering::Relaxed);
    match queue.try_lock() {
        Ok(mut q) => {
            for frame in data.chunks_mut(channels) {
                let l = q.pop_front().unwrap_or(0);
                let r = q.pop_front().unwrap_or(0);
                write_frame_i16(frame, l, r, muted);
            }
        }
        Err(_) => data.fill(0),
    }
}

/// Realtime audio callback body for `f32` device output.
fn fill_f32(data: &mut [f32], channels: usize, queue: &Mutex<VecDeque<i16>>, mute_flag: &AtomicBool) {
    let muted = mute_flag.load(Ordering::Relaxed);
    match queue.try_lock() {
        Ok(mut q) => {
            for frame in data.chunks_mut(channels) {
                let l = q.pop_front().unwrap_or(0);
                let r = q.pop_front().unwrap_or(0);
                write_frame_f32(frame, l, r, muted);
            }
        }
        Err(_) => data.fill(0.0),
    }
}

fn write_frame_i16(frame: &mut [i16], l: i16, r: i16, muted: bool) {
    let (l, r) = if muted { (0, 0) } else { (l, r) };
    if let Some(s) = frame.first_mut() {
        *s = l;
    }
    if let Some(s) = frame.get_mut(1) {
        *s = r;
    }
    for s in frame.iter_mut().skip(2) {
        *s = 0;
    }
}

fn write_frame_f32(frame: &mut [f32], l: i16, r: i16, muted: bool) {
    let (l, r) = if muted { (0, 0) } else { (l, r) };
    if let Some(s) = frame.first_mut() {
        *s = l as f32 / 32768.0;
    }
    if let Some(s) = frame.get_mut(1) {
        *s = r as f32 / 32768.0;
    }
    for s in frame.iter_mut().skip(2) {
        *s = 0.0;
    }
}

/// Total i16-sample count (interleaved, all channels) corresponding to
/// `ms` milliseconds of audio at `rate_hz` with `channels` channels.
fn ms_to_sample_count(ms: u32, rate_hz: u32, channels: usize) -> usize {
    (rate_hz as usize) * channels * (ms as usize) / 1000
}

/// Round down to a multiple of 2: queue thresholds must be whole stereo
/// pairs or a watermark trim would permanently swap L/R alignment.
fn even_floor(n: usize) -> usize {
    n & !1
}

/// Re-prime a fully drained queue with `target` samples of silence (the
/// one-step recovery for a stalled producer; see the module doc). Returns
/// whether priming happened. Pure logic, testable without a device.
fn ensure_primed(queue: &mut VecDeque<i16>, target: usize) -> bool {
    if !queue.is_empty() {
        return false;
    }
    queue.extend(std::iter::repeat_n(0i16, target));
    true
}

/// Producer-side watermark policy: if `queue` holds more than `high`
/// samples, drop the oldest samples down to `low`. Returns the number of
/// samples dropped (0 if the watermark was not exceeded). Pure logic over
/// the queue type so it is testable without a device or the sink.
fn apply_watermark(queue: &mut VecDeque<i16>, high: usize, low: usize) -> usize {
    if queue.len() <= high {
        return 0;
    }
    let target = low.min(queue.len());
    let drop_count = queue.len() - target;
    for _ in 0..drop_count {
        queue.pop_front();
    }
    drop_count
}

// ─── Resampler ─────────────────────────────────────────────────────────────

/// Stateful linear-interpolation resampler for interleaved stereo `i16`.
///
/// Carries the fractional phase and the last input pair across `push`
/// calls, so resampling one logical stream through many small `push` calls
/// produces the same output as one big call (bit-identical in the
/// `continuity` test; in general equal up to f64 rounding of the phase
/// accumulator, never a waveform discontinuity). Never reset the state
/// mid-stream — a fresh [`Resampler`] implies a discontinuity at that point.
///
/// The effective ratio can be slewed multiplicatively via
/// [`Resampler::set_rate_scale`] (closed-loop rate control; see the module
/// doc). Ratio changes take effect between output samples and introduce no
/// discontinuity — only a tiny, inaudible pitch shift.
///
/// Because linear interpolation needs a "next" sample to interpolate
/// towards, the very last input pair of any given `push` call is held back
/// (not yet emitted) until either more input arrives in a later call, or
/// forever if it never does. This is normal for a streaming resampler and
/// is exercised directly by the tests below.
struct Resampler {
    /// Nominal input-pairs-per-output-pair (`in_rate / out_rate`).
    base_ratio: f64,
    /// Effective ratio: `base_ratio` times the current rate scale.
    ratio: f64,
    /// Fractional position of the next output sample, in input-sample
    /// units, relative to `prev`.
    phase: f64,
    /// Last input pair consumed so far (interpolation anchor for the start
    /// of the next `push` call).
    prev: (i16, i16),
    has_prev: bool,
}

impl Resampler {
    fn new(in_rate: u32, out_rate: u32) -> Resampler {
        let base_ratio = in_rate as f64 / out_rate as f64;
        Resampler {
            base_ratio,
            ratio: base_ratio,
            phase: 0.0,
            prev: (0, 0),
            has_prev: false,
        }
    }

    /// Set the effective ratio to `base_ratio * scale`. `scale < 1` emits
    /// more output per input (fills a draining queue), `scale > 1` less.
    /// Clamped to ±[`MAX_RATE_SLEW`] regardless of the caller's value.
    fn set_rate_scale(&mut self, scale: f64) {
        let scale = scale.clamp(1.0 - MAX_RATE_SLEW, 1.0 + MAX_RATE_SLEW);
        self.ratio = self.base_ratio * scale;
    }

    /// Resample `input` (interleaved stereo i16) and append the result to
    /// `out`. `input.len()` is expected to be even (a whole number of
    /// stereo pairs); a trailing odd sample, if any, is ignored.
    fn push(&mut self, input: &[i16], out: &mut Vec<i16>) {
        let n_total = input.len() / 2;
        if n_total == 0 {
            return;
        }
        let pair_at = |i: usize| -> (i16, i16) { (input[2 * i], input[2 * i + 1]) };

        let mut start = 0usize;
        if !self.has_prev {
            self.prev = pair_at(0);
            self.has_prev = true;
            start = 1;
        }
        // `n` remaining pairs, addressable at synthetic positions 1..=n
        // relative to `prev` at position 0 (`pair_at(start + k - 1)` is the
        // sample at position `k`).
        let n = n_total - start;

        loop {
            let idx0f = self.phase.floor();
            let idx0 = idx0f as usize;
            if idx0 >= n {
                break;
            }
            let frac = self.phase - idx0f;
            let s0 = if idx0 == 0 {
                self.prev
            } else {
                pair_at(start + idx0 - 1)
            };
            let s1 = pair_at(start + idx0);
            out.push(lerp(s0.0, s1.0, frac));
            out.push(lerp(s0.1, s1.1, frac));
            self.phase += self.ratio;
        }
        if n > 0 {
            self.prev = pair_at(start + n - 1);
            self.phase -= n as f64;
        }
    }
}

fn lerp(a: i16, b: i16, t: f64) -> i16 {
    let v = a as f64 + (b as f64 - a as f64) * t;
    v.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

// ─── Tests ───────────────────────────────────────────────────────────────────
//
// No test in this module opens a device: `try_open` (and everything else
// that touches cpal) is only reachable through `AudioSink::open`, which
// nothing here calls.

#[cfg(test)]
mod tests {
    use super::*;

    fn interleave(l: &[i16], r: &[i16]) -> Vec<i16> {
        assert_eq!(l.len(), r.len());
        l.iter()
            .zip(r.iter())
            .flat_map(|(&a, &b)| [a, b])
            .collect()
    }

    #[test]
    fn resampler_identity_at_equal_rates() {
        // At a 1:1 rate the resampler reproduces its input exactly, minus
        // the trailing pair that is always held back awaiting the "next"
        // sample (see the struct docs).
        let l: Vec<i16> = (0..8).map(|i| i * 100).collect();
        let r: Vec<i16> = (0..8).map(|i| -i * 50).collect();
        let input = interleave(&l, &r);

        let mut resampler = Resampler::new(32_000, 32_000);
        let mut out = Vec::new();
        resampler.push(&input, &mut out);

        assert_eq!(out, input[..input.len() - 2]);
    }

    #[test]
    fn resampler_length_ratio_32k_to_48k() {
        // 32k -> 48k is a 1.5x upsample; verify the output length matches
        // that ratio within a small fixed-point tolerance.
        let n = 3200usize; // 0.1s of audio at 32kHz
        let l: Vec<i16> = (0..n as i16).collect();
        let r: Vec<i16> = (0..n as i16).map(|v| -v).collect();
        let input = interleave(&l, &r);

        let mut resampler = Resampler::new(32_000, 48_000);
        let mut out = Vec::new();
        resampler.push(&input, &mut out);

        let expected_pairs = n as f64 * 48_000.0 / 32_000.0;
        let actual_pairs = out.len() / 2;
        assert!(
            (actual_pairs as f64 - expected_pairs).abs() <= 2.0,
            "actual {actual_pairs} vs expected ~{expected_pairs}"
        );
    }

    #[test]
    fn resampler_preserves_channel_interleaving() {
        // L carries a ramp, R stays at zero throughout; a channel swap or
        // cross-mix would show up as nonzero R output.
        let n = 64usize;
        let l: Vec<i16> = (0..n as i16).map(|v| v * 100).collect();
        let r = vec![0i16; n];
        let input = interleave(&l, &r);

        let mut resampler = Resampler::new(32_000, 48_000);
        let mut out = Vec::new();
        resampler.push(&input, &mut out);

        assert!(!out.is_empty());
        assert_eq!(out.len() % 2, 0);
        for pair in out.chunks_exact(2) {
            assert_eq!(pair[1], 0, "R channel leaked a nonzero sample: {pair:?}");
        }
    }

    #[test]
    fn resampler_empty_input_is_a_no_op() {
        let mut resampler = Resampler::new(32_000, 48_000);
        let mut out = Vec::new();
        resampler.push(&[], &mut out);
        assert!(out.is_empty());

        // State is unaffected: a real chunk afterwards behaves as if it
        // were the first push.
        let input = interleave(&[10, 20, 30], &[-10, -20, -30]);
        resampler.push(&input, &mut out);
        assert!(!out.is_empty());
    }

    #[test]
    fn resampler_continuity_across_chunk_boundaries() {
        // A deterministic, non-trivial waveform (values vary enough that
        // interpolation actually matters at different fractional phases).
        let n = 500usize;
        let l: Vec<i16> = (0..n as i16)
            .map(|i| ((i as f64 * 0.37).sin() * 10_000.0) as i16)
            .collect();
        let r: Vec<i16> = (0..n as i16)
            .map(|i| ((i as f64 * 0.11).cos() * 8_000.0) as i16)
            .collect();
        let input = interleave(&l, &r);

        let mut whole = Resampler::new(32_000, 48_000);
        let mut out_whole = Vec::new();
        whole.push(&input, &mut out_whole);

        // Same input fed through many small, unevenly sized chunks.
        let mut split = Resampler::new(32_000, 48_000);
        let mut out_split = Vec::new();
        let chunk_pair_sizes = [1usize, 3, 7, 50, 2, 137, 1, 300 /* covers the remainder */];
        let mut offset_pairs = 0usize;
        let total_pairs = input.len() / 2;
        for &size in &chunk_pair_sizes {
            if offset_pairs >= total_pairs {
                break;
            }
            let take = size.min(total_pairs - offset_pairs);
            let start = offset_pairs * 2;
            let end = start + take * 2;
            split.push(&input[start..end], &mut out_split);
            offset_pairs += take;
        }
        assert_eq!(offset_pairs, total_pairs, "test chunking must cover all input");

        assert_eq!(out_whole, out_split);
    }

    #[test]
    fn watermark_leaves_queue_untouched_below_threshold() {
        let mut queue: VecDeque<i16> = (0..100).collect();
        let dropped = apply_watermark(&mut queue, 200, 50);
        assert_eq!(dropped, 0);
        assert_eq!(queue.len(), 100);
    }

    #[test]
    fn watermark_trims_oldest_down_to_target() {
        let mut queue: VecDeque<i16> = (0..300).collect();
        let dropped = apply_watermark(&mut queue, 200, 50);
        assert_eq!(dropped, 250);
        assert_eq!(queue.len(), 50);
        // The oldest samples were dropped: what remains is the tail.
        assert_eq!(queue.front().copied(), Some(250));
        assert_eq!(queue.back().copied(), Some(299));
    }

    #[test]
    fn watermark_exactly_at_high_is_not_trimmed() {
        let mut queue: VecDeque<i16> = (0..200).collect();
        let dropped = apply_watermark(&mut queue, 200, 50);
        assert_eq!(dropped, 0);
        assert_eq!(queue.len(), 200);
    }

    #[test]
    fn sink_mute_toggle_semantics() {
        let mut sink = AudioSink::disabled();
        assert!(!sink.muted());
        sink.set_muted(true);
        assert!(sink.muted());
        sink.set_muted(false);
        assert!(!sink.muted());
    }

    #[test]
    fn disabled_sink_push_is_a_no_op_and_never_panics() {
        let mut sink = AudioSink::disabled();
        sink.push(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(sink.watermark_drops(), 0);
    }

    #[test]
    fn ms_to_sample_count_matches_expectation() {
        // 250ms @ 48kHz stereo = 48000 * 2 * 0.25 = 24000 samples.
        assert_eq!(ms_to_sample_count(250, 48_000, 2), 24_000);
        assert_eq!(ms_to_sample_count(100, 48_000, 2), 9_600);
    }

    #[test]
    fn ensure_primed_refills_only_an_empty_queue() {
        let mut queue: VecDeque<i16> = VecDeque::new();
        assert!(ensure_primed(&mut queue, 8));
        assert_eq!(queue.len(), 8);
        assert!(queue.iter().all(|&s| s == 0));
        // Non-empty: untouched, reports false.
        assert!(!ensure_primed(&mut queue, 8));
        assert_eq!(queue.len(), 8);
        queue.clear();
        queue.push_back(7);
        assert!(!ensure_primed(&mut queue, 8));
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn even_floor_keeps_pair_alignment() {
        // 11025 Hz is the classic odd case: 11025*2*100/1000 = 2205.
        assert_eq!(even_floor(ms_to_sample_count(100, 11_025, 2)), 2204);
        assert_eq!(even_floor(24_000), 24_000);
        assert_eq!(even_floor(0), 0);
    }

    #[test]
    fn rate_scale_slews_output_volume_and_is_clamped() {
        // Same input through three resamplers: scale < 1 must yield at
        // least as many output pairs as nominal, scale > 1 at most — and
        // an out-of-range scale must clamp to the ±MAX_RATE_SLEW bound.
        let n = 8000usize;
        let l: Vec<i16> = (0..n as i16).collect();
        let r = vec![0i16; n];
        let input = interleave(&l, &r);

        let run = |scale: Option<f64>| -> usize {
            let mut rs = Resampler::new(32_000, 48_000);
            if let Some(s) = scale {
                rs.set_rate_scale(s);
            }
            let mut out = Vec::new();
            rs.push(&input, &mut out);
            out.len() / 2
        };
        let nominal = run(None);
        let faster = run(Some(1.0 - MAX_RATE_SLEW));
        let slower = run(Some(1.0 + MAX_RATE_SLEW));
        assert!(faster > nominal, "faster {faster} vs nominal {nominal}");
        assert!(slower < nominal, "slower {slower} vs nominal {nominal}");
        // Wildly out-of-range scales clamp to the same bounds.
        assert_eq!(run(Some(0.0)), faster);
        assert_eq!(run(Some(10.0)), slower);
        // The slew is small: within ~1% of nominal either way.
        let tol = nominal / 100 + 2;
        assert!(nominal.abs_diff(faster) <= tol);
        assert!(nominal.abs_diff(slower) <= tol);
    }
}
