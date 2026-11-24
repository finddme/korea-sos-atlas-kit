# Organic Maps 데스크톱 사용성 개선

Organic Maps 기존 C++/Qt 창에 한국어 장소 검색, 출발·도착 선택, 도보/차량 전환, 거리·예상 시간·이동 단계, 장소 이름·메모 저장 및 GPS 상태 표시를 추가한다. Korea SOS Atlas Kit 본체는 Rust이며 지도는 별도 프로세스로 실행한다. 음성 안내는 포함하지 않는다.

Organic Maps 이름·로고·지도 고지를 보존한다. 수정판임을 창 제목과 정보 화면에 표시한다. 원래 지도와 경로 엔진을 변경하지 않는다. MAPS.ME 코드를 복사하지 않았다.

## 소스와 권리

- upstream: https://github.com/organicmaps/organicmaps
- 고정 소스: `d81f9197c6759f67aaed60f4f9c819e834dd02cd`
- 수정 소스: `desktop-usability.patch`; 원래 코드와 이 패치는 Apache-2.0이다. `LICENSE`, `NOTICE`를 함께 보존한다.
- 앱 자체 AGPL과 지도 데이터 권리는 별개이다. [지도 권리 대장](../../LICENSES/MAPS.md)을 따른다.
- 원본 `.reuse/dep5`, `data/copyright.html` 및 각 의존성의 LICENSE/NOTICE도 바이너리 배포 시 보존한다.
- 개발 체크아웃, SDK, 중간 빌드, GUI 캡처는 `dev/poc/maps-ui`에 두며 원격에 올리지 않는다.

## 재현 빌드

KDE SDK 6.10, GCC, CMake, Ninja, Git이 있는 개발 환경에서 실행한다. 최종 사용자는 이 도구들을 설치하지 않는다. 소스·서브모듈 준비에 네트워크가 필요하며 준비 후 컴파일과 실행은 오프라인으로 한다.

```sh
# Korea SOS Atlas Kit 프로젝트 루트에서 실행
git clone --filter=blob:none --no-checkout https://github.com/organicmaps/organicmaps dev/poc/maps-ui/upstream
git -C dev/poc/maps-ui/upstream checkout d81f9197c6759f67aaed60f4f9c819e834dd02cd
git -C dev/poc/maps-ui/upstream submodule update --init --recursive --depth 1
flatpak run --user --unshare=network --filesystem="$PWD" --command=bash org.kde.Sdk//6.10 \
  "$PWD/integrations/organicmaps/build.sh" "$PWD/dev/poc/maps-ui/upstream" "$PWD/dev/poc/maps-ui/build"
```

이 스크립트는 패치 적용·컴파일·위치 시각 경계 검사를 수행한다. GUI 검증까지 자동 통과한 것으로 취급하지 않는다. 노트북 메모리를 고려해 기본 병렬도는 2이다. `SOS_BUILD_JOBS`로 조정할 수 있다.

## 실행 구성

공식 시설은 Rust TUI `대피시설` 분류에서 선택하고 `m`으로 보낸다. 해시 검증된 시설의 EPSG4326 좌표만 전용 상태 파일에 원자적으로 기록하며 지도는 `SOS_ATLAS_REQUEST`로 전달받은 로컬 파일을 읽어 목적지를 설정한다. 기존 지도 창에서도 목적지를 교체할 수 있다. 좌표 없는 시설에는 위치를 추정하지 않는다. 이 연결은 선택한 시설을 표시하는 방식이며 전국 대피시설 전체를 OSM 기본 데이터에 합치지 않는다.

원래 서명된 Flatpak 앱을 수정하지 않고, 수정 바이너리를 `dev/runtime/maps/overlay/bin/OMaps`에 별도로 둔다. 기존 Organic Maps Flatpak의 리소스와 KDE 런타임을 사용한다. Rust 실행기는 `maps.json`의 지도 파일과 수정 바이너리 SHA-256을 확인한 후 네트워크 차단 상태로 실행한다. 수정 바이너리가 없거나 다르면 기존 화면으로 조용히 대체하지 않고 오류를 표시한다.

개발 SDK는 사용자용 USB에 필요하지 않다. 최종 USB 이미지에는 실행 런타임과 지도·폰트·수정 바이너리 및 각 구성 요소의 고지·대응 소스 제공 방법을 담아야 한다. 이 디렉토리의 소스 패치는 부팅 이미지 자체가 아니다.

## 위치 수신 범위

기존 Qt/GeoClue 위치 서비스를 사용한다. 지도 파일이나 노트북 Wi-Fi만으로 오프라인 GPS가 생기는 것은 아니다. 실제 오프라인 현재 위치에는 GNSS 수신기와 운영체제 위치 서비스 연결이 필요하다. 위치가 없거나 15초 이상 오래됐거나 시각이 미래로 잘못 들어오면 현재 위치 기반 안내를 시작하지 않는다. 출발점을 수동 선택하는 경로 계산은 계속 사용할 수 있다.

현재 개발 환경에는 GNSS 수신기가 없어 실제 위성 수신·이동·재탐색은 검증 대상 하드웨어가 필요하다. 시계 경계 검사나 수동 경로 계산을 실제 GPS 검증으로 표시하지 않는다.
