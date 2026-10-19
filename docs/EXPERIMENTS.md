# Experiments

2026-10-19: empty project directory. Windows 11 Home 10.0.26300; Core Ultra 7 255H, 16 cores / 16 logical; Intel Arc 140T driver 32.0.101.8801; Intel AI Boost driver 32.0.100.4778, PnP status OK. Existing Rust 1.97.0 MSVC and VS Build Tools 2022 reused. Python 3.11.9 with OpenVINO installed; runtime/device tests pending.

All results below must indicate synthetic vs physical I/O, duration, settings and limitations. No subjective claims without participant ratings.

## DSP automated baseline

Initial 7 tests passed, then the stronger-second-harmonic regression and real-time allocation test passed (9 total). A following CPAL event-classification regression was added after physical capture exposed a failure. Final test logs are retained under reports/.

Strongest FFT peak and harmonic fundamental are now separate: observed peak families are scored, with temporal stability required; missing subharmonics are not invented. Spectral descriptors use a 70 Hz–4 kHz fan-analysis band; the spectrum display covers 40 Hz–20 kHz subject to Nyquist. Noise floor uses median bin power with temporal averaging. These are uncalibrated digital measures.

## OpenVINO diagnostic inference

Existing 2025.4 and isolated project 2026.4.1 both enumerated CPU/GPU/NPU, compiled the fixed 1x16 -> 32 -> 8 MLP explicitly on CPU and NPU, returned finite outputs, and reported the expected EXECUTION_DEVICES. The graph is deterministic and untrained, not a music policy. The app continues to display CPU rules.

2026 preliminary synchronous 100-call latency: CPU median 0.03555 ms / p95 0.051785 ms; NPU median 0.6413 ms / p95 0.72051 ms. Maximum absolute output difference 0.00012379885. Rust compilation was concurrent, so these timings are diagnostic observations, not controlled system-efficiency benchmarks. System/NPU power, CPU utilization and any change in fan noise were not measured. No NPU speed/quality/energy advantage is established.

## Physical capture: initial failure

The first 5-second microphone test delivered 239040 frames at a negotiated 48000 Hz but failed with fault 1 and 173504 queued-sample drops. Analysis had stopped because every CPAL event was treated as fatal. The original failure report is preserved as reports/capture-initial-failure.json. The fix distinguishes reported Xrun and RealtimeDenied events from fatal route/device/permission/backend failures, counts them, and keeps fatal failures safe. The repeat test must determine the actual event kind and validate the fix.

## Native UI

The actual Windows x64 GUI executable launched, rendered an iced software-rendered window and exited successfully in the no-audio UI smoke test. PE machine 0x8664 and subsystem 2 (Windows GUI), not a console application. Initial screenshot retained as reports/ui-smoke.png. Fixed Start/Stop controls are being moved outside the scroll area. No speaker audio has been played by automation.

## Repeat physical validation — succeeded with a recorded Xrun

The repeated 5-second microphone test returned fault 0, 239040 input frames, no queue drops, and one reported input Xrun. Features were nonzero (e.g. -70.13 dBFS in the final snapshot), but no reliable fan pitch was established; fan_hz remained 0. This validates capture/analysis, not fan source identity or raw audio quality.

An explicit built-in microphone / Realtek built-in speaker 15-second duplex test at zero gain returned fault 0; 719520 input frames, 720576 output frames, zero queue drops, maximum output callback compute time 403 us and zero compute-budget overruns. One input Xrun was reported; no output error was reported. This confirms physical WASAPI data delivery with silent samples, not audible speaker quality. No microphone waveform was saved. Windows default output was actually the Realtek headphone endpoint, so built-in speakers were selected by stable device ID for the test.

## Final diagnostic repeat, no Rust compilation concurrent

OpenVINO 2026.4.1: CPU median 0.03415 ms / p95 0.05163 ms; NPU median 0.68695 ms / p95 1.03725 ms, 100 synchronous calls after 5 warmups each. Maximum output difference unchanged. External system load was not controlled; power and fan changes remain unmeasured. See reports/openvino-2026.json.

Final DSP/error classification tests: 10 passed. Clippy with warnings denied and formatting checks passed. Release build succeeded. The portable GUI app was opened in stopped state for explicit user Start and auditory feedback. No automatic audible output was used.

## Follow-up: barely audible output

The user reported almost inaudible sound on all three modes using the built-in Realtek speaker. This supersedes the pending auditory acceptance; 0.1.0 was not accepted as audibly usable. The original meter showed microphone spectrum only. The default gain + broad automatic attenuation and bass-heavy voicing were investigated; Windows master was measured at 15% / -27.702854 dB, unmuted. No active per-app session existed during the query. See AUDIBILITY_0.1.1.md and quiet-output-baseline.json / quiet-output-windows.json.

0.1.1 adds explicit speaker test, actual output metering, read-only Windows mixer status, dB gain controls, midrange voicing and a conservative optional interference gate (default OFF). Digital-only default synthesis RMS: Harmonic -30.67, Nature -32.30, Generative -29.26 dBFS. Peak cap unchanged at 0.08. Fifteen automated tests pass; audible acceptance remains pending.

## 0.2.0 continuity and NPU primary

User feedback: 0.1.1 was audible and broadly acceptable, including Generative. They reported periodic interruptions and requested NPU primary with CPU fallback when NPU is absent. The interruptions came from the fixed 30s/3.5s quiet calibration. This is now opt-in, default OFF; initial and manual calibration remain. Fan estimates are held during playback.

A locally trained rule-teacher 16-32-6 MLP is now used by the Rust app, via OpenVINO C API on its own inference thread at 1Hz. Six outputs affect actual synthesis. Production selection is NPU -> OpenVINO CPU -> rule CPU. Native NPU 32-call test succeeded; packaged runtime also succeeded. An injected unavailable primary selected actual CPU and completed 32 model calls. This injection is not a physical no-NPU machine test.

Packaged 45s silent physical duplex: NPU decisions 45, CPU/rules 0, fault 0, input 2166240/output 2167776 frames, no queue drops, callback max 446us, over-budget 0, startup Xrun 1. No quiet samples after 4 seconds, including the old 30s boundary. Nineteen tests pass, including 100 virtual seconds per mode without silence across the old boundaries. These do not establish subjective quality or 1h/8h endurance. See NPU_CONTINUITY_0.2.0.md and versioned reports.
