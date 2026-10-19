# Using Fan Composer 0.3.0

## Start with the simple screen

Choose the intended speaker, set Maximum volume, choose a mode, and press Start. The app first measures the fan/background for 3.5 seconds with playback muted. It then fades in the sound. It never starts playback merely because the application was launched.

On the development laptop, the built-in endpoint was named **스피커(2- Realtek(R) Audio)**. Windows default output could instead be a headphone endpoint. Device names vary on other machines.

The main screen shows the three modes, maximum volume, a Start/Stop button, the current microphone activity, a spectrum, and the policy backend. Automatic volume is always active. There is no automatic-volume toggle.

## What Maximum volume means

This is a ceiling on the application's gain, not a calibrated sound-pressure level or a Windows master-volume setting. The initial sound starts 6 dB below this ceiling, preserving room for a louder fan. A sustained 12 dB increase in the estimated fan band level requests up to 6 dB more sound, stopping at the maximum. A quieter fan requests lower playback.

Requested changes are limited to 0.45 dB/s upward and 1.5 dB/s downward, with hysteresis and a stability delay. The sample synthesizer adds fades and enforces the digital peak cap. At Maximum volume = 0, output is muted exactly.

The displayed percentage is an application control, not a percentage of perceived loudness.

Existing fixed-volume settings are migrated to a visible maximum with 6 dB of headroom so the starting reference gain stays approximately unchanged. Saved maximum settings are not doubled again.

## How the app avoids following itself

After the quiet measurement, a short spectral speaker-reference profile is learned with sound continuing. The audio callback sends the actual post-limiter stereo output to a bounded queue; analysis uses its band powers. A conservative power-leakage estimate and usable-band check reduce contamination from the speaker.

The controller uses stable changes across usable bands. Short broad bursts and changes limited to some frequency bands should not directly increase the gain. When the reference is missing, invalid, inconsistent, or dominant, upward adaptation is held; dominant playback can be reduced gently to reveal the background again.

This is an experimental fan/background estimate. Other steady noises, microphone AGC, movement, changing Windows volume, nonlinear speakers, and room changes can affect it. It is not a speech recognizer or acoustic echo canceller.

## Measure fan again

Use this after moving the laptop, changing the room/speaker path, or when the estimate seems wrong. It deliberately pauses playback for 3.5 seconds and learns a new background reference. Fan pitch used for harmony remains held between quiet measurements, while the volume estimate can update during playback.

Automatic periodic quiet measurement remains OFF by default so sound is continuous. The optional repeating measurement is under Advanced and explicitly warns that it interrupts sound.

## Advanced

- Microphone and speaker devices, FFT size, and synthetic scenario: change while stopped.
- Synthetic fan: analyzes an internal test signal instead of the microphone. The simulated fan itself is not played.
- Test speaker: an explicit 500 Hz, two-second test, with the microphone off.
- Peak cap: hard digital safety ceiling; this is not dB SPL.
- NPU state: actual backend, completed policy counts, last inference time and fallback reason.
- Diagnostics: callback processing time, budget events, input/reference drops and stream events.
- Interference attenuation: a separate optional experimental heuristic; it does not switch automatic fan-volume control off.

Settings save locally after a short debounce. Worse/Same/Better stores explicit feedback and numerical features. Microphone audio is not saved or transmitted.

## Manual checks before long use

Listen to all three modes with a steady fan and confirm there are no periodic 30-second pauses. Allow the fan to change and observe gradual volume adaptation, then check Stop and exact mute. Repeat after changing a speaker path and remeasuring.

Formal comfort comparisons and 1-hour/8-hour sessions are still needed. Do not interpret the synthetic controller tests as a validated listener study.
