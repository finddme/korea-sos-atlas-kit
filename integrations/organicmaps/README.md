# Organic Maps 데스크톱 사용성 개선

Organic Maps 데스크톱(C++/Qt) 창에 한국어 화면, 장소 검색 기본 노출, 출발·도착 선택, 도보/차량 전환, 거리·예상 시간·이동 단계, 장소 이름·메모 저장, GPS 상태 표시를 추가한다. Korea SOS Atlas Kit 본체는 Rust이며 지도는 별도 프로세스로 실행한다. 음성 안내는 포함하지 않는다.

Organic Maps 이름·로고·지도 고지를 보존하고, 수정판임을 창 제목과 정보 화면에 표시한다. 원래 지도와 경로 엔진은 바꾸지 않는다. MAPS.ME 코드를 복사하지 않았다.

## 소스와 권리

- upstream: https://github.com/organicmaps/organicmaps
- 원래 코드와 이 디렉토리의 패치는 Apache-2.0이다. `LICENSE`, `NOTICE`를 함께 보존한다.
- 지도 데이터(`.mwm`)는 코드와 별개인 Organic Maps Binary Data License를 따른다. [지도 권리 대장](../../LICENSES/MAPS.md)을 참고한다.
- 바이너리를 배포할 때 원본의 `LICENSE`·`NOTICE`·`DATA_LICENSE.txt`·`LICENSES/`, 의존성 고지, 이 패치를 함께 둔다.

| 플랫폼 | upstream 기준 | 패치 | 빌드 |
|---|---|---|---|
| Windows x64 | 릴리스 `2026.11.11-39` (`5c0c5ec0a706b9bea17b4d5b0b40e7b60f916a13`) — upstream Windows 지원이 들어간 첫 릴리스 계열 | `desktop-usability-2026.11.patch` | GitHub Actions `.github/workflows/organicmaps-windows.yml` |
| Linux x86_64 | `d81f9197c6759f67aaed60f4f9c819e834dd02cd` (2026-05-28) | `desktop-usability.patch` | `build.sh` (KDE SDK 6.10 flatpak) |

두 패치는 같은 기능이다. Windows용은 Linux용을 새 기준에 맞춰 옮긴 것으로, upstream이 새로 쓰는 번역 키(`Tr("clear_route")` 등)와 새 기능(공유 메뉴, 저장 색상)을 유지하고 이 프로젝트의 동작 변경만 다시 적용했다. upstream 한국어 번역이 비어 있는 화면 문구는 `qt/atlas_ko.json`이 채운다. Linux는 Flathub 런타임의 데이터 파일과 맞춰야 해서 기존 기준을 유지한다.

## Windows 빌드 (GitHub Actions)

`organicmaps-windows.yml`은 upstream의 Windows CI 단계(MSVC, Qt 6.8.3 msvc2022_64 + qtpositioning, `ci-windows` 프리셋)를 그대로 따른다. 그다음 패치를 적용하고 `OMIM_WINDOWS_DISTRIBUTION=direct`로 빌드한 뒤, upstream의 배포 준비 스크립트(`tools/windows/stage_runtime.py`)로 실행 폴더를 만든다.

- upstream 배포 준비는 Visual C++ 런타임을 넣지 않는다. 그래서 워크플로가 `vcruntime140*.dll`·`msvcp140*.dll`을 실행 파일 옆에 함께 둔다. 새로 설치한 Windows 10/11에서도 따로 설치할 것이 없다.
- CPU 렌더링용으로 Mesa 26.2.5 llvmpipe(pal1000/mesa-dist-win의 MSVC 빌드, 버전·SHA-256 고정)를 `mesa/`에 둔다. Mesa·LLVM 라이선스는 `licenses/mesa/`에 넣는다. 그래픽 드라이버가 정상인 PC에서는 쓰이지 않는다.
- 결과 폴더에는 `licenses/korea-sos-atlas-kit/`(이 패치, 수정 고지, 빌드 기록)와 전체 파일의 `SHA256SUMS`가 들어간다. 지도 데이터는 넣지 않는다.
- 워크플로 결과물(artifact) `organicmaps-windows-x64`를 받아 키트의 `data/runtime/organicmaps-win-x64/`에 둔다. 키트의 `assets.json`이 `SHA256SUMS`의 해시를 고정하고, 앱은 지도를 열기 전에 모든 파일을 대조한다.

## Linux 빌드

KDE SDK 6.10, GCC, CMake, Ninja, Git이 있는 개발 환경에서 실행한다. 최종 사용자는 이 도구들을 설치하지 않는다. 소스와 서브모듈을 받을 때만 네트워크가 필요하고, 컴파일과 실행은 오프라인으로 한다.

```sh
# 작업 폴더(예: build/organicmaps)에서 실행
git clone --filter=blob:none --no-checkout https://github.com/organicmaps/organicmaps upstream
git -C upstream checkout d81f9197c6759f67aaed60f4f9c819e834dd02cd
git -C upstream submodule update --init --recursive --depth 1
flatpak run --user --unshare=network --filesystem="$PWD" --command=bash org.kde.Sdk//6.10 \
  <키트 소스>/integrations/organicmaps/build.sh "$PWD/upstream" "$PWD/build"
```

