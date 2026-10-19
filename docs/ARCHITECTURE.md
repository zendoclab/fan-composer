# Architecture — 0.3.0

iced owns the UI and stream lifetime. CPAL uses shared WASAPI; microphone input is downmixed to mono. Hann STFT, harmonic-family scoring and temporal confidence run on the analysis worker.

The audio callbacks own preallocated DSP state. Atomics and bounded SPSC queues carry samples, post-limiter stereo playback references, and parameters. They perform no file access, model compilation, heap allocation, or blocking waits. Diagnostics count both input and reference queue drops.

## Always-on automatic volume

Quiet acquisition supplies a 24-band baseline (80–5,000 Hz). A stereo playback power meter runs on the analysis worker; a four-second conservative leakage profile is learned with playback continuing. Smoothed residual-band ratios, usable-energy coverage, a lower weighted quantile, stability time and hysteresis guide the gain.

The user sets one maximum. Relative gain begins at -6 dB, requests -6 + 0.5 × estimated fan-level change, and is clamped to -30…0 dB. Upward slew is 0.45 dB/s; downward slew is 1.5 dB/s. Dominant playback can reduce gain gently to expose the background. Invalid/missing reference and inconsistent bands hold upward adaptation; reference drops cause an additional settling hold.

This is a spectral power-leakage guard, not phase-aligned acoustic echo cancellation or a source classifier. Calibrated pitch is still held between quiet measurements. A trusted live volume estimate may update the NPU level feature. Physical path changes, AGC, steady non-fan sounds, and nonlinear playback need real-room validation.

## NPU worker

Analysis publishes a fixed 16-feature request at 1 Hz to fan-npu-policy. Rust directly loads the local teacher-distilled OpenVINO IR through the C API. NPU is primary, followed by OpenVINO CPU and CPU rules. Enumeration, compile, successful inference and EXECUTION_DEVICES are distinct evidence.

Six outputs control brightness, density, overtones, motion, reverb and stereo width. They do not control maximum volume or peak cap. Compilation and inference stay off callbacks, so fallback need not stop audio.

## Continuous synthesis and safety

Mode, oscillator, LFO and phrase state continue across parameter updates. Maximum-volume mute, stop, faults and calibration take precedence over automatic gain. Finite checks, gain slew, fades and digital peak limits remain active.

Initial quiet measurement lasts 3.5 seconds. Manual remeasurement is explicit; periodic quiet measurement is an advanced opt-in and defaults OFF. Playback-reference learning adds no extra silent window.

## UI and storage

The simple main screen exposes a fixed Start/Stop action, three mode buttons, maximum volume, speaker choice and compact status/spectrum. Advanced contains technical settings and diagnostics. Automatic volume has no off switch. Settings save after a short debounce; only explicit feedback features/settings are persisted.

The portable folder includes EXE, model, CPU/NPU runtime and licenses. DLLs use normalized absolute paths without changing global PATH. Python is only an offline development/training tool.
