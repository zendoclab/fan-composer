use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use serde::Serialize;
use std::f32::consts::TAU;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct Features {
    pub rms: f32,
    pub dbfs: f32,
    pub peak_hz: f32,
    pub fan_hz: f32,
    pub centroid: f32,
    pub flatness: f32,
    pub tonality: f32,
    pub harmonicity: f32,
    pub broadband: f32,
    pub noise_floor_dbfs: f32,
    pub pitch_change: f32,
    pub level_change: f32,
    pub confidence: f32,
    pub flux: f32,
    pub spectrum: Vec<f32>,
}
impl Default for Features {
    fn default() -> Self {
        Self {
            rms: 0.0,
            dbfs: -120.0,
            peak_hz: 0.0,
            fan_hz: 0.0,
            centroid: 0.0,
            flatness: 0.0,
            tonality: 0.0,
            harmonicity: 0.0,
            broadband: 0.0,
            noise_floor_dbfs: -120.0,
            pitch_change: 0.0,
            level_change: 0.0,
            confidence: 0.0,
            flux: 0.0,
            spectrum: vec![-120.0; 96],
        }
    }
}

pub struct Analyzer {
    rate: f32,
    n: usize,
    hop: usize,
    ring: Vec<f32>,
    cursor: usize,
    total: usize,
    window: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex32>,
    scratch: Vec<Complex32>,
    power: Vec<f32>,
    previous: Vec<f32>,
    last_peak: f32,
    stable: f32,
    previous_db: f32,
    noise_floor: f32,
}
impl Analyzer {
    pub fn new(rate: f32, n: usize) -> Self {
        assert!(matches!(n, 2048 | 4096));
        assert!(rate.is_finite() && rate > 4000.0);
        let fft = FftPlanner::new().plan_fft_forward(n);
        let scratch = vec![Complex32::default(); fft.get_inplace_scratch_len()];
        Self {
            rate,
            n,
            hop: n / 4,
            ring: vec![0.0; n],
            cursor: 0,
            total: 0,
            window: (0..n)
                .map(|i| 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos())
                .collect(),
            fft,
            buffer: vec![Complex32::default(); n],
            scratch,
            power: vec![0.0; n / 2 + 1],
            previous: vec![0.0; n / 2 + 1],
            last_peak: 0.0,
            stable: 0.0,
            previous_db: -120.0,
            noise_floor: 1e-12,
        }
    }
    pub fn band_powers(&self) -> crate::auto_volume::Powers {
        let mut bands = [0.0; crate::auto_volume::BANDS];
        for (bin, power) in self.power.iter().enumerate().skip(1) {
            if let Some(band) =
                crate::auto_volume::band_index(bin as f32 * self.rate / self.n as f32)
            {
                bands[band] += power * (4.0 / 3.0);
            }
        }
        bands
    }
    pub fn push(&mut self, sample: f32) -> Option<Features> {
        self.ring[self.cursor] = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.cursor = (self.cursor + 1) % self.n;
        self.total += 1;
        if self.total < self.n || !(self.total - self.n).is_multiple_of(self.hop) {
            return None;
        }
        Some(self.analyze())
    }
    fn analyze(&mut self) -> Features {
        let mut energy = 0.0;
        for i in 0..self.n {
            let s = self.ring[(self.cursor + i) % self.n];
            energy += s * s;
            self.buffer[i] = Complex32::new(s * self.window[i], 0.0);
        }
        let rms = (energy / self.n as f32).sqrt();
        let dbfs = 20.0 * rms.max(1e-6).log10();
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        let norm = 4.0 / (self.n as f32 * self.n as f32);
        for (i, p) in self.power.iter_mut().enumerate() {
            *p = self.buffer[i].norm_sqr() * norm;
        }
        let lo = ((70.0 * self.n as f32 / self.rate) as usize).max(2);
        let hi = ((4000.0 * self.n as f32 / self.rate) as usize).min(self.n / 2 - 2);
        let mut total = 0.0;
        let mut weighted = 0.0;
        let mut logs = 0.0;
        let mut peak = lo;
        let mut flux = 0.0;
        for i in lo..=hi {
            let p = self.power[i].max(1e-15);
            total += p;
            weighted += p * i as f32 * self.rate / self.n as f32;
            logs += p.ln();
            flux += (p - self.previous[i]).abs();
            if p > self.power[peak] {
                peak = i;
            }
        }
        let count = (hi - lo + 1) as f32;
        let flatness = ((logs / count).exp() / (total / count).max(1e-15)).clamp(0.0, 1.0);
        let interpolate = |bin: usize| {
            let y0 = self.power[bin - 1].max(1e-15).ln();
            let y1 = self.power[bin].max(1e-15).ln();
            let y2 = self.power[bin + 1].max(1e-15).ln();
            let denominator = y0 - 2.0 * y1 + y2;
            let delta = if denominator.abs() > 1e-10 {
                (0.5 * (y0 - y2) / denominator).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            (bin as f32 + delta) * self.rate / self.n as f32
        };
        let peak_hz = interpolate(peak);
        let local_energy = |bin: usize| self.power[bin - 1..=bin + 1].iter().sum::<f32>();
        // Score observed fundamentals against their harmonic families. An absent subharmonic
        // is never invented simply by dividing the strongest peak by an integer.
        let mut candidate_bin = peak;
        let mut best_score = 0.0;
        for bin in lo..=hi {
            if self.power[bin] < self.power[bin - 1]
                || self.power[bin] < self.power[bin + 1]
                || local_energy(bin) < total * 0.012
            {
                continue;
            }
            let hz = interpolate(bin);
            let mut score = local_energy(bin);
            for harmonic in 2..=5 {
                let other = (hz * harmonic as f32 * self.n as f32 / self.rate).round() as usize;
                if other < hi {
                    score += local_energy(other);
                }
            }
            if score > best_score {
                best_score = score;
                candidate_bin = bin;
            }
        }
        let candidate_hz = interpolate(candidate_bin);
        let concentration = local_energy(peak) / total.max(1e-15);
        let mut harmonic_energy = local_energy(candidate_bin);
        let mut harmonics = 0;
        for h in 2..=5 {
            let bin = (candidate_hz * h as f32 * self.n as f32 / self.rate).round() as usize;
            if bin < hi {
                let energy = local_energy(bin);
                harmonic_energy += energy;
                if energy > total * 0.012 {
                    harmonics += 1;
                }
            }
        }
        let harmonicity = (harmonic_energy / total.max(1e-15)).clamp(0.0, 1.0);
        let relative = (candidate_hz - self.last_peak).abs() / candidate_hz.max(70.0);
        let dt = self.hop as f32 / self.rate;
        if relative < 0.025 {
            self.stable = (self.stable + dt / 1.2).min(1.0);
        } else {
            self.stable *= 0.35;
        }
        let pitch_change = if self.last_peak > 0.0 {
            (candidate_hz - self.last_peak) / dt
        } else {
            0.0
        };
        self.last_peak = candidate_hz;
        let flux = (flux / total.max(1e-15)).min(2.0);
        let tonality = ((1.0 - flatness) * concentration.sqrt()).clamp(0.0, 1.0);
        // Stable single tones remain candidates; harmonic support raises confidence, never establishes source identity.
        let confidence = if dbfs < -85.0 {
            0.0
        } else {
            (self.stable
                * tonality
                * (0.65 + 0.35 * (harmonics as f32 / 2.0).min(1.0))
                * (1.0 - flux * 0.25).max(0.0))
            .clamp(0.0, 1.0)
        };
        let broadband = (total * (1.0 - harmonicity)).sqrt();
        let mut floor_bins = self.power[lo..=hi].to_vec();
        let middle = floor_bins.len() / 2;
        let (_, median, _) = floor_bins.select_nth_unstable_by(middle, |a, b| a.total_cmp(b));
        let background = *median;
        self.noise_floor += if background < self.noise_floor {
            0.15
        } else {
            0.005
        } * (background - self.noise_floor);
        let level_change = (dbfs - self.previous_db) / dt;
        self.previous_db = dbfs;
        self.previous.copy_from_slice(&self.power);
        let mut spectrum = vec![-120.0; 96];
        for (b, v) in spectrum.iter_mut().enumerate() {
            let f0 = 40.0 * (500.0_f32).powf(b as f32 / 96.0);
            let f1 = 40.0 * (500.0_f32).powf((b + 1) as f32 / 96.0);
            let a = (f0 * self.n as f32 / self.rate) as usize;
            let z = ((f1 * self.n as f32 / self.rate) as usize)
                .max(a + 1)
                .min(self.power.len());
            if a < z {
                *v = 10.0
                    * self.power[a..z]
                        .iter()
                        .copied()
                        .fold(1e-12_f32, f32::max)
                        .log10();
            }
        }
        Features {
            rms,
            dbfs,
            peak_hz,
            fan_hz: if confidence > 0.35 { candidate_hz } else { 0.0 },
            centroid: weighted / total.max(1e-15),
            flatness,
            tonality,
            harmonicity,
            broadband,
            noise_floor_dbfs: 10.0 * self.noise_floor.max(1e-12).log10(),
            pitch_change,
            level_change,
            confidence,
            flux,
            spectrum,
        }
    }
}