스크립트는 패치 적용, 컴파일, 위치 시각 경계 검사를 한다. GUI 검증까지 자동으로 통과한 것으로 보지 않는다. 기본 병렬도는 2이며 `SOS_BUILD_JOBS`로 바꿀 수 있다. Linux에서는 서명된 Flathub 앱을 고치지 않고, 수정한 실행 파일만 따로 두어 Flathub 앱의 데이터 파일과 KDE 런타임을 쓴다. Linux의 한글 입력기 런타임(Fcitx5)은 개인용 Linux 구성에만 쓰며 이 저장소의 배포 대상이 아니다.

## 실행 구성

공식 시설은 TUI의 `대피시설` 분류에서 고르고 `m`으로 지도에 보낸다. 해시 검증된 시설의 좌표만 상태 파일에 원자적으로 기록하고, 지도는 `SOS_ATLAS_REQUEST` 환경 변수로 받은 파일을 읽어 목적지를 설정한다. 좌표가 없는 시설은 위치를 추정하지 않는다. 이 연결은 선택한 시설을 표시하는 방식이며 전국 대피시설을 OSM 지도 데이터에 합치지 않는다.

좌표가 있는 민방위 대피시설·비상급수시설은 지도를 열기 전에 시·도별 KML(`korea-sos-NN.kml`, 문서 이름 `대피시설 · <시·도>`)로 지도 프로그램의 저장한 장소 폴더에 쓴다(`src/map_places.rs`). 시설 자료가 바뀔 때만 다시 쓰고, 사용자가 만든 저장 장소 파일은 건드리지 않는다. 확대 13 미만에서는 숨긴다. 지도의 `가까운 대피시설` 칸(`qt/atlas_nearby.cpp`)은 오른쪽 도구 막대의 단추로 열고(기본은 장소 검색), 이 목록만 읽어 경로 출발지, 검색·선택한 장소, GPS 위치, 지도 중심 순으로 기준을 잡고 가까운 30곳을 직선거리로 정렬한다. 선택한 장소 표시(`libs/drape_frontend/selection_shape_generator.cpp`)는 원래의 파란 원 대신 연한 노랑(#FFB300, 약간 투명) 원에 보라(#7C4DFF) 테두리로 그려, 파란 검색 결과 핀·비상급수 표시와 빨간 대피시설 표시와 구분되게 한다. Windows판(2026.11 패치)은 선택한 장소의 파란 검색 핀을 선택하는 동안 숨긴다(`libs/map/search_mark.*`, `framework.cpp`). 그 핀은 6픽셀 아래, 검색 결과 중심에 그려져 선택 원과 어긋나 보였다. 다른 검색 핀은 그대로 두고, 선택을 풀면 다시 보인다.

앱은 지도 파일(`maps.json`)과 지도 프로그램의 해시를 확인한 뒤에만 지도를 연다. 맞지 않으면 다른 프로그램으로 조용히 대체하지 않고 오류를 표시한다. Linux는 네트워크를 차단한 샌드박스에서 실행한다. Windows는 관리자 권한 없이 실행해야 해서 운영체제 방화벽 대신 지도 프로그램 안에서 네트워크를 끈다. 키트가 `SOS_ATLAS_OFFLINE=1`로 실행하면 모든 HTTP 요청이 소켓을 열기 전에 실패하고(`libs/platform/atlas_offline.hpp`, `http_client_qt.cpp`), 지도 내려받기 단추·환영 화면·내려받기/재시도 단추가 사라진다. 키트에 없는 지역은 "오프라인 키트에 들어 있지 않습니다"라고만 표시한다. 지도 보기·검색·경로 계산에는 네트워크가 필요하지 않다.

그래픽 드라이버가 없거나 OpenGL 3.2를 지원하지 않는 PC에서는 지도 엔진(drape)이 그릴 수 없다. drape는 `opengl32.dll`을 직접 호출하므로 Qt의 `opengl32sw.dll`로는 그리지 못하고 흰 화면이 된다. 그래서 패치는 지도 창을 만들 때 `opengl32.dll`이 쓸 만한 OpenGL 3.2 이상을 주는지 확인하고, 아니면 코드 75로 종료한다. 앱은 시작 20초 안의 비정상 종료를 감지하면 `SOS_ATLAS_SOFTWARE_GL=1`로 한 번 다시 연다. 이때 지도 프로그램은 `opengl32.dll`을 지연 로드하도록 빌드돼 있어서, 무엇보다 먼저 `mesa/opengl32.dll`(Mesa)을 같은 이름으로 불러(앱이 `GALLIUM_DRIVER=llvmpipe`를 함께 줘서 그래픽 드라이버가 필요한 D3D12 경로 대신 CPU 렌더러 llvmpipe를 쓴다) drape와 Qt가 모두 CPU 렌더러를 쓴다. 앱은 상태 폴더에 `opengl-software` 파일을 남겨 다음부터 바로 CPU 렌더링으로 연다. 이 파일을 지우면 다시 그래픽 카드를 쓴다.

## 위치 수신 범위

운영체제 위치 서비스(Qt Positioning)를 사용한다. 지도 파일만으로 오프라인 GPS가 생기지는 않으며, 실제 현재 위치에는 GNSS 수신기와 운영체제 위치 서비스가 필요하다. 위치가 없거나 15초 이상 오래됐거나 시각이 미래로 잘못 들어오면 현재 위치 기반 안내를 시작하지 않는다. 출발점을 직접 고르는 경로 계산은 계속 쓸 수 있다. 실제 위성 수신·이동·재탐색은 수신기가 없어 검증하지 못했다.
