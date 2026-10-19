use fan_composer::{
    analysis::Features,
    duck::InterferenceGate,
    level,
    speaker_test::SpeakerTest,
    synth::{Mode, Parameters, Synth},
};
#[test]
fn low_level_spectral_jitter_does_not_duck_quiet_fan() {
    let mut gate = InterferenceGate::default();
    let f = Features {
        dbfs: -72.0,
        flux: 2.0,
        pitch_change: 5000.0,
        ..Default::default()
    };
    for i in 1..150 {
        assert_eq!(gate.update(&f, i as f64 * 0.02, true, true), 1.0);
    }
    for i in 150..3150 {
        assert_eq!(gate.update(&f, i as f64 * 0.02, false, true), 1.0);
    }
}
#[test]
fn sustained_louder_interference_ducks_then_releases() {
    let mut gate = InterferenceGate::default();
    let floor = Features {
        dbfs: -70.0,
        ..Default::default()
    };
    for i in 1..151 {
        gate.update(&floor, i as f64 * 0.02, true, true);
    }
    let voice = Features {
        dbfs: -32.0,
        flux: 0.9,
        pitch_change: 1800.0,
        ..Default::default()
    };
    let mut gain = 1.0;
    for i in 151..161 {
        gain = gate.update(&voice, i as f64 * 0.02, false, true);
    }
    assert_eq!(gain, 0.2);
    assert_eq!(gate.update(&floor, 3.4, false, true), 0.2);
    assert_eq!(gate.update(&floor, 4.2, false, true), 1.0);
    assert_eq!(gate.update(&voice, 4.22, false, false), 1.0);
}
#[test]
fn level_slider_covers_audible_adjustment_and_exact_mute() {
    for db in [-48.0, -32.0, -24.0, -18.0, -6.0] {
        assert!((level::gain_to_db(level::db_to_gain(db)) - db).abs() < 0.001);
    }
    assert_eq!(level::db_to_gain(-60.0), 0.0);
    assert_eq!(level::db_to_gain(f32::NAN), 0.0);
    assert!((level::gain_to_db(level::DEFAULT_GAIN) + 18.0).abs() < 0.001);
}
#[test]
fn default_modes_have_measurable_output_above_the_quiet_regression() {
    for mode in Mode::ALL {
        let mut synth = Synth::new(48000.0, 255);
        synth.set_parameters(Parameters {
            mode,
            ..Default::default()
        });
        let mut energy = 0.0_f64;
        let mut count = 0_u64;
        let mut peak = 0.0_f32;
        for i in 0..48000 * 20 {
            for sample in synth.next_frame(level::DEFAULT_GAIN, 0.08) {
                if i > 48000 * 5 {
                    energy += f64::from(sample).powi(2);
                    count += 1;
                    peak = peak.max(sample.abs());
                }
            }
        }
        let rms = (energy / count as f64).sqrt() as f32;
        println!(
            "{mode:?}: RMS {:.2} dBFS, peak {:.2} dBFS",
            level::gain_to_db(rms),
            level::gain_to_db(peak)
        );
        assert!(rms > 0.007 && rms < 0.04, "{mode:?} rms {rms}");
        assert!(peak <= 0.08);
    }
}
#[test]
fn explicit_speaker_test_is_bounded_and_ends_after_two_seconds() {
    let mut tone = SpeakerTest::new(48000.0);
    assert!(!tone.active());
    assert_eq!(tone.next_sample(level::DEFAULT_GAIN, 0.08), 0.0);
    tone.trigger();
    let mut maximum = 0.0_f32;
    let mut jumps = 0.0_f32;
    let mut previous = 0.0;
    for _ in 0..96000 {
        let sample = tone.next_sample(level::DEFAULT_GAIN, 0.08);
        maximum = maximum.max(sample.abs());
        jumps = jumps.max((sample - previous).abs());
        previous = sample;
    }
    assert!(!tone.active());
    assert_eq!(tone.next_sample(level::DEFAULT_GAIN, 0.08), 0.0);
    assert!(maximum > 0.02 && maximum < 0.06);
    assert!(jumps < 0.004);
    tone.trigger();
    for _ in 0..8000 {
        tone.next_sample(level::DEFAULT_GAIN, 0.08);
    }
    tone.stop();
    for _ in 0..2000 {
        tone.next_sample(level::DEFAULT_GAIN, 0.08);
    }
    assert!(!tone.active());
}
