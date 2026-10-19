use std::f32::consts::TAU;
pub struct SpeakerTest {
    rate: f32,
    left: u32,
    total: u32,
    phase: f32,
}
impl SpeakerTest {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            left: 0,
            total: (rate * 2.0) as u32,
            phase: 0.0,
        }
    }
    pub fn trigger(&mut self) {
        self.left = self.total;
        self.phase = 0.0;
    }
    pub fn stop(&mut self) {
        self.left = self.left.min((self.rate * 0.04) as u32);
    }
    pub fn active(&self) -> bool {
        self.left > 0
    }
    pub fn next_sample(&mut self, gain: f32, cap: f32) -> f32 {
        if self.left == 0 {
            return 0.0;
        }
        let fade = self.rate * 0.04;
        let progress = (self.total - self.left) as f32;
        let envelope = (progress / fade).min(1.0) * (self.left as f32 / fade).min(1.0);
        self.left -= 1;
        self.phase = (self.phase + TAU * 500.0 / self.rate) % TAU;
        let amplitude = if gain.is_finite() {
            (gain.max(0.0) * 0.8).min(0.06)
        } else {
            0.0
        };
        crate::level::limit(self.phase.sin() * amplitude * envelope, cap)
    }
}
