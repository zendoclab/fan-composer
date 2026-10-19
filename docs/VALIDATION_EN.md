# Validation for 0.3.0

This is an engineering prototype, not a validated comfort intervention. The machine-readable snapshot is [validation.json](../reports/public/validation.json); the sanitized full automated output is [tests-0.3.0.log](../reports/public/tests-0.3.0.log).

## Automated checks

**26 tests passed, 0 failed.** Release build, rustfmt and strict clippy also passed.

The suite covers FFT accuracy at both frame sizes, harmonic/fan candidate stability, synthetic scenarios, finite and bounded DSP, reproducibility, mode/stop continuity, exact mute, callback DSP/reference-queue allocation behavior, stream-error handling and settings migration.

Seven new automatic-volume tests cover sustained louder/quieter fan steps, self-playback growth, narrow interference regressions, invalid/missing reference, gain/peak limits, settings migration and an actual-FFT closed-loop simulation.

## Automatic-volume simulation

Each mode uses 125 virtual seconds of signal at 8 kHz, FFT size 2048. The actual analyzer, reference meter, controller and synthesizer run together with a known 70 ms stereo leakage path. Fan levels are baseline / +12 dB / -12 dB. These are controlled digital mixtures, not physical acoustic validation.

| Mode | Baseline gain | Louder fan | Quieter fan |
|---|---:|---:|---:|
| Harmonic | -6.00 dB | -0.14 dB | -12.19 dB |
| Nature | -6.00 dB | -0.01 dB | -11.98 dB |
| Generative | -6.00 dB | -0.09 dB | -12.15 dB |

Gains are relative to the user-selected maximum, not perceived loudness. Self-output-only growth did not cause automatic amplification in the tested cases. Short/partial-band interference rejection is not general speech recognition.

## Actual portable Windows run

A **45-second silent duplex** test used the built-in microphone and Realtek speaker endpoint. Maximum gain was zero; raw microphone audio was not saved.

- Input frames: 2,165,760. Output frames: 2,167,296.
- Actual execution device: **NPU**, 45 policy decisions; CPU/rule decisions 0.
- Input/reference queue drops: 0/0. Stream fault: 0.
- Maximum observed callback processing: 619 microseconds. Callback over-budget: 0.
- Startup input Xrun: **1**, retained in diagnostics.
- Quiet-state observations after 4 seconds: **0**, including the old 30-second interruption boundary.
- Automatic volume reached a valid tracking state with the playback muted. This does not exercise physical speaker contamination.

Callback processing time is not total audio latency or a guarantee of long-run stability.

## Real CPU/NPU model execution

The portable build completed 32 varying inputs on explicit NPU and 32 on explicit OpenVINO CPU. The maximum output difference was **0.0005520582**. The unavailable-primary injection completed 32 actual CPU decisions using the same fallback-selection code. A physically NPU-less computer was not tested.

The model is rule-teacher imitation. There is no evidence of better music or energy savings from NPU execution. Current-policy latency distributions, CPU usage and power were not benchmarked.

## Human and long-session evidence

The user reported audible, generally acceptable sound for the earlier 0.1.1 modes. That informal feedback predates automatic volume. Version 0.3.0 still needs real-room listening, physical fan-level/path-change experiments, other speech/music tests, and 1-hour/8-hour sessions.

Raw capture/APO status, calibrated SPL, acoustic AEC, system loopback and tray operation remain unverified or unimplemented.
