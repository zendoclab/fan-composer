# 0.2.0 — 연속 재생과 NPU 우선 음악 정책

## 사용자 요청 반영

0.1.1에서 세 모드와 Generative가 잘 들리고 대체로 괜찮다는 사용자 의견을 받았습니다. 모든 모드가 주기적으로 끊기던 원인은 30초마다 실행하던 3.5초 무음 팬 재측정이었습니다.

0.2.0은 시작 보정 3.5초 후 연속 재생합니다. 무음 보정은 Calibrate 버튼으로 요청하거나, 사용자가 Auto quiet calibration을 켠 경우에만 반복합니다. 기본값은 OFF입니다. 재생 중 팬 특징은 마지막 보정값을 유지해 자기 스피커 출력을 팬으로 추적하지 않도록 합니다. 팬 상태가 바뀌면 수동 보정이 필요하며, 이 방식이 음향 에코 제거를 구현한 것은 아닙니다.

## NPU primary / CPU fallback

앱의 음악 정책은 NPU를 우선합니다. 별도의 Python 프로세스 없이 Rust가 OpenVINO 2026 C API를 직접 호출합니다.

1. 실제 장치 열거 → NPU 모델 컴파일 → 시험 추론 → EXECUTION_DEVICES 확인
2. 성공하면 음악 정책을 NPU에서 1Hz로 실행
3. NPU 미검출 또는 컴파일·추론 실패 시 동일 모델을 OpenVINO CPU로 전환
4. 런타임/모델까지 사용할 수 없으면 CPU 규칙 정책으로 계속 동작하며 이유를 표시

FFT와 오디오 샘플 합성은 CPU DSP입니다. 추론은 별도 fan-npu-policy 스레드에 있고, 분석·오디오 콜백은 추론 완료를 기다리지 않습니다. 실패·재컴파일 시에도 기존 합성 상태를 유지합니다. UI는 실제 장치, NPU/CPU 결정 횟수, 마지막 추론 지연과 폴백 이유를 표시합니다. 요청 장치나 열거만으로 NPU 실행 성공을 표시하지 않습니다.

## 실제 음악 정책 모델

자체 생성한 규칙 기준선을 지도 학습한 16→32→6 고정 MLP입니다. 진단용 난수 그래프가 아닙니다. 실제 출력은 brightness, density, overtones, motion, reverb, stereo이며 세 모드의 합성 파라미터에 연결했습니다. 출력 게인과 피크 상한은 사용자가 제어하며 모델이 올릴 수 없습니다.

데이터: 자체 합성 특징량 32,768개, holdout 4,096개, seed 255, Adam 1,200 steps. Holdout MAE 0.00292718, 최대 오차 0.05688055. 이는 규칙 모방 오차이며 쾌적성·음악 품질 점수가 아닙니다. 사람의 청취 평가로 학습하지 않았으며 규칙 대비 품질 향상을 주장하지 않습니다.

모델 파일·형태·학습 정보: models/policy.xml, policy.bin, policy.json. 재생성: .venv/Scripts/python.exe tools/train_policy.py.

## 실제 검증

- 자동 테스트 19개 통과, release build 및 clippy -D warnings, fmt 통과.
- 세 모드 각각 100초의 가상 시간 합성에서 기존 30/60/90초 경계의 무음 회귀가 없음. 실제 100초 청취나 장시간 안정성 검증으로 해석하지 않음.
- Rust 네이티브 NPU 시험: 모델 32회 추론, actual NPU, CPU/rules 결정 0.
- 같은 32개 입력의 OpenVINO CPU와 NPU 출력 최대 절대 차이 0.00055206.
- NPU 사용 불가 조건을 명시적으로 주입한 시험에서 실제 OpenVINO CPU 32회 추론. 물리적으로 NPU를 제거하거나 드라이버를 변경한 시험은 아님.
- 동봉 런타임의 포터블 EXE도 NPU/CPU 폴백 시험 각각 성공. 모델은 실행 파일 옆 models/, 런타임은 runtime/에서 로드.
- 포터블 EXE 45초 실제 내장 장치 무음 입출력: NPU 정책 45회, CPU/rules 0, fault 0, 입력 2,166,240 / 출력 2,167,776프레임, queue drops 0, callback max 446µs, over-budget 0. 시작 Xrun 1회는 계속 기록.
- 시작 4초 이후 100ms 간격 관측에서 quiet=true 0회. 이전 30초 자동 중단이 없음.
- 위 입출력 시험은 volume=0으로 수행했으며 자동화로 음악을 재생하지 않았음. 0.2.0 청취 품질은 사용자 재검증 대상.
- 시스템/NPU 전력, CPU 사용률, 1h/8h 안정성, NPU의 음악 품질 우월성은 측정하지 않음.

## 재배포와 확인

dist/FanComposer 전체 폴더를 유지합니다. EXE만 옮기면 NPU용 runtime과 models를 잃을 수 있습니다. Python이나 전역 OpenVINO 설치는 앱 실행에 필요하지 않습니다. OpenVINO 라이선스는 runtime/licenses에 동봉했습니다.

Start 후 상단 Music policy가 NPU로 바뀌고 NPU 결정 카운터가 약 초당 1씩 증가하는지 확인합니다. 모델이 작고 저주기로 동작하므로 작업 관리자 사용률이 높게 유지될 필요는 없습니다.

## 공식 API와 구현

설치된 OpenVINO 2026 C 헤더로 ABI, F32 값, shape, 함수 시그니처를 확인하고 실제 실행으로 검증했습니다. DLL 검색은 정규화한 절대 경로와 프로세스 내 DLL 로드 옵션을 사용하며 전역 PATH를 변경하지 않았습니다. 혼합 경로 구분자 때문에 DLL 로딩이 실패한 문제를 경로 정규화로 수정했습니다.

- [OpenVINO Core C API](https://docs.openvino.ai/2026/api/c_cpp_api/group__ov__core__c__api.html)
- [Infer Request C API](https://docs.openvino.ai/2026/api/c_cpp_api/group__ov__infer__request__c__api.html)
- [Tensor C API](https://docs.openvino.ai/2026/api/c_cpp_api/group__ov__tensor__c__api.html)
