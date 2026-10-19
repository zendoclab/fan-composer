use fan_composer::{
    auto_volume::{
        AdaptiveVolume, BANDS, INITIAL_DB, MIN_DB, Observation, Powers, gain_from_db, limited_gain,
    },
    settings::Settings,
    synth::{Mode, Parameters, Synth},
};
fn fan() -> Powers {
    std::array::from_fn(|i| 2e-8 * (1.0 + (i % 7) as f32 * 0.2))
}
fn reference(mode: Mode) -> Powers {
    std::array::from_fn(|i| {
        let color = match mode {
            Mode::Harmonic => {
                if i % 4 == 0 {
                    2.0
                } else {
                    0.1
                }
            }
            Mode::Nature => 1.0,
            Mode::Generative => {
                if i % 3 == 0 {
                    1.6
                } else {
                    0.2
                }
            }
        };
        fan()[i] * 100.0 * color
    })
}
fn observation(mode: Mode, fan_ratio: f32, output: Powers, quiet: bool) -> Observation {
    Observation {
        microphone: std::array::from_fn(|i| fan()[i] * fan_ratio + output[i] * 0.002),
        playback: output,
        quiet,
        trusted_quiet: quiet,
        independent: false,
        reference_valid: true,
        mode,
        seconds: 0.02,
    }
}
fn calibrated(mode: Mode) -> AdaptiveVolume {
    let mut controller = AdaptiveVolume::default();
    for _ in 0..150 {
        controller.update(observation(mode, 1.0, [0.0; BANDS], true));
    }
    for _ in 0..350 {
        controller.update(observation(mode, 1.0, reference(mode), false));
    }
    controller
}
#[test]
fn all_modes_follow_sustained_fan_increase_and_decrease_with_closed_loop_playback() {
    for mode in Mode::ALL {
        let mut c = calibrated(mode);
        let initial = c.status.gain_db;
        let mut last = initial;
        for _ in 0..1800 {
            let output =
                reference(mode).map(|p| p * 10.0_f32.powf((c.status.gain_db - INITIAL_DB) / 10.0));
            let s = c.update(observation(mode, 15.8489, output, false));
            assert!(s.gain_db <= 0.0 && s.gain_db >= MIN_DB);
            assert!((s.gain_db - last).abs() <= 0.031);
            last = s.gain_db;
        }
        assert!(c.status.gain_db > initial + 4.5, "{mode:?}: {:?}", c.status);
        for _ in 0..2600 {
            let output =
                reference(mode).map(|p| p * 10.0_f32.powf((c.status.gain_db - INITIAL_DB) / 10.0));
            c.update(observation(mode, 0.0630957, output, false));
        }
        assert!(c.status.gain_db < initial - 3.5, "{mode:?}: {:?}", c.status);
    }
}
#[test]
fn own_speaker_level_increase_never_causes_automatic_amplification() {
    for mode in Mode::ALL {
        let mut c = calibrated(mode);
        let before = c.status.gain_db;
        for _ in 0..1600 {
            let output = reference(mode)
                .map(|p| p * 16.0 * 10.0_f32.powf((c.status.gain_db - INITIAL_DB) / 10.0));
            c.update(observation(mode, 1.0, output, false));
        }
        assert!(
            c.status.gain_db <= before + 0.35,
            "{mode:?}: {:?}",
            c.status
        );
    }
}
#[test]
fn short_interference_and_partial_band_voice_do_not_raise_volume() {
    let mut c = calibrated(Mode::Nature);
    let before = c.status.gain_db;
    for i in 0..1200 {
        let mut o = observation(Mode::Nature, 1.0, reference(Mode::Nature), false);
        if i < 30 {
            for v in &mut o.microphone {
                *v *= 16.0;
            }
        } else {
            for v in &mut o.microphone[6..12] {
                *v *= 40.0;
            }
        }
        c.update(o);
    }
    assert!(c.status.gain_db <= before + 0.35, "{:?}", c.status);
}
#[test]
fn invalid_or_missing_reference_cannot_raise_pending_gain() {
    let mut c = calibrated(Mode::Harmonic);
    for _ in 0..200 {
        c.update(observation(
            Mode::Harmonic,
            16.0,
            reference(Mode::Harmonic),
            false,
        ));
    }
    let before = c.status.gain_db;
    for _ in 0..800 {
        let mut o = observation(Mode::Harmonic, 16.0, reference(Mode::Harmonic), false);
        o.reference_valid = false;
        c.update(o);
    }
    assert!((c.status.gain_db - before).abs() < 0.001);
    let mut o = observation(Mode::Harmonic, 1.0, reference(Mode::Harmonic), false);
    o.microphone[0] = f32::NAN;
    assert!(c.update(o).gain_db.is_finite());
}
#[test]
fn maximum_mute_peak_and_no_complete_silence_during_normal_tracking_are_preserved() {
    for mode in Mode::ALL {
        let mut synth = Synth::new(48000.0, 255);
        synth.set_parameters(Parameters {
            mode,
            ..Default::default()
        });
        for db in [-30.0, -18.0, -6.0, 0.0] {
            let maximum = 0.25;
            let gain = limited_gain(maximum, gain_from_db(db), 1.0);
            assert!(gain <= maximum);
            let mut energy = 0.0;
            for _ in 0..48000 {
                for s in synth.next_frame(gain, 0.08) {
                    assert!(s.is_finite() && s.abs() <= 0.080001);
                    energy += s * s;
                }
            }
            assert!(energy > 0.0);
        }
    }
    assert_eq!(limited_gain(0.0, 1.0, 1.0), 0.0);
    assert_eq!(limited_gain(0.3, 10.0, 1.0), 0.3);
    assert_eq!(limited_gain(f32::NAN, 1.0, 1.0), 0.0);
}
#[test]
fn existing_fixed_volume_settings_migrate_to_visible_maximum_without_a_starting_level_jump() {
    let old = Settings::from_json(br#"{"volume":0.12589254,"max_peak":0.08}"#);
    assert_eq!(old.schema_version, 3);
    assert!((old.volume * gain_from_db(INITIAL_DB) - 0.12589254).abs() < 0.001);
    let serialized = serde_json::to_vec(&old).unwrap();
    assert!((Settings::from_json(&serialized).volume - old.volume).abs() < 1e-6);
}

#[test]
fn actual_fft_and_stereo_reference_follow_fan_steps_in_all_modes() {
    use fan_composer::{
        analysis::Analyzer,
        auto_volume::{ReferenceFrame, ReferenceMeter},
        simulator::Noise,
    };
    for mode in Mode::ALL {
        let rate = 8000;
        let n = 2048;
        let mut analyzer = Analyzer::new(rate as f32, n);
        let mut meter = ReferenceMeter::new(rate as f32, n);
        let mut noise = Noise::new(255);
        let mut synth = Synth::new(rate as f32, 255);
        synth.set_parameters(Parameters {
            mode,
            ..Default::default()
        });
        let mut controller = AdaptiveVolume::default();
        let mut echo = vec![[0.0_f32; 2]; 560];
        let mut cursor = 0;
        let mut gains = [0.0; 3];
        let mut gain_counts = [0_usize; 3];
        let mut last_reference = [0.0; BANDS];
        for sample in 0..rate * 125 {
            let seconds = sample as f32 / rate as f32;
            let quiet = seconds < 3.5;
            let stage = if seconds < 25.0 {
                0
            } else if seconds < 70.0 {
                1
            } else {
                2
            };
            let fan_scale = [1.0, 3.9810717, 0.25118864][stage];
            let t = seconds * std::f32::consts::TAU;
            let fan_sample = (noise.next_signed() * 0.006
                + (t * 370.0).sin() * 0.002
                + (t * 740.0).sin() * 0.0015)
                * fan_scale;
            let output = synth.next_frame(
                if quiet {
                    0.0
                } else {
                    0.25 * gain_from_db(controller.status.gain_db)
                },
                0.08,
            );
            let delayed = echo[cursor];
            echo[cursor] = output;
            cursor = (cursor + 1) % echo.len();
            let microphone = fan_sample + delayed[0] * 0.030 + delayed[1] * 0.025;
            meter.push(ReferenceFrame { channels: output });
            if analyzer.push(microphone).is_some() {
                if let Some(p) = meter.powers() {
                    last_reference = p;
                }
                let status = controller.update(Observation {
                    microphone: analyzer.band_powers(),
                    playback: last_reference,
                    quiet,
                    trusted_quiet: quiet && seconds > 0.5,
                    independent: false,
                    reference_valid: true,
                    mode,
                    seconds: n as f32 / 4.0 / rate as f32,
                });
                assert!(status.gain_db.is_finite() && status.gain_db <= 0.0);
                if (15.0..24.0).contains(&seconds)
                    || (60.0..69.0).contains(&seconds)
                    || (112.0..124.0).contains(&seconds)
                {
                    gains[stage] += status.gain_db;
                    gain_counts[stage] += 1;
                }
            }
        }
        let measured = std::array::from_fn::<_, 3, _>(|i| gains[i] / gain_counts[i] as f32);
        eprintln!("{mode}: actual FFT/echo simulation mean gains = {measured:?} dB of maximum");
        assert!(measured[1] > measured[0] + 3.0, "{mode}: {measured:?}");
        assert!(measured[2] < measured[0] - 3.0, "{mode}: {measured:?}");
    }
}
