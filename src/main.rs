#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod ui;
use fan_composer::{
    audio::{self, AudioSession},
    settings::Settings,
    synth::{Mode, Parameters, Synth},
};
use std::{fs, path::Path, sync::atomic::Ordering, time::Duration};
fn main() {
    if let Err(e) = entry() {
        let _ = fs::create_dir_all(fan_composer::settings::data_dir());
        let _ = fs::write(
            fan_composer::settings::data_dir().join("last-error.txt"),
            &e,
        );
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn entry() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--policy-test") => {
            let forced = args.iter().any(|v| v == "--force-cpu");
            let missing = args.iter().any(|v| v == "--simulate-unavailable-npu");
            let mut engine = if missing {
                fan_composer::policy::Engine::for_device_order(&["UNAVAILABLE_NPU_TEST", "CPU"])
            } else {
                fan_composer::policy::Engine::new(forced)
            };
            let mut results = vec![];
            for step in 0..32 {
                let mut input = [0.5; 16];
                input[3] = step as f32 / 31.0;
                input[14] = (step % 3) as f32 / 2.0;
                results.push(engine.infer(&input));
            }
            let report = serde_json::json!({"forced_cpu":forced,"injected_unavailability":missing,"actual":engine.status,"outputs":results,"finite":results.iter().flatten().all(|v|v.is_finite()),"model_source":"rule teacher distillation; no claimed quality advantage"});
            fs::write(
                args.get(2)
                    .map(String::as_str)
                    .unwrap_or("reports/policy-native.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            if engine.status.npu_decisions + engine.status.cpu_decisions == 0 {
                return Err("Native model inference failed; see report".into());
            }
            Ok(())
        }
        Some("--output-diagnostics") => {
            let pid = args
                .get(4)
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(std::process::id);
            let report = fan_composer::windows_audio::output_mixer(args.get(3).cloned(), pid)?;
            fs::write(
                args.get(2)
                    .map(String::as_str)
                    .unwrap_or("reports/windows-output.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Some("--diagnose") => {
            let report = serde_json::to_string_pretty(&audio::device_report()).unwrap();
            let file = args
                .get(2)
                .map(String::as_str)
                .unwrap_or("reports/audio-devices.json");
            fs::write(file, &report).map_err(|e| e.to_string())?;
            println!("{report}");
            Ok(())
        }
        Some("--capture-test" | "--silent-duplex-test") => {
            let seconds = args
                .get(2)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(5)
                .clamp(1, 60);
            let duplex = args[1] == "--silent-duplex-test";
            let settings = Settings {
                volume: 0.0,
                output: args.get(4).cloned(),
                input: args.get(5).cloned(),
                ..Default::default()
            };
            let s = AudioSession::start(&settings, false, duplex)?;
            let started = std::time::Instant::now();
            let mut quiet_trace = vec![];
            for _ in 0..seconds * 10 {
                std::thread::sleep(Duration::from_millis(100));
                quiet_trace.push(serde_json::json!({"seconds":started.elapsed().as_secs_f64(),"quiet":s.controls.quiet.load(Ordering::Acquire),"auto_gain_db":s.snapshot().auto_volume.gain_db}));
            }
            let c = &s.controls;
            let report = serde_json::json!({"duration_seconds":seconds,"quiet_trace":quiet_trace,"selected_input_id":settings.input,"selected_output_id":settings.output,"raw_audio_saved":false,"physical_capture":true,"speaker_output":duplex,"audible_output":false,"output_frames":c.output_frames.load(Ordering::Relaxed),"callback_max_us":c.callback_max_us.load(Ordering::Relaxed),"callback_over_budget":c.callback_late.load(Ordering::Relaxed),"input_frames":c.input_frames.load(Ordering::Relaxed),"input_dropped":c.input_dropped.load(Ordering::Relaxed),"reference_dropped":c.reference_dropped.load(Ordering::Relaxed),"input_error_kind":audio::error_kind_name(c.input_error_kind.load(Ordering::Relaxed)),"output_error_kind":audio::error_kind_name(c.output_error_kind.load(Ordering::Relaxed)),"xruns":c.xruns.load(Ordering::Relaxed),"realtime_warnings":c.realtime_warnings.load(Ordering::Relaxed),"fault":c.fault.load(Ordering::Acquire),"telemetry":s.snapshot()});
            fs::write(
                args.get(3)
                    .map(String::as_str)
                    .unwrap_or("reports/capture-test.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            if c.input_frames.load(Ordering::Relaxed) == 0
                || c.fault.load(Ordering::Acquire) != 0
                || duplex && c.output_frames.load(Ordering::Relaxed) == 0
            {
                return Err("Capture validation failed; inspect report".into());
            }
            Ok(())
        }
        Some("--render-demo") => render(args.get(2).map(String::as_str).unwrap_or("reports/demos")),
        _ => ui::run().map_err(|e| e.to_string()),
    }
}
fn render(directory: &str) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    for mode in Mode::ALL {
        let mut synth = Synth::new(48000.0, 255);
        synth.set_parameters(Parameters {
            mode,
            ..Default::default()
        });
        let file = Path::new(directory).join(format!("{mode:?}.wav"));
        let mut writer = hound::WavWriter::create(
            file,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|e| e.to_string())?;
        for i in 0..48000 * 15 {
            let gain = if i > 48000 * 14 { 0.0 } else { 0.04 };
            for sample in synth.next_frame(gain, 0.08) {
                writer
                    .write_sample((sample * i16::MAX as f32) as i16)
                    .map_err(|e| e.to_string())?;
            }
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(())
}
