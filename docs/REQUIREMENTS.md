# Fan Composer — requirements baseline

Turning Machine Noise into Music. Windows 11 x64; Rust; iced; CPAL/WASAPI; rustfft. CPU performs FFT and synthesis. OpenVINO music policy executes at 0.5–2 Hz, outside audio callbacks: NPU is primary when available, with CPU fallback when unavailable or failing. Local processing only, no accounts, cloud, advertising, recorded samples or external model requirement.

## Research hypothesis
Adding quiet, adaptive harmony or synthesized nature textures may reduce perceived fan annoyance without reducing physical fan SPL. This is not ANC. dBFS is not dB SPL. Subjective benefit remains unproven.

## Required behavior
- Explicit start/stop, device selection, input activity indication, spectrum, maximum volume, always-on fan-aware automatic volume with no enable switch, calibration, three distinct sound modes. Keep primary UI simple; advanced diagnostics collapsed.
- Hann STFT 2048/4096, 50–75% overlap, peak interpolation, stable harmonic tracking, RMS/dBFS, centroid, flatness, tonality, broadband estimate, changes and confidence.
- Continuous playback by default; startup/manual quiet calibration, periodic quiet calibration opt-in. Output reference contamination guard, silent calibration with frozen fan features; speech/music confidence reduction. No claim of acoustic echo cancellation without delay/path validation.
- Harmonic drone with pitch/chord hysteresis; mathematical noise nature textures; seeded stateful procedural generative fallback.
- Low initial volume, finite-value guard, peak cap, slew and fades, stream error stop, immediate stop, no unattended audio validation.
- Settings and explicit feedback stored locally; microphone samples never persisted by default.
- Synthetic fan cases: fixed, ramp, step, harmonics, turbulence, multiple fans, inharmonic sound.
- NPU enumeration, compile and inference are separate evidence. Explicit CPU fallback, actual execution device. No synthetic success or made-up power data.

## Acceptance
Release build, native EXE launch, real capture and output independently validated, three working modes, visualization and safety controls, real NPU attempt, CPU fallback, automated and manual results, build/use docs. Tray, loopback, long-run tests and blinded comfort study tracked separately until verified.
