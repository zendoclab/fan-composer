# Development status — 0.3.0 / 2026-10-20

Always-on automatic volume and the simplified main screen are implemented. The user sets one maximum; there is no automatic-volume switch. Speaker-reference profiling continues without a silent gap. NPU-first policy with CPU fallback remains active.

Current results: 26 automated tests pass; release build, strict clippy and format pass. Portable real-device silent duplex: 45 seconds, 45 actual NPU decisions, input/reference drops 0, stream fault 0, over-budget 0, callback maximum 619 microseconds, startup input Xrun 1. No additional quiet observations after the initial 4 seconds.

[English validation and limits](VALIDATION_EN.md), [usage](USAGE.md), [building](BUILDING.md), [architecture](ARCHITECTURE.md), [public evidence](../reports/public/validation.json). Earlier 0.2.0 notes are in history/.

Real-room automatic-volume listening, calibrated SPL, controlled comfort and power comparisons, Raw/APO validation, acoustic AEC, loopback, tray, and 1-hour/8-hour sessions remain open. Do not treat the controlled FFT/echo simulation as physical acoustic validation or the rule-teacher model as evidence of NPU musical superiority.
