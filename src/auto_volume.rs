//! Conservative fan-level control using a quiet baseline and a playback power reference.
//! This is a spectral leakage guard, not acoustic echo cancellation or a speech classifier.
use crate::synth::Mode;
use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use serde::Serialize;
use std::{f32::consts::TAU, sync::Arc};

pub const BANDS: usize = 24;
pub type Powers = [f32; BANDS];
pub const INITIAL_DB: f32 = -6.0;
pub const MIN_DB: f32 = -30.0;

pub fn band_index(hz: f32) -> Option<usize> {
    if !(80.0..5000.0).contains(&hz) {
        return None;
    }
    Some((((hz / 80.0).ln() / (5000.0_f32 / 80.0).ln()) * BANDS as f32) as usize)
}
pub fn level_db(p: &Powers) -> f32 {
    10.0 * p.iter().sum::<f32>().max(1e-12).log10()
}
pub fn gain_from_db(db: f32) -> f32 {
    10.0_f32.powf(db.clamp(MIN_DB, 0.0) / 20.0)
}
pub fn limited_gain(maximum: f32, adaptive: f32, attenuation: f32) -> f32 {
    if !maximum.is_finite() || !adaptive.is_finite() || !attenuation.is_finite() {
        return 0.0;
    }
    maximum.clamp(0.0, crate::level::db_to_gain(crate::level::MAX_LEVEL_DB))
        * adaptive.clamp(0.0, 1.0)
        * attenuation.clamp(0.0, 1.0)
}

/// Written to a bounded SPSC queue by playback. No heap data or blocking operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReferenceFrame {
    pub channels: [f32; 2],
}

