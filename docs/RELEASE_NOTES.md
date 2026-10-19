# Fan Composer 0.3.0 — Research Preview

**Turning Machine Noise into Music**

Fan Composer began because the Intel Core Ultra 7 laptop's NPU was rarely used in everyday work. This project explores a useful local workload: turning fan-noise characteristics into a quiet adaptive soundscape.

## Download

Download **FanComposer-0.3.0-windows-x64.zip**, extract the whole folder, and run `FanComposer/fan-composer.exe`. Keep `models/` and `runtime/` beside the EXE. The portable package includes the OpenVINO CPU/NPU runtime and its licenses; Python is not needed to run it.

## Use

1. Choose the intended **Speaker**.
2. Set **Maximum volume** and press **Start**.
3. Wait for the initial **3.5-second quiet fan measurement**.
4. Try **Harmonic**, **Nature**, or **Generative**.
5. Automatic volume is **always on**, adjusting gently within your maximum. **Stop** stays visible.
6. Use **Measure fan again** after a room/path change. It deliberately pauses sound for 3.5 seconds.

Technical controls are collapsed under **Advanced**, and settings save automatically. There is no microphone recording, cloud processing, login, or automatic playback at launch.

![Actual Fan Composer 0.3.0 UI, stopped](https://raw.githubusercontent.com/zendoclab/fan-composer/main/docs/images/fan-composer.png)

## NPU and automatic volume

The 1 Hz music policy prefers **NPU**, then **OpenVINO CPU**, then **CPU rules**. Advanced shows actual execution device, decision counts and fallback reasons. FFT, gain control and sample synthesis remain CPU DSP.

The new volume controller uses a quiet baseline and the actual stereo playback power reference. It holds upward adaptation or gently reduces playback when the estimate is uncertain. This experimental guard is not acoustic echo cancellation or general speech separation.

## Build

Windows 11 x64, Rust MSVC, Visual Studio C++ Build Tools and Windows SDK:

```powershell
git clone https://github.com/zendoclab/fan-composer.git
cd fan-composer
cargo build --release --locked
cargo test --release --locked
```

For NPU packaging, Python 3.11 x64 and Node.js:

```powershell
python -m venv .venv
.venv\Scripts\python.exe -m pip install -r tools/requirements-openvino.txt
node tools/package.mjs
```

[Full build guide](https://github.com/zendoclab/fan-composer/blob/main/docs/BUILDING.md) · [Usage](https://github.com/zendoclab/fan-composer/blob/main/docs/USAGE.md)

## Recorded checks and limits

- **26 tests passed**; release build, format and strict clippy passed.
- All three modes followed louder/quieter fan steps in an actual-FFT, delayed-leakage simulation.
- Portable **45-second silent physical duplex**: 45 actual NPU decisions, input/reference drops 0, fault 0, over-budget 0, one startup input Xrun.
- Real NPU and OpenVINO CPU inference plus injected-unavailability CPU fallback passed.
- The model imitates local rules. Comfort improvement, energy savings, physical acoustic tracking, and 1-hour/8-hour stability are not established.

Author: **Kyu Lee · cording.ai**.
[English preprint on Zenodo](https://doi.org/10.5281/zenodo.23478732) · [Validation evidence](https://github.com/zendoclab/fan-composer/blob/main/reports/public/validation.json)

Code/model: MIT. Preprint: CC BY 4.0. Verify downloads with `SHA256SUMS.txt`.
