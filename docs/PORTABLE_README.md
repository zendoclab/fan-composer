# Fan Composer 0.3.0

Turning Machine Noise into Music
Kyu Lee · cording.ai

Run fan-composer.exe. Keep models/ and runtime/ beside the EXE. Python and a global OpenVINO installation are not required.

1. Choose the intended Speaker.
2. Set Maximum volume and press Start.
3. Wait for the initial 3.5-second quiet fan measurement.
4. Select Harmonic, Nature, or Generative.
5. Automatic volume always follows the estimated fan/background level within your maximum. There is no enable switch.
6. Stop is always visible. Measure fan again deliberately pauses sound for 3.5 seconds.

The initial sound starts 6 dB below the maximum. The speaker-reference profile is learned while sound continues. Uncertain estimates cannot request increasing amplification; output-dominated estimates can reduce playback. This experimental guard is not acoustic echo cancellation or general speech separation.

The NPU is primary for the 1 Hz music policy. An unavailable or failing NPU falls back to OpenVINO CPU, then CPU rules if runtime/model use fails. Advanced shows actual devices, decision counts, fallback reasons, stream events and volume estimates. FFT, gain control and synthesis are CPU DSP.

Settings save automatically. No microphone recording or cloud processing. The app does not change Windows master volume. The digital peak cap is not a calibrated sound-pressure limit.

The project began because the Intel Core Ultra 7 laptop's NPU had few everyday uses, motivating a useful local fan-aware application. Perceptual comfort and energy savings remain unproven.

Source, build guide, usage, screenshots and validation:
https://github.com/zendoclab/fan-composer

Code/model: MIT. Runtime notices: runtime/licenses/. Preprint: CC BY 4.0.