/// Only used by the analysis worker, never the audio callback.
pub struct ReferenceMeter {
    ring: Vec<ReferenceFrame>,
    cursor: usize,
    count: usize,
    window: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex32>,
    scratch: Vec<Complex32>,
    rate: f32,
}
impl ReferenceMeter {
    pub fn new(rate: f32, n: usize) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(n);
        Self {
            ring: vec![ReferenceFrame::default(); n],
            cursor: 0,
            count: 0,
            window: (0..n)
                .map(|i| 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos())
                .collect(),
            scratch: vec![Complex32::default(); fft.get_inplace_scratch_len()],
            buffer: vec![Complex32::default(); n],
            fft,
            rate,
        }
    }
    pub fn push(&mut self, frame: ReferenceFrame) {
        self.ring[self.cursor] = frame;
        self.cursor = (self.cursor + 1) % self.ring.len();
        self.count = self.count.saturating_add(1);
    }
    pub fn powers(&mut self) -> Option<Powers> {
        let n = self.ring.len();
        if self.count < n {
            return None;
        }
        let mut bands = [0.0; BANDS];
        for ch in 0..2 {
            for i in 0..n {
                self.buffer[i] = Complex32::new(
                    self.ring[(self.cursor + i) % n].channels[ch] * self.window[i],
                    0.0,
                );
            }
            self.fft
                .process_with_scratch(&mut self.buffer, &mut self.scratch);
            // Hann mean-square correction, one-sided PSD, mean of channel powers.
            let norm = 8.0 / (3.0 * (n * n) as f32);
            for bin in 1..n / 2 {
                if let Some(band) = band_index(bin as f32 * self.rate / n as f32) {
                    bands[band] += self.buffer[bin].norm_sqr() * norm;
                }
            }
        }
        Some(bands)
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct VolumeStatus {
    pub gain_db: f32,
    pub target_db: f32,
    pub fan_dbfs: f32,
    pub reference_dbfs: f32,
    pub confidence: f32,
    pub state: &'static str,
}
impl Default for VolumeStatus {
    fn default() -> Self {
        Self {
            gain_db: INITIAL_DB,
            target_db: INITIAL_DB,
            fan_dbfs: -120.0,
            reference_dbfs: -120.0,
            confidence: 0.0,
            state: "Waiting for fan measurement",
        }
    }
}
pub struct Observation {
    pub microphone: Powers,
    pub playback: Powers,
    pub quiet: bool,
    pub trusted_quiet: bool,
    pub independent: bool,
    pub reference_valid: bool,
    pub mode: Mode,
    pub seconds: f32,
}
pub struct AdaptiveVolume {
    baseline: Powers,
    microphone: Powers,
    playback: Powers,
    coupling: Powers,
    profile_reference: Powers,
    learning_microphone: Powers,
    learning_reference: Powers,
    calibrated_frames: usize,
    profile_frames: usize,
    learning_time: f32,
    profile_ready: bool,
    was_quiet: bool,
    mode: Option<Mode>,
    stable_seconds: f32,
    candidate_db: f32,
    last_ratio_db: f32,
    pub status: VolumeStatus,
}
impl Default for AdaptiveVolume {
    fn default() -> Self {
        Self {
            baseline: [0.0; BANDS],
            microphone: [0.0; BANDS],
            playback: [0.0; BANDS],
            coupling: [0.0; BANDS],
            profile_reference: [0.0; BANDS],
            learning_microphone: [0.0; BANDS],
            learning_reference: [0.0; BANDS],
            calibrated_frames: 0,
            profile_frames: 0,
            learning_time: 0.0,
            profile_ready: false,
            was_quiet: true,
            mode: None,
            stable_seconds: 0.0,
            candidate_db: 0.0,
            last_ratio_db: 0.0,
            status: VolumeStatus::default(),
        }
    }
}
impl AdaptiveVolume {
    fn reset_profile(&mut self) {
        self.learning_time = 0.0;
        self.profile_frames = 0;
        self.learning_microphone.fill(0.0);
        self.learning_reference.fill(0.0);
        self.profile_ready = false;
        self.stable_seconds = 0.0;
    }
    fn hold(&mut self, dt: f32) -> VolumeStatus {
        self.status.target_db = self.status.gain_db;
        self.finish(dt)
    }
    fn finish(&mut self, dt: f32) -> VolumeStatus {
        // Slow level changes in addition to the sample-level fade in Synth.
        let delta = self.status.target_db - self.status.gain_db;
        self.status.gain_db += delta.clamp(-1.5 * dt, 0.45 * dt);
        self.status.gain_db = self.status.gain_db.clamp(MIN_DB, 0.0);
        self.status.clone()
    }
    pub fn update(&mut self, o: Observation) -> VolumeStatus {
        let dt = if o.seconds.is_finite() {
            o.seconds.clamp(0.001, 0.25)
        } else {
            0.02
        };
        self.status.confidence = 0.0;
        if o.microphone
            .iter()
            .chain(o.playback.iter())
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            self.status.state = "Holding: invalid level measurement";
            return self.hold(dt);
        }
        if o.quiet {
            if !self.was_quiet {
                self.calibrated_frames = 0;
                self.baseline.fill(0.0);
                self.reset_profile();
            }
            self.was_quiet = true;
            if o.trusted_quiet {
                self.calibrated_frames += 1;
                let alpha = 1.0 / self.calibrated_frames.min(80) as f32;
                for (baseline, power) in self.baseline.iter_mut().zip(o.microphone) {
                    *baseline += alpha * (power - *baseline);
                }
                self.status.reference_dbfs = level_db(&self.baseline);
                self.status.fan_dbfs = self.status.reference_dbfs;
            }
            self.status.state = "Measuring fan";
            return self.hold(dt);
        }
        if self.calibrated_frames < 4 || self.baseline.iter().sum::<f32>() < 1e-11 {
            self.status.state = "Holding: fan too quiet to estimate";
            return self.hold(dt);
        }
        if self.was_quiet {
            self.microphone = self.baseline;
            self.playback.fill(0.0);
            self.last_ratio_db = 0.0;
        }
        self.was_quiet = false;
        if self.mode != Some(o.mode) {
            self.mode = Some(o.mode);
            self.reset_profile();
        }
        if !o.independent && !o.reference_valid {
            self.status.state = "Holding: playback reference unavailable";
            return self.hold(dt);
        }
        let alpha = 1.0 - (-dt / 0.65).exp();
        for i in 0..BANDS {
            self.microphone[i] += alpha * (o.microphone[i] - self.microphone[i]);
            self.playback[i] += alpha * (o.playback[i] - self.playback[i]);
        }
        let reference_present = self.playback.iter().sum::<f32>() > 1e-12;
        if !o.independent && reference_present && !self.profile_ready {
            self.learning_time += dt;
            // Leave playback running. Wait for fade/path settling, then average a profile.
            if self.learning_time > 1.0 {
                self.profile_frames += 1;
                for i in 0..BANDS {
                    self.learning_microphone[i] += o.microphone[i];
                    self.learning_reference[i] += o.playback[i];
                }
            }
            if self.learning_time >= 4.0 && self.profile_frames > 0 {
                let scale = 10.0_f32.powf(self.last_ratio_db / 10.0);
                for i in 0..BANDS {
                    let mic = self.learning_microphone[i] / self.profile_frames as f32;
                    let output = self.learning_reference[i] / self.profile_frames as f32;
                    self.profile_reference[i] = output;
                    self.coupling[i] = if output > 1e-14 {
                        ((mic - self.baseline[i] * scale).max(0.0) / output * 1.12).min(100.0)
                    } else {
                        0.0
                    };
                }
                self.profile_ready = true;
            }
            self.status.state = "Learning speaker reference; playback continues";
            return self.hold(dt);
        }
        let total = self.baseline.iter().sum::<f32>();
        let mut ratios = [(0.0_f32, 0.0_f32); BANDS];
        let mut used = 0;
        let mut covered = 0.0;
        for i in 0..BANDS {
            if self.baseline[i] < total * 0.0001 {
                continue;
            }
            let leakage = if o.independent || !reference_present {
                0.0
            } else {
                self.coupling[i] * self.playback[i]
            };
            // Reject output-dominated/unprofiled bands, rather than amplifying uncertain residuals.
            let unknown_band = reference_present
                && !o.independent
                && self.profile_reference[i] < 1e-14
                && self.playback[i] > 1e-12;
            if unknown_band || leakage > self.microphone[i] * 0.65 {
                continue;
            }
            let residual = (self.microphone[i] - leakage).max(1e-15);
            ratios[used] = (
                10.0 * (residual / self.baseline[i]).log10(),
                self.baseline[i].min(total * 0.12),
            );
            covered += self.baseline[i];
            used += 1;
        }
        let coverage = covered / total.max(1e-15);
        if used < 3 || coverage < 0.25 {
            self.status.state = "Reducing: speaker sound dominates fan estimate";
            self.stable_seconds = 0.0;
            self.status.target_db = (self.status.gain_db - 0.75 * dt).max(MIN_DB);
            return self.finish(dt);
        }
        ratios[..used].sort_by(|a, b| a.0.total_cmp(&b.0));
        let weight = ratios[..used].iter().map(|v| v.1).sum::<f32>();
        let quantile = |q: f32| {
            let mut accumulated = 0.0;
            for &(db, w) in &ratios[..used] {
                accumulated += w;
                if accumulated >= weight * q {
                    return db;
                }
            }
            ratios[used - 1].0
        };
        if quantile(0.85) - quantile(0.15) > 12.0 {
            self.status.state = "Holding: inconsistent ambient sound";
            self.stable_seconds = 0.0;
            return self.hold(dt);
        }
        // A lower weighted quantile resists speech/music occupying only some bands.
        let ratio_db = quantile(0.35).clamp(-48.0, 30.0);
        if (ratio_db - self.candidate_db).abs() > 1.2 {
            self.candidate_db = ratio_db;
            self.stable_seconds = 0.0;
        } else {
            self.stable_seconds += dt;
        }
        self.status.confidence = coverage;
        if self.stable_seconds >= 1.5 {
            self.last_ratio_db = ratio_db;
            self.status.fan_dbfs = self.status.reference_dbfs + ratio_db;
            let target = (INITIAL_DB + ratio_db * 0.5).clamp(MIN_DB, 0.0);
            if (target - self.status.target_db).abs() > 0.6 {
                self.status.target_db = target;
            }
            self.status.state = "Following fan level";
        } else {
            self.status.state = "Waiting for a stable fan level";
            self.status.target_db = self.status.gain_db;
        }
        self.finish(dt)
    }
}
