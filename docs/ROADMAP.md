# HTDD roadmap — 0.3.0

## Phase 1 — core DSP MVP (implemented; long-session validation pending)

H1: stable harmonic estimates on synthetic fans and bounded synthesis support a usable native app.
Analyzer, simulator, safe capture/output streams, spectrum and native release EXE are implemented. Actual microphone capture and silent built-in speaker delivery pass. The user reports audible, acceptable sound in all three modes in 0.1.1. This is informal feedback, not a controlled comfort study. Current automated suite: 26 tests. Always-on, bounded automatic volume and a simplified UI are implemented; actual-FFT closed-loop simulation passes. Physical acoustic level-tracking and user listening remain pending. See STATUS.md.

## Phase 2 — generative music (initial engine implemented)

H2: seeded stateful musical decisions and slow changes avoid random-note fatigue.
Procedural motif/state engine and local feedback history are implemented. Reproducibility and basic continuity tests pass. Continuous playback is now the default; 30-second quiet recalibration is opt-in. Preference optimization and long-term auditory evaluation remain pending.

## Phase 3 — NPU (real music-policy integration implemented)

H3: a low-rate policy can use the NPU without increasing total fan noise or harming continuity.
Rust directly calls OpenVINO 2026 C API. A locally generated 16→32→6 rule-teacher imitation model controls six synthesis parameters at 1Hz in all modes. Primary NPU, then OpenVINO CPU, then CPU rules. Actual execution device and inference counts are shown. Portable build: real NPU inference and injected-unavailability CPU fallback pass. Same-input CPU/NPU numerical comparison passes.

This is rule imitation, not evidence of better music. Controlled latency distributions for the current model, CPU load, system power and fan-noise comparisons remain pending.

## Phase 4 — robustness and evaluation

Minimal portable packaging with CPU/NPU runtime is implemented and exercised. Validate acoustic contamination, output-reference delay/path estimation, optional loopback, raw capture, tray and 1h/8h sessions. Compare fan only / fixed ambient / FFT-adaptive / NPU-adaptive with matched fan state and randomized order. Six ratings 1–7: annoyance, distraction, naturalness, harmony, loudness burden and use intention. No benefit claim before adequate evidence.
