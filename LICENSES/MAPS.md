# 지도 데이터와 지도 프로그램 권리

검토일: 2026-11-14, 배포판 기준 갱신: 2026-11-25. 앱 자체 라이선스(AGPL-3.0-only)와 지도 데이터·지도 프로그램의 라이선스는 서로 다르며, 앱 라이선스로 바꾸지 않습니다.

## 배포판에 들어 있는 것

| 구성 | 내용 | 권리 |
|---|---|---|
| 지도 프로그램 | Organic Maps 데스크톱 릴리스 `2026.11.11-39`(`5c0c5ec0a706b9bea17b4d5b0b40e7b60f916a13`)에 이 저장소의 [사용성 개선 패치](../integrations/organicmaps/desktop-usability-2026.11.patch)를 적용해 GitHub Actions로 빌드한 Windows x64 판 | Apache-2.0. 원본 `LICENSE`·`NOTICE`와 패치, 빌드 기록을 배포판 `data/runtime/organicmaps-win-x64/licenses/`에 보존 |
| 함께 든 실행 구성 요소 | Qt 6.8.3, Visual C++ 런타임, CPU 렌더링용 Mesa 26.2.5(llvmpipe) | 각 고지를 같은 `licenses/` 폴더에 보존(Mesa·LLVM 라이선스 포함) |
| 지도 데이터 | 대한민국 북부·남부 지도 `260527`판(`South Korea_North.mwm` 197,402,970바이트, `South Korea_South.mwm` 167,567,274바이트)과 지도 프로그램에 든 세계 기본 지도(`World.mwm`, `WorldCoasts.mwm`) | Organic Maps Binary Data License([원문](Organic-Maps-DATA-LICENSE.txt)). OpenStreetMap ODbL 기반 Produced Work |

지도 파일과 지도 프로그램은 배포판 `data/assets.json`과 `content-library/maps.json`에 SHA-256이 고정되어 있고, 앱은 지도를 열기 전에 모든 파일을 대조합니다.

## Organic Maps 지도 데이터 조건과 지킨 방법

| 조건(Binary Data License) | 지킨 방법 |
|---|---|
| 지도와 정보 화면·메인 메뉴 등 보이는 곳에 "Map data © OpenStreetMap and Organic Maps" 표시 | 지도 프로그램 창 왼쪽 아래와 정보 화면에 원래 고지 유지. 키트 앱에서는 Settings 화면, 지도 분류, 지도를 열 때 알림에 같은 문구 표시 |
| Organic Maps·OpenStreetMap 표기마다 `https://organicmaps.app`, `https://openstreetmap.org/copyright` 링크 | 지도 프로그램은 누를 수 있는 링크로 표시. 키트 앱 Settings 화면과 README에 두 주소를 함께 표시(Windows Terminal에서는 주소를 눌러 열 수 있음) |
| 출처 표시 없이 지도 파일을 배포하지 않기 | 위 표시와 이 문서, 원문 라이선스를 배포판에 함께 둠 |
| 화이트라벨·리브랜딩 금지(별도 서면 허가 필요) | 이름·로고·지도 고지를 바꾸지 않음. 창 제목은 "Organic Maps · Korea SOS Atlas Kit 사용성 개선판"으로 Organic Maps 이름을 유지하고 수정판임만 덧붙임. 지도 파일을 키트 자체 지도라고 표기하지 않음 |

지도 데이터에는 OpenStreetMap(ODbL), Wikipedia 문구(CC BY-SA 4.0), 지형 자료 등 원래 출처가 들어 있습니다. 한국어 지명은 원래 지도 데이터 그대로이며 별도로 번역·편집하지 않았습니다.

- OpenStreetMap: https://www.openstreetmap.org/copyright , ODbL 1.0: https://opendatacommons.org/licenses/odbl/1-0/
- Organic Maps 소스: https://github.com/organicmaps/organicmaps

## 이용 범위

- 지도 프로그램은 오프라인 모드로 실행되어 인터넷 요청을 하지 않습니다(지도 내려받기·업데이트·편집 업로드를 막음).
- 위치는 Windows 위치 서비스를 통해 받으며, 지도 파일만으로 GPS가 생기지는 않습니다.
- 공식 대피·급수시설은 행정안전부 공공데이터(별도 권리)에서 오며, OpenStreetMap 시설 목록으로 대체하지 않았습니다. 키트에서 고른 시설 좌표만 지도에 목적지로 보냅니다.

## Linux 빌드(개발용)

