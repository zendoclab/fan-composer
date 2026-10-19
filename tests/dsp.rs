use fan_composer::{
    analysis::{Analyzer, Features},
    simulator::{FanSimulator, Scenario},
    synth::{Mode, Parameters, Synth},
};
fn analyze(scenario: Scenario, n: usize, seconds: usize) -> Features {
    let mut a = Analyzer::new(48000.0, n);
    let mut fan = FanSimulator::new(scenario, 48000.0, 255);
    let mut last = Features::default();
    for _ in 0..48000 * seconds {
        if let Some(f) = a.push(fan.next_sample()) {
            last = f;
        }
    }
    last
}
#[test]
fn interpolated_tone_accuracy_at_both_fft_sizes() {
    for n in [2048, 4096] {
        let f = analyze(Scenario::Fixed, n, 3);
        assert!((f.peak_hz - 467.0).abs() < 1.0, "{n}: {}", f.peak_hz);
        assert!(f.confidence > 0.45);
        assert!(f.dbfs < 0.0);
    }
}
#[test]
fn harmonic_evidence_and_noise_rejection() {
    let harmonic = analyze(Scenario::Harmonic, 4096, 3);
    let noise = analyze(Scenario::Turbulence, 4096, 3);
    assert!(harmonic.harmonicity > 0.8);
    assert!(noise.flatness > 0.4);
    assert!(noise.confidence < 0.35);
    assert_eq!(noise.fan_hz, 0.0);
}
#[test]
fn step_and_ramp_reacquire() {
    let step = analyze(Scenario::Step, 4096, 6);
    assert!((step.peak_hz - 510.0).abs() < 1.0);
    assert!(step.confidence > 0.45);
    let ramp = analyze(Scenario::Ramp, 4096, 6);
    assert!((ramp.peak_hz - 268.0).abs() < 3.0);
}
#[test]
fn all_simulator_cases_are_finite_and_bounded() {
    for scenario in Scenario::ALL {
        let f = analyze(scenario, 2048, 2);
        assert!(f.rms.is_finite() && f.dbfs.is_finite() && f.centroid.is_finite());
        assert!(f.rms < 0.3);
    }
}
#[test]
fn silence_and_corrupt_input_do_not_create_fan() {
    let mut a = Analyzer::new(48000.0, 4096);
    let mut last = Features::default();
    for i in 0..96000 {
        if let Some(f) = a.push(if i % 19 == 0 { f32::NAN } else { 0.0 }) {
            last = f;
        }
    }
    assert_eq!(last.confidence, 0.0);
    assert_eq!(last.fan_hz, 0.0);
    assert!(last.dbfs.is_finite());
}
#[test]
fn synth_modes_are_distinct_repeatable_and_limited() {
    let mut energies = vec![];
    for mode in Mode::ALL {
        let mut a = Synth::new(48000.0, 41);
        let mut b = Synth::new(48000.0, 41);
        let p = Parameters {
            mode,
            ..Default::default()
        };
        a.set_parameters(p);
        b.set_parameters(p);
        let mut energy = 0.0;
        for _ in 0..48000 * 5 {
            let x = a.next_frame(0.04, 0.03);
            let y = b.next_frame(0.04, 0.03);
            assert_eq!(x, y);
            for sample in x {
                assert!(sample.is_finite() && sample.abs() <= 0.03);
                energy += sample * sample;
            }
        }
        energies.push(energy);
        assert!(energy > 0.01);
    }
    assert!((energies[0] - energies[1]).abs() > 0.1);
    assert!((energies[0] - energies[2]).abs() > 0.1);
}
#[test]
fn start_stop_switch_and_invalid_controls_are_safe() {
    let mut s = Synth::new(48000.0, 1);
    for _ in 0..48000 {
        assert_eq!(s.next_frame(0.0, 0.1), [0.0; 2]);
    }
    let mut previous = 0.0_f32;
    let mut max_step = 0.0_f32;
    for i in 0..48000 * 6 {
        if i == 48000 * 2 {
            s.set_parameters(Parameters {
                mode: Mode::Nature,
                root_hz: 500.0,
                ..Default::default()
            });
        }
        if i == 48000 * 3 {
            s.set_parameters(Parameters {
                mode: Mode::Generative,
                root_hz: 65.0,
                ..Default::default()
            });
        }
        let x = s.next_frame(if i > 48000 * 5 { 0.0 } else { 0.08 }, 0.08)[0];
        max_step = max_step.max((x - previous).abs());
        previous = x;
    }
    assert!(max_step < 0.035, "click-sized discontinuity {max_step}");
    assert!(s.gain() < 1e-8);
    s.set_parameters(Parameters {
        root_hz: f32::NAN,
        brightness: f32::INFINITY,
        density: f32::NAN,
        preference: f32::NAN,
        ..Default::default()
    });
    for _ in 0..48000 {
        assert!(
            s.next_frame(f32::NAN, f32::NAN)
                .iter()
                .all(|s| s.is_finite())
        );
    }
}

#[test]
fn strong_second_harmonic_does_not_replace_observed_fundamental() {
    let mut a = Analyzer::new(48000.0, 4096);
    let mut last = Features::default();
    for i in 0..48000 * 3 {
        let phase = std::f32::consts::TAU * 233.0 * i as f32 / 48000.0;
        let signal = 0.035 * phase.sin() + 0.12 * (phase * 2.0).sin() + 0.055 * (phase * 3.0).sin();
        if let Some(f) = a.push(signal) {
            last = f;
        }
    }
    assert!((last.peak_hz - 466.0).abs() < 1.0);
    assert!(
        (last.fan_hz - 233.0).abs() < 1.0,
        "fundamental {}",
        last.fan_hz
    );
}
