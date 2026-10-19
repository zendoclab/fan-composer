> Historical Korean notes for 0.2.0. Current 0.3.0 uses always-on automatic volume and a simplified UI. See [current README](README.md).

# Fan Composer

Turning Machine Noise into Music — Windows 11용 로컬 Rust 네이티브 실험 앱입니다. 팬 소음을 제거하는 ANC가 아니라, 낮은 음량의 합성 사운드스케이프를 더하는 가설을 검증합니다. 청각적 쾌적성 개선은 아직 검증하지 않았습니다.

## 실행 (0.2.0)

포터블 EXE: `dist/FanComposer/fan-composer.exe`. 실행만으로 마이크나 스피커를 시작하지 않습니다.

1. Speaker에서 내장 **스피커(2- Realtek(R) Audio)**를 선택합니다. Windows 기본 출력은 헤드폰일 수 있습니다.
2. Level을 **-18 dB**로 맞추고 정지 상태에서 **Test speaker (2 s)**를 누릅니다. 마이크를 열지 않고 500 Hz를 2초 출력합니다.
3. Windows 볼륨이 낮아 들리지 않으면 직접 조금씩 조절합니다. 앱은 Windows 전체 볼륨·음소거를 변경하지 않습니다.
4. **Start** 후 3.5초 무음 보정을 거쳐 세 모드를 비교합니다. 상단 출력 RMS·peak와 보정·감쇄 상태를 확인합니다. 아래 움직이는 그래프는 마이크 입력입니다.
5. 상단 **Stop**으로 정지하고 **Calibrate**로 다시 측정합니다. 기본적으로 중단 없이 재생합니다. 30초 주기 무음 보정은 선택 옵션이며 기본 OFF입니다.
6. **Save settings**, **Worse / Same / Better**로 설정·특징량을 로컬 저장합니다.

기본 출력 게인은 -18dB, Peak cap은 0.08이며 음압 dB SPL이 아닙니다. 자동 감쇄는 실험 기능으로 기본 OFF입니다. 기존 저장된 게인은 유지되므로 옛 설정이면 Level 값을 확인합니다.

Synthetic fan은 수학적 팬 신호를 분석하며 테스트 팬 자체를 스피커로 내보내지 않습니다. 장치·시나리오·FFT 크기는 정지 상태에서 바꿉니다.

[작게 들리는 문제의 조사·수정](docs/AUDIBILITY_0.1.1.md)

## 빌드와 검증

기존 Rust MSVC, Visual Studio Build Tools C++와 Windows SDK를 사용합니다. 현재 개발 환경 Rust 1.97.0. 라이브러리 버전은 Cargo.lock으로 고정합니다.

```text
cargo build --release --locked
node tools/package.mjs
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
```

백그라운드 자동화는 콘솔 창을 만들지 않는 Node 래퍼를 사용합니다. VS Code의 기존 대화형 터미널 설정은 변경하지 않습니다.

```text
node tools/dev.mjs build
node tools/dev.mjs test
node tools/dev.mjs check
```

진단 명령은 프로젝트 루트에서 실행합니다.

```text
target/release/fan-composer.exe --policy-test reports/policy-native.json
target/release/fan-composer.exe --policy-test reports/policy-fallback.json --simulate-unavailable-npu
target/release/fan-composer.exe --diagnose reports/audio-devices.json
target/release/fan-composer.exe --capture-test 5 reports/capture-test.json
target/release/fan-composer.exe --silent-duplex-test 15 reports/silent-duplex-test.json
target/release/fan-composer.exe --render-demo reports/demos
target/release/fan-composer.exe --ui-smoke
.venv/Scripts/python.exe tools/probe_openvino.py --report reports/openvino-2026.json
```

`--capture-test`는 실제 마이크를 지정 시간 동안 수집·분석하지만 원시 오디오는 저장하지 않습니다. `--silent-duplex-test`는 음량 0으로 실제 입력·출력 콜백을 검증하며, 소리가 들리는지는 검증하지 않습니다. 두 명령은 보고서 경로 뒤에 출력 장치 ID, 입력 장치 ID를 선택 인자로 받습니다. `--render-demo`는 합성 음악 WAV만 생성하며 자동 재생하지 않습니다. `--ui-smoke`는 오디오를 켜지 않고 네이티브 창을 잠시 표시한 후 종료합니다.

## 데이터와 한계

설정과 특징량·피드백만 `%LOCALAPPDATA%/FanComposer`에 저장합니다. 외부 전송·마이크 녹음·자동 실행 등록은 없습니다.

음악 정책은 **NPU 우선**, 사용할 수 없으면 **OpenVINO CPU → CPU 규칙**으로 자동 폴백합니다. FFT·오디오 합성은 CPU DSP입니다. 실제 장치와 정책 결정 카운터가 상단에 표시됩니다. 자체 규칙을 모방 학습한 초기 모델이며 음악 품질 향상을 검증하지는 않았습니다. [NPU·연속 재생 구현과 검증](docs/NPU_CONTINUITY_0.2.0.md).

일반 WASAPI 공유 캡처를 사용합니다. Raw/AGC/APO 상태는 확인되지 않았고, 음성·음악 자동 감쇄는 휴리스틱입니다. 음향 경로를 추정하는 에코 제거·시스템 loopback·트레이·음향 경로 검증·사람 평가에 기반한 정책 학습·1시간/8시간 실험은 후속 작업입니다.

[요구사항](docs/REQUIREMENTS.md) · [구조](docs/ARCHITECTURE.md) · [로드맵](docs/ROADMAP.md) · [현재 개발 상태](docs/STATUS.md) · [환경 조사](docs/ENVIRONMENT.md) · [실험 기록](docs/EXPERIMENTS.md) · [수동 검증](docs/VALIDATION.md)
