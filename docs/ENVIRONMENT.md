# 개발 환경 조사 — 2026-10-19

| 항목 | 확인 결과 |
|---|---|
| 프로젝트 | D:/aProjects/fan — 시작 시 빈 폴더 |
| Windows | Windows 11 Home x64, 10.0.26300 |
| CPU | Intel Core Ultra 7 255H, 16 cores / 16 logical |
| GPU | Intel Arc 140T GPU (16GB), driver 32.0.101.8801 |
| NPU | Intel AI Boost, PnP OK, driver 32.0.100.4778 |
| Rust / Cargo | 1.97.0, x86_64-pc-windows-msvc |
| C++ 도구 | VS Build Tools 2022 17.14.35, 기존 설치 재사용 |
| Python | 기존 3.11.9 |
| OpenVINO 기존 | 2025.4.0, CPU/GPU/NPU 열거 및 CPU/NPU 진단 추론 성공 |
| OpenVINO 프로젝트 | .venv에 2026.4.1 설치, CPU/GPU/NPU 열거 및 CPU/NPU 진단 추론 성공 |

마이크·스피커의 실제 레이트, 지원 범위, 지원 버퍼 표시는 `reports/audio-devices.json`으로 조사합니다. 드라이버가 버퍼 범위를 Unknown으로 반환하면 임의의 숫자를 지원 값으로 기록하지 않습니다. 기본값이 48 kHz라고 가정하지 않습니다.

현재 캡처는 정상 WASAPI 공유 모드입니다. Raw 캡처, AGC, 노이즈 억제, 에코 처리 상태를 앱에서 검증하거나 변경하지 않았습니다. 팬 RPM, 시스템/NPU 전력도 측정하지 않았습니다.

## 안정 버전 조사와 선택

crates.io API 및 공식 crate API 문서 기준으로 iced 0.14.0, cpal 0.18.2, rustfft 6.4.1, rtrb 0.4.0, serde 1.0.229, serde_json 1.0.154, hound 3.5.1을 선택했습니다. 의존성의 실제 해석은 Cargo.lock에 기록됩니다. Rust OpenVINO crate 0.11.1도 조사했으나 Phase 1 앱에는 추가하지 않았습니다.

## 공식 자료

- [iced 0.14 API](https://docs.rs/iced/0.14.0/iced/)
- [CPAL 0.18.2 DeviceTrait](https://docs.rs/cpal/0.18.2/cpal/traits/trait.DeviceTrait.html)
- [rustfft API](https://docs.rs/rustfft/6.4.1/rustfft/)
- [rtrb API](https://docs.rs/rtrb/0.4.0/rtrb/)
- [OpenVINO 2026 NPU device](https://docs.openvino.ai/2026/openvino-workflow/running-inference/inference-devices-and-modes/npu-device.html)

AUTO를 사용하지 않고 CPU/NPU를 각각 지정하며 `EXECUTION_DEVICES`와 컴파일/추론 성공을 별도 기록합니다. 기존 전역 Python·Rust·PATH·오디오·드라이버 설정은 변경하지 않았습니다.

## 실제 오디오 장치 조사

내장 마이크 배열 and Realtek speaker report F32 stereo 48000 Hz, supported buffer range 480–480 frames. Analysis downmixes to mono. Windows default output is the Realtek headphone endpoint, not the built-in speaker. Full supported format enumeration: reports/audio-devices.json. Normal shared WASAPI may perform format conversion; enumerated supported formats do not establish raw hardware capture support.

## 0.2.0 음악 정책 통합 — 2026-10-20

libloading 0.9.0을 사용해 Rust에서 OpenVINO 2026.4.1 C API를 직접 호출합니다. 절대 경로 DLL 로드로 전역 PATH를 변경하지 않습니다. 포터블 폴더에는 CPU/NPU 플러그인만 포함하며 실제 장치 CPU/NPU 열거·NPU 추론·CPU 폴백을 검증했습니다. GPU 추론 플러그인은 동봉하지 않습니다. 런타임과 모델은 실행 파일 기준으로 찾고 개발 중에는 프로젝트 .venv/models를 사용합니다. Python은 학습/개발 도구이며 배포 앱 실행 경로에는 사용하지 않습니다.
