# 0.1.1 — 출력이 잘 들리지 않는 문제

## 사용자 관찰과 원인 조사

사용자는 내장 Realtek 스피커에서 세 모드를 선택했으나 소리가 잘 들리지 않는다고 보고했습니다. 움직이던 그래프는 마이크 입력 스펙트럼이므로 스피커 출력 성공을 나타내지 않습니다.

기존 합성 WAV의 5–14초 구간을 측정한 RMS는 gain=0.04일 때 Harmonic -38.55, Nature -41.21, Generative -40.35 dBFS였습니다. 기존 기본 gain=0.025와 자동 감쇄 gain=0.2가 동시에 적용된다고 계산하면 약 -56.61 / -59.27 / -58.41 dBFS입니다. 이는 신호 경로에서 계산한 값이며 사용자 청취 시점의 실제 출력 측정값은 아닙니다.

기존 감쇄 조건은 flux > 0.55 또는 pitch 변화율 > 700 Hz/s만 보았습니다. -70 dBFS 부근의 작은 잡음도 정규화된 FFT 변화량은 커질 수 있어, 주변 소리가 작아도 -14 dB 감쇄를 유발할 수 있었습니다. 저음에 치우친 음색도 별도 개선 대상이었습니다.

Windows의 내장 스피커 볼륨을 읽기 전용으로 조회한 결과 master 15%, -27.702854 dB, mute=false였습니다. 조회 시점에는 해당 PID의 활성 오디오 세션이 없어 앱별 믹서 값은 null이었습니다. null을 0% 또는 음소거로 해석하지 않았습니다. 전체 Windows 볼륨이나 믹서 설정은 변경하지 않았습니다.

## 수정

- 기본 앱 게인 -18 dB, 조절 범위 MUTE / -59..-6 dB. 기존 저장 게인은 보존합니다.
- 디지털 Peak cap 기본 0.08 유지, 부드러운 제한·fade 유지. 음압 dB SPL을 의미하지 않습니다.
- 저역 비중을 줄이고 2·3차 배음 및 생성 멜로디의 옥타브를 보강했습니다.
- Nature의 중역을 보강하고 모드 사이 합성 RMS 차이를 줄였습니다.
- 자동 감쇄는 절대 입력 레벨 > -52 dBFS, 보정 바닥보다 10 dB 이상 큼, 변화 지속 60ms를 함께 요구합니다. 0.8초 hold 후 해제합니다.
- 스피커 회귀음과 실제 주변 소리를 구분하는 음향 경로는 아직 검증되지 않았으므로 자동 감쇄 기본값을 OFF로 두었습니다. 옵션은 실험 기능으로 유지합니다.
- 실제 출력 RMS·peak, 보정 무음, 자동 감쇄 상태, Windows master/app mixer 볼륨·mute를 상단에 표시합니다.
- Test speaker 버튼: 사용자가 누를 때만 500 Hz를 2초 출력하며 마이크는 열지 않습니다. 제한과 시작·종료 fade, Stop 시 release를 적용합니다.
- 마이크 스펙트럼임을 화면에 명시했습니다. Start나 Test를 누르기 전에는 소리가 나지 않습니다.

## 검증

15개 자동 테스트 통과: 기존 DSP/장치 오류/무할당 검증과 함께 저레벨 잡음의 오감쇄 방지, 실제 큰 간섭 감쇄·해제, 게인 범위·mute, 2초 시험음의 상한·종료·release를 검증했습니다.

팬 입력과 Windows 볼륨을 제외한 순수 합성 테스트(48kHz, seed=255, 기본 -18dB, peak cap=0.08, 5–20초 구간):

| 모드 | RMS dBFS | Peak dBFS |
|---|---:|---:|
| Harmonic | -30.67 | -23.39 |
| Nature | -32.30 | -22.03 |
| Generative | -29.26 | -22.87 |

실제 스피커에서 잘 들리는지와 쾌적성은 사용자 재검증이 필요합니다. 자동화로 소리를 재생하지 않았습니다.

## 재검증 방법

1. 0.1.1 창인지 확인하고 내장 Realtek 스피커를 선택합니다.
2. Level을 -18dB로 맞춥니다. Windows 볼륨은 현재 15%이므로 들리지 않으면 30% 정도부터 서서히 조절합니다.
3. 정지 상태에서 Test speaker를 누릅니다. 500Hz 소리는 2초 후 멈춥니다.
4. 시험음이 들리면 Start를 눌러 3.5초 보정 후 세 모드를 비교합니다.
5. 시험음도 들리지 않으면 상단 Windows master/app mixer mute, 출력 RMS·peak 상태를 확인합니다. 입력 그래프 움직임만으로 출력 성공을 판단하지 않습니다.

Windows 볼륨 조회는 [IAudioEndpointVolume](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume)과 [ISimpleAudioVolume](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-isimpleaudiovolume)을 사용합니다. COM 조회는 오디오 콜백 밖에서 수행하며 setter는 호출하지 않습니다.

최종 실제 내장 마이크/스피커 무음 회귀 검사: fault 0, 입력 239520 / 출력 241056 프레임, 큐 손실 0, 시작 Xrun 1, callback max 151 us, 처리 예산 초과 0. 실제 청취는 포함하지 않았습니다.
