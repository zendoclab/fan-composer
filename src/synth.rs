use crate::simulator::Noise;
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Harmonic,
    Nature,
    Generative,
}
impl Mode {
    pub const ALL: [Self; 3] = [Self::Harmonic, Self::Nature, Self::Generative];
}
impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub mode: Mode,
    pub root_hz: f32,
    pub brightness: f32,
    pub density: f32,
    pub preference: f32,
    pub adaptation: f32,
    pub overtones: f32,
    pub motion: f32,
    pub wet: f32,
    pub stereo: f32,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            mode: Mode::Harmonic,
            root_hz: 146.832,
            brightness: 0.4,
            density: 0.25,
            preference: 0.0,
            adaptation: 0.25,
            overtones: 1.0,
            motion: 1.0,
            wet: 0.18,
            stereo: 0.8,
        }
    }
}

pub struct Synth {
    rate: f32,
    phase: [f32; 6],
    lfo: [f32; 2],
    overtones: f32,
    motion: f32,
    wet: f32,
    stereo: f32,
    root: f32,
    params: Parameters,
    mix: [f32; 3],
    gain: f32,
    noise: Noise,
    brown: [f32; 2],
    pink: [f32; 2],
    highpass: [f32; 2],
    highpass_coeff: f32,
    low: [f32; 2],
    time: f64,
    note_clock: f32,
    note_env: f32,
    note_ratio: f32,
    step: usize,
    delay: Vec<[f32; 2]>,
    delay_pos: usize,
    attack: f32,
    release: f32,
    root_coeff: f32,
    mix_coeff: f32,
    low_coeff: f32,
}
impl Synth {
    pub fn new(rate: f32, seed: u64) -> Self {
        Self {
            rate,
            phase: [0.0; 6],
            lfo: [0.0; 2],
            overtones: 1.0,
            motion: 1.0,
            wet: 0.18,
            stereo: 0.8,
            root: 146.832,
            params: Parameters::default(),
            mix: [1.0, 0.0, 0.0],
            gain: 0.0,
            noise: Noise::new(seed),
            brown: [0.0; 2],
            pink: [0.0; 2],
            highpass: [0.0; 2],
            highpass_coeff: 1.0 - (-TAU * 120.0 / rate).exp(),
            low: [0.0; 2],
            time: 0.0,
            note_clock: 0.0,
            note_env: 0.0,
            note_ratio: 1.0,
            step: 0,
            delay: vec![[0.0; 2]; (rate * 0.31) as usize],
            delay_pos: 0,
            attack: 1.0 - (-1.0 / (0.25 * rate)).exp(),
            release: 1.0 - (-1.0 / (0.020 * rate)).exp(),
            root_coeff: 1.0 - (-1.0 / (6.5 * rate)).exp(),
            mix_coeff: 1.0 / rate,
            low_coeff: 1.0 - (-TAU * 1180.0 / rate).exp(),
        }
    }
    pub fn set_parameters(&mut self, p: Parameters) {
        self.params = Parameters {
            mode: p.mode,
            root_hz: if p.root_hz.is_finite() {
                p.root_hz.clamp(65.0, 500.0)
            } else {
                146.832
            },
            brightness: finite_unit(p.brightness),
            density: finite_unit(p.density),
            preference: if p.preference.is_finite() {
                p.preference.clamp(-1.0, 1.0)
            } else {
                0.0
            },
            adaptation: finite_unit(p.adaptation),
            overtones: if p.overtones.is_finite() {
                p.overtones.clamp(0.6, 1.4)
            } else {
                1.0
            },
            motion: if p.motion.is_finite() {
                p.motion.clamp(0.5, 1.5)
            } else {
                1.0
            },
            wet: if p.wet.is_finite() {
                p.wet.clamp(0.05, 0.25)
            } else {
                0.18
            },
            stereo: if p.stereo.is_finite() {
                p.stereo.clamp(0.4, 1.0)
            } else {
                0.8
            },
        };
        self.low_coeff = 1.0 - (-TAU * (300.0 + 2200.0 * self.params.brightness) / self.rate).exp();
        self.root_coeff =
            1.0 - (-1.0 / ((0.5 + 8.0 * (1.0 - self.params.adaptation)) * self.rate)).exp();
    }
    pub fn gain(&self) -> f32 {
        self.gain
    }
    pub fn next_frame(&mut self, target_gain: f32, peak_limit: f32) -> [f32; 2] {
        let target = finite_unit(target_gain).min(0.5011872);
        let coefficient = if target < self.gain {
            self.release
        } else {
            self.attack
        };
        self.gain += (target - self.gain) * coefficient;
        self.root += (self.params.root_hz - self.root) * self.root_coeff;
        let selected = match self.params.mode {
            Mode::Harmonic => 0,
            Mode::Nature => 1,
            Mode::Generative => 2,
        };
        for i in 0..3 {
            self.mix[i] += ((if i == selected { 1.0 } else { 0.0 }) - self.mix[i]) * self.mix_coeff;
        }
        self.time += 1.0 / self.rate as f64;
        self.overtones += (self.params.overtones - self.overtones) * self.mix_coeff;
        self.motion += (self.params.motion - self.motion) * self.mix_coeff;
        self.wet += (self.params.wet - self.wet) * self.mix_coeff;
        self.stereo += (self.params.stereo - self.stereo) * self.mix_coeff;
        self.lfo[0] = (self.lfo[0] + TAU * self.motion / (13.0 * self.rate)) % TAU;
        self.lfo[1] = (self.lfo[1] + TAU * self.motion / (9.7 * self.rate)) % TAU;
        let ratios = [0.5, 1.0, 1.5, 2.0, self.note_ratio, 3.0];
        for (i, ratio) in ratios.iter().enumerate() {
            self.phase[i] = (self.phase[i] + TAU * self.root * ratio / self.rate) % TAU;
        }
        let breath = 0.87 + 0.13 * self.lfo[0].sin();
        let drone = (0.06 * self.phase[0].sin()
            + 0.22 * self.phase[1].sin()
            + 0.20 * self.phase[2].sin()
            + 0.28 * self.phase[3].sin()
            + 0.18 * self.overtones * self.phase[5].sin())
            * breath;
        self.note_clock -= 1.0 / self.rate;
        if self.note_clock <= 0.0 {
            const MOTIF: [i32; 12] = [0, 7, 12, 7, 5, 7, 0, 3, 7, 5, 3, 0];
            let degree = MOTIF[self.step % MOTIF.len()];
            self.step += 1;
            self.note_ratio = 2.0 * 2.0_f32.powf(degree as f32 / 12.0);
            self.note_env = 0.0;
            self.note_clock = 2.5 + (1.0 - self.params.density) * 4.0;
        }
        self.note_env = (self.note_env + 1.0 / (self.rate * 0.8)).min(1.0);
        let note_amp = self.note_env * (self.note_clock / 1.5).clamp(0.0, 1.0);
        let generated = (drone * 0.45 + self.phase[4].sin() * note_amp * 0.32) * 1.4;
        let wave = 0.3 + 0.7 * (0.5 + 0.5 * self.lfo[1].sin()).powi(2);
        let mut out = [0.0; 2];
        for (ch, dst) in out.iter_mut().enumerate() {
            let white = self.noise.next_signed();
            self.brown[ch] = (self.brown[ch] * 0.9992 + white * 0.025).clamp(-1.0, 1.0);
            self.pink[ch] = self.pink[ch] * 0.97 + white * 0.03;
            self.low[ch] += self.low_coeff * (white - self.low[ch]);
            self.highpass[ch] += self.highpass_coeff * (self.low[ch] - self.highpass[ch]);
            let mid = self.low[ch] - self.highpass[ch];
            let nature = (self.brown[ch] * 0.25 + self.pink[ch] * 0.8 + mid * 0.65) * wave * 2.4;
            let dry = drone * self.mix[0] + nature * self.mix[1] + generated * self.mix[2];
            let old = self.delay[self.delay_pos][ch];
            self.delay[self.delay_pos][ch] = dry * 0.12 + old * 0.35;
            let raw = (dry + old * self.wet) * self.gain;
            *dst = crate::level::limit(raw, peak_limit);
        }
        let mid = (out[0] + out[1]) * 0.5;
        for sample in &mut out {
            *sample = mid + (*sample - mid) * self.stereo;
        }
        self.delay_pos = (self.delay_pos + 1) % self.delay.len();
        out
    }
}
fn finite_unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
