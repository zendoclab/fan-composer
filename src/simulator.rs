use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scenario {
    Fixed,
    Ramp,
    Step,
    Harmonic,
    Turbulence,
    Multiple,
    Inharmonic,
}
impl Scenario {
    pub const ALL: [Self; 7] = [
        Self::Fixed,
        Self::Ramp,
        Self::Step,
        Self::Harmonic,
        Self::Turbulence,
        Self::Multiple,
        Self::Inharmonic,
    ];
}
impl std::fmt::Display for Scenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct Noise {
    state: u64,
}
impl Noise {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }
    pub fn next_signed(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        ((self.state >> 40) as f32 / 8388608.0) - 1.0
    }
}
pub struct FanSimulator {
    pub scenario: Scenario,
    rate: f32,
    sample: u64,
    phase: f32,
    phase2: f32,
    noise: Noise,
}
impl FanSimulator {
    pub fn new(scenario: Scenario, rate: f32, seed: u64) -> Self {
        Self {
            scenario,
            rate,
            sample: 0,
            phase: 0.0,
            phase2: 0.0,
            noise: Noise::new(seed),
        }
    }
    pub fn next_sample(&mut self) -> f32 {
        let time = self.sample as f32 / self.rate;
        self.sample += 1;
        let frequency = match self.scenario {
            Scenario::Ramp => 220.0 + (time * 8.0).min(400.0),
            Scenario::Step => {
                if time < 3.0 {
                    310.0
                } else {
                    510.0
                }
            }
            _ => 467.0,
        };
        self.phase = (self.phase + TAU * frequency / self.rate) % TAU;
        self.phase2 = (self.phase2 + TAU * 631.0 / self.rate) % TAU;
        let tone = 0.08 * self.phase.sin();
        let noise = self.noise.next_signed();
        match self.scenario {
            Scenario::Turbulence => noise * 0.07,
            Scenario::Multiple => tone + 0.065 * self.phase2.sin() + 0.015 * noise,
            Scenario::Inharmonic => {
                tone + 0.05 * (self.phase * 1.437).sin()
                    + 0.03 * (self.phase * 2.713).sin()
                    + 0.012 * noise
            }
            Scenario::Harmonic => {
                tone + 0.045 * (self.phase * 2.0).sin()
                    + 0.025 * (self.phase * 3.0).sin()
                    + 0.008 * noise
            }
            _ => tone + 0.009 * noise,
        }
    }
}
