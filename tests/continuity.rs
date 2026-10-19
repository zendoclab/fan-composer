use fan_composer::{
    calibration::CalibrationSchedule,
    level::DEFAULT_GAIN,
    settings::Settings,
    synth::{Mode, Parameters, Synth},
};
use std::time::Duration;
#[test]
fn startup_and_manual_calibration_are_explicit_and_finite() {
    let mut s = CalibrationSchedule::default();
    assert!(s.update(Duration::ZERO, false, false));
    assert!(!s.update(Duration::from_secs(4), false, false));
    assert!(!s.update(Duration::from_secs(60), false, false));
    assert!(s.update(Duration::from_secs(65), true, false));
    assert!(s.update(Duration::from_millis(68499), false, false));
    assert!(!s.update(Duration::from_millis(68500), false, false));
    assert!(!Settings::default().periodic_calibration);
}
#[test]
fn periodic_quiet_measurement_requires_opt_in() {
    let mut s = CalibrationSchedule::default();
    s.update(Duration::ZERO, false, true);
    assert!(!s.update(Duration::from_secs(29), false, true));
    assert!(s.update(Duration::from_secs(30), false, true));
    assert!(!s.update(Duration::from_millis(33500), false, true));
    s.update(Duration::from_secs(40), false, false);
    assert!(!s.update(Duration::from_secs(100), false, true));
    assert!(s.update(Duration::from_secs(130), false, true));
}
#[test]
fn all_modes_remain_non_silent_across_old_30_second_boundaries() {
    for mode in Mode::ALL {
        let mut schedule = CalibrationSchedule::default();
        let mut synth = Synth::new(48000.0, 255);
        synth.set_parameters(Parameters {
            mode,
            ..Default::default()
        });
        let mut energy = 0.0_f64;
        let mut frames = 0;
        for block in 0..10000 {
            // 100 virtual seconds, not a real-time endurance claim
            let quiet = schedule.update(Duration::from_millis(block * 10), false, false);
            for _ in 0..480 {
                let frame = synth.next_frame(if quiet { 0.0 } else { DEFAULT_GAIN }, 0.08);
                energy += f64::from(frame[0]).powi(2);
                frames += 1;
            }
            if block % 40 == 39 {
                let rms = (energy / frames as f64).sqrt();
                if block >= 499 {
                    assert!(
                        rms > 0.002,
                        "{mode:?}, at {} seconds: {rms}",
                        block as f64 / 100.0
                    );
                }
                energy = 0.0;
                frames = 0;
            }
        }
    }
}
#[test]
fn policy_changes_are_bounded_and_keep_synthesis_finite() {
    let mut synth = Synth::new(48000.0, 255);
    for t in 0..12 {
        let p = Parameters {
            mode: Mode::ALL[t % 3],
            overtones: if t % 2 == 0 { 0.6 } else { 1.4 },
            motion: if t % 2 == 0 { 0.5 } else { 1.5 },
            wet: 0.25,
            stereo: 1.0,
            ..Default::default()
        };
        synth.set_parameters(p);
        for _ in 0..48000 {
            for s in synth.next_frame(DEFAULT_GAIN, 0.08) {
                assert!(s.is_finite() && s.abs() <= 0.080001);
            }
        }
    }
}