이 저장소에는 Linux용 패치(`integrations/organicmaps/desktop-usability.patch`, Organic Maps `d81f9197` 기준)와 빌드 스크립트도 있습니다. 배포판(Windows)에는 들어가지 않으며, 빌드 방법은 [integrations/organicmaps/](../integrations/organicmaps/README.md)를 참고하세요.

## MAPS.ME 대체 후보 검토 — 2026-11-14

판정: 공개 `mapsme/omim` 코드 활용은 가능하다. 현재 상용 MAPS.ME 앱/서비스와 그 공개 코드의 권리는 별개이며, 현재 앱을 USB 공개 배포물에 그대로 묶어도 된다는 판정은 하지 않는다. 이번에는 조사만 수행했고 기존 지도 구현을 교체하지 않았다.

### 공개 코드

- https://github.com/mapsme/omim/blob/master/LICENSE : Apache-2.0. 소스/바이너리 재배포 및 수정 가능. LICENSE 제공, 수정 파일 변경 고지, 관련 저작권/출처 고지 보존, NOTICE의 관련 고지를 읽을 수 있게 제공해야 한다. 상표 사용 권한까지 부여하지 않는다.
- https://github.com/mapsme/omim/blob/master/NOTICE : 제3자 라이브러리의 별도 조건과 이미지·미디어 저작권 고지를 명시한다. 코드 라이선스만으로 로고·모든 리소스·외부 SDK의 재사용을 일괄 승인하지 않는다. 특정 파일을 실제 이식할 때 해당 고지를 확인한다.
- 기본 브랜치 HEAD: `1892903b63f2c85b16ed4966d21fe76aba06b9ba`, committer 2021-04-28. API의 저장소 pushed_at은 2022-10-05이나 기본 브랜치 최신 코드 날짜와 구분한다. archived=false이므로 '아카이브 저장소'라고 부르지 않는다. https://github.com/mapsme/omim/commit/1892903b63f2c85b16ed4966d21fe76aba06b9ba
- README와 설치 문서는 Qt 5/C++ 데스크톱 빌드를 안내한다. 과거 모바일 화면을 Rust TUI에 그대로 옮기는 구조가 아니다. https://github.com/mapsme/omim/blob/master/docs/INSTALL.md . 구버전 엔진과 새 MWM 지도 호환성·빌드·배포 도구를 다시 검증해야 한다. 이번 조사에서 빌드/실행 비교는 하지 않았다.

### 현재 배포 앱·서비스 및 지도

https://maps.me/terms/ (표기 개정일 2026-09-19): 5.1 개인·비상업적 사용 허가, 5.5.1 대량 제3자 제공/체계적 데이터 취득 제한, 8.3 명시 허가 밖 앱·서비스·콘텐츠의 복제/공개 제공 및 파생물 이용 제한이 있다. 1.3에는 OSM ODbL도 명시돼 있으므로 기초 OSM 데이터 권리와 서비스/가공물/앱 권리를 구분해야 한다. 무료 생존 키트라도 공개 재배포가 개인 이용과 동일하지 않다. 이 약관을 과거 Apache 공개 코드의 사용 금지로 확대 해석하지 않는다. 지도는 권리가 확인된 OSM 공급처에서 독립 취득·생성하거나 별도로 재배포 허가가 확인된 파일을 사용한다.

### 기능 및 채택 판단

Organic Maps 공식 사이트는 같은 MapsWithMe/MAPS.ME 제작자 계보와 오프라인 경로/음성 안내를 명시한다: https://organicmaps.app/ . 그러나 모바일 제품 기능이 현재 연결한 Linux 배포본에 동일한 사용자 경험으로 제공된다는 뜻은 아니다. Linux 공식 FAQ도 데스크톱 앱이 주로 개발·시험 용도라고 설명한다: https://organicmaps.app/faq/linux/how-to-use-flatpak/ . 현재 우리 검증은 경로 계산이며 이동 중 GPS 추종·한국어 음성 안내 실증이 아니다.

권고: 현재 Organic Maps를 유지하면서 한국어 출발/도착 검색·도보/차량 선택·경로 단계 표시를 쉽게 만드는 방향을 우선 검토한다. MAPS.ME의 실제 경로 품질이 더 좋다는 비교 실험 근거는 없다. 공개 구버전 채택만으로 현재 모바일 MAPS.ME의 조작성이 확보된다고 볼 수 없다. 이동 중 안내에는 실제 위치 입력(GPS 등), 음성에는 오프라인 TTS 구성의 별도 검증이 필요하다.
