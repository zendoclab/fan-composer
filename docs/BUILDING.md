# Build and package

## Windows prerequisites

- Windows 11 x64.
- Rust MSVC toolchain; the project declares Rust 1.94 or newer and was developed with 1.97.0.
- Visual Studio C++ Build Tools and Windows SDK.
- For packaging the optional NPU runtime: Python 3.11 x64 and Node.js.
- A supported Intel NPU driver for NPU execution. NPU presence alone does not establish model compatibility.

A CPU rule fallback is built into the EXE. It works without Python or OpenVINO. NPU/OpenVINO CPU inference requires the runtime and committed model files.

## Build and checks

```powershell
cargo build --release --locked
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

Output: `target/release/fan-composer.exe`.

## Portable package with CPU/NPU inference

```powershell
python -m venv .venv
.venv\Scripts\python.exe -m pip install -r tools/requirements-openvino.txt
node tools/package.mjs
```

Output folder:

```text
dist/FanComposer/
  fan-composer.exe
  README.md
  LICENSE.txt
  manifest.json
  models/policy.xml
  models/policy.bin
  models/policy.json
  runtime/*.dll
  runtime/licenses/
```

Keep the folder intact. The end user does not need Python or a global OpenVINO installation. The packaging script copies the CPU/NPU runtime from the local virtual environment and includes its licenses. It does not change the system PATH or install drivers.

If the Windows loader reports missing MSVC runtime components, install the official Microsoft Visual C++ x64 Redistributable. NPU driver installation is separate from the portable app.

## Regenerate the policy (optional)

```powershell
.venv\Scripts\python.exe tools/train_policy.py
```

The seed is 255. The script generates 32,768 teacher examples and a 4,096-example holdout, trains a 16→32→6 MLP for 1,200 Adam steps, and writes OpenVINO IR plus model metadata. It imitates explicit rules, not listener preference data. Regenerating weights can vary slightly with numerical-library versions.

## Diagnostics

```powershell
target\release\fan-composer.exe --policy-test reports/policy.json
target\release\fan-composer.exe --policy-test reports/cpu.json --force-cpu
target\release\fan-composer.exe --policy-test reports/fallback.json --simulate-unavailable-npu
target\release\fan-composer.exe --diagnose reports/audio-devices.json
target\release\fan-composer.exe --capture-test 5 reports/capture.json
target\release\fan-composer.exe --silent-duplex-test 45 reports/duplex.json
target\release\fan-composer.exe --ui-smoke
```

Create `reports/` first if needed. The policy test records real model inference and actual execution device; without a working runtime it exits unsuccessfully rather than presenting rules as NPU inference. Injected unavailability tests the selection path but is not a physically NPU-less computer.

The silent duplex test opens real devices with maximum gain zero. It cannot verify audibility or acoustic feedback rejection. Optional arguments after the report select output and input endpoint IDs. Device inventory and raw local diagnostics are intentionally ignored by Git.

## Background automation on Windows

The local agent workflow uses Node child processes with `windowsHide: true`, `shell: false`, and piped output. `tools/dev.mjs` provides build/test/check/fmt wrappers. Existing interactive terminal preferences are preserved.
