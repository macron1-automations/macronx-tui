use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

/// Number of amplitude buckets kept for the live visualization history.
const LEVEL_HISTORY: usize = 96;
/// Samples aggregated into a single visualization level.
const LEVEL_WINDOW: usize = 2048;
/// Frames per waveform peak entry (~46ms at 44.1kHz).
const PEAK_CHUNK_FRAMES: usize = 2048;

/// Amplitude history shared between the audio thread and the UI thread.
#[derive(Default)]
pub struct VizState {
    levels: Mutex<VecDeque<f32>>,
}

impl VizState {
    fn push(&self, level: f32) {
        let mut levels = self.levels.lock().unwrap();
        if levels.len() == LEVEL_HISTORY {
            levels.pop_front();
        }
        levels.push_back(level);
    }

    pub fn snapshot(&self) -> Vec<f32> {
        self.levels.lock().unwrap().iter().copied().collect()
    }
}

struct VizTap<S> {
    inner: S,
    viz: Arc<VizState>,
    window_peak: f32,
    window_remaining: usize,
}

impl<S: Iterator<Item = f32>> Iterator for VizTap<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let amplitude = sample.abs();
        if amplitude > self.window_peak {
            self.window_peak = amplitude;
        }
        self.window_remaining -= 1;
        if self.window_remaining == 0 {
            self.viz.push(self.window_peak.min(1.0));
            self.window_peak = 0.0;
            self.window_remaining = LEVEL_WINDOW;
        }
        Some(sample)
    }
}

impl<S: Source<Item = f32>> Source for VizTap<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

/// An audio attachment loaded for inline playback with live visualization.
pub struct AudioPlayer {
    _sink: MixerDeviceSink,
    player: Player,
    pub viz: Arc<VizState>,
    /// Max amplitude per `PEAK_CHUNK_FRAMES` frames, mono mixdown.
    pub peaks: Arc<Vec<f32>>,
    pub duration: Option<Duration>,
    pub attachment_id: u64,
}

impl AudioPlayer {
    pub fn load(attachment_id: u64, bytes: Vec<u8>) -> Result<Self> {
        let sink =
            DeviceSinkBuilder::open_default_sink().context("No audio output device available")?;
        let player = Player::connect_new(sink.mixer());

        let peaks = Arc::new(compute_peaks(&bytes)?);
        let viz = Arc::new(VizState::default());

        let source = Decoder::try_from(Cursor::new(bytes)).context("Unsupported audio format")?;
        let duration = source.total_duration();

        let tap = VizTap {
            inner: source,
            viz: Arc::clone(&viz),
            window_peak: 0.0,
            window_remaining: LEVEL_WINDOW,
        };
        player.append(tap);

        Ok(Self {
            _sink: sink,
            player,
            viz,
            peaks,
            duration,
            attachment_id,
        })
    }

    pub fn toggle(&self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn position(&self) -> Duration {
        self.player.get_pos()
    }

    pub fn seek_by(&self, delta: Duration) {
        let pos = self.position();
        let target = if delta.is_zero() {
            return;
        } else if pos + delta > pos {
            pos + delta
        } else {
            Duration::ZERO
        };
        let clamped = match self.duration {
            Some(total) if target > total => total,
            _ => target,
        };
        let _ = self.player.try_seek(clamped);
    }

    pub fn set_volume(&self, volume: f32) {
        self.player.set_volume(volume.clamp(0.0, 1.0));
    }

    pub fn volume(&self) -> f32 {
        self.player.volume()
    }
}

fn compute_peaks(bytes: &[u8]) -> Result<Vec<f32>> {
    let decoder =
        Decoder::try_from(Cursor::new(bytes.to_vec())).context("Unsupported audio format")?;
    let channels = decoder.channels().get().max(1) as usize;

    let mut peaks = Vec::new();
    let mut chunk_peak: f32 = 0.0;
    let mut frame_in_chunk: usize = 0;
    let mut channel_sum: f32 = 0.0;
    let mut channel_count: usize = 0;

    for sample in decoder {
        channel_sum += sample;
        channel_count += 1;
        if channel_count == channels {
            let mono = channel_sum / channels as f32;
            let amplitude = mono.abs();
            if amplitude > chunk_peak {
                chunk_peak = amplitude;
            }
            channel_sum = 0.0;
            channel_count = 0;
            frame_in_chunk += 1;
            if frame_in_chunk == PEAK_CHUNK_FRAMES {
                peaks.push(chunk_peak);
                chunk_peak = 0.0;
                frame_in_chunk = 0;
            }
        }
    }
    if frame_in_chunk > 0 {
        peaks.push(chunk_peak);
    }

    Ok(peaks)
}
