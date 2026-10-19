use crate::analysis::Features;
/// Conservative level + change gate. Digital silence/noise-floor flux alone is insufficient.
pub struct InterferenceGate {
    baseline_db: f32,
    previous_time: f64,
    attack: f64,
    hold_until: f64,
}
impl Default for InterferenceGate {
    fn default() -> Self {
        Self {
            baseline_db: -70.0,
            previous_time: 0.0,
            attack: 0.0,
            hold_until: 0.0,
        }
    }
}
impl InterferenceGate {
    pub fn update(&mut self, f: &Features, time: f64, calibrating: bool, enabled: bool) -> f32 {
        let dt = (time - self.previous_time).clamp(0.0, 0.1);
        self.previous_time = time;
        if calibrating && f.dbfs.is_finite() {
            let weight = 1.0 - (-dt / 0.4).exp() as f32;
            self.baseline_db += weight * (f.dbfs - self.baseline_db);
        }
        if !enabled || calibrating {
            self.attack = 0.0;
            self.hold_until = 0.0;
            return 1.0;
        }
        let changing = f.flux > 0.65 || f.pitch_change.abs() > 900.0;
        let above_floor = f.dbfs > self.baseline_db + 10.0;
        let audible = f.dbfs > -52.0;
        if audible && above_floor && changing {
            self.attack += dt;
        } else {
            self.attack = (self.attack - dt * 2.0).max(0.0);
        }
        if self.attack >= 0.06 {
            self.hold_until = time + 0.8;
        }
        if time < self.hold_until { 0.2 } else { 1.0 }
    }
}
