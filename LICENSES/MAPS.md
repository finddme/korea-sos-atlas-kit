# 지도 데이터와 뷰어 권리 검토

검토일: 2026-11-14. 앱 자체 라이선스와 지도 데이터 라이선스를 구분한다.

## Organic Maps

- 뷰어 소스: https://github.com/organicmaps/organicmaps — Apache-2.0. 원래 브랜드·저작권·제3자 고지를 유지한 별도 뷰어로 실행한다.
- 제공 바이너리 지도: https://github.com/organicmaps/organicmaps/blob/master/DATA_LICENSE.txt — 별도 Organic Maps Binary Data License. 원문은 `Organic-Maps-DATA-LICENSE.txt`에 보존했다.
- 지도와 사용자에게 보이는 메뉴/소개에 `Map data © OpenStreetMap and Organic Maps` 표시 및 양쪽 클릭 가능한 링크가 필요하다. 데이터 재배포는 이 고지 조건을 준수해야 한다. 화이트라벨·리브랜딩은 별도 서면 허가가 필요하므로 하지 않는다.
- 데이터에는 OSM ODbL, Wikipedia CC BY-SA 4.0 및 지형 등 별도 출처가 있다. 배포본 원래 copyright 자료도 함께 보존한다. 다운로드 파일을 SOS 자체 지도라고 재표기하지 않는다.
- OSM: https://www.openstreetmap.org/copyright ; https://opendatacommons.org/licenses/odbl/1-0/ . 복사·재배포·수정 가능, 출처 고지 및 파생 데이터베이스 동일조건 의무. Organic Maps 지도는 해당 제공자 문서상 Produced Work이다.
- 한국어 지명은 원래 지도 데이터 그대로 이용한다. 이번 작업에서 지형/위키 설명의 독자 번역·편집은 하지 않는다.

## 수집 범위와 상태

대한민국 지도와 실행에 필요한 세계 기본 지도만 대상으로 한다. Organic Maps 명칭·고지를 유지하면서 기존 C++/Qt 뷰어의 조작성을 개선하고 Korea SOS Atlas Kit에서 연결 및 한국어 사용 안내를 제공한다. 공식 대피소·급수시설 데이터는 별도 출처 심사 없이는 OSM 시설 목록으로 대체하여 확보 완료 처리하지 않는다.

### 데스크톱 수정판

고정 소스 `d81f9197c6759f67aaed60f4f9c819e834dd02cd`에 적용하는 [수정 패치·빌드 명세](../integrations/organicmaps/README.md)를 보존한다. 이 패치는 Apache-2.0으로 제공하며 upstream LICENSE·NOTICE를 함께 둔다. 변경 파일에 수정 고지를 추가하고 창 제목·정보 화면에 수정판임을 표시했다. 원래 로고·브랜드·저작권 자료와 클릭 가능한 OSM/Organic Maps 및 Organic Maps Project 링크를 유지한다. 지도 데이터는 변경하지 않았다. MAPS.ME 코드를 복사하지 않았다. Rust 본체의 AGPL과 이 별도 실행 뷰어·지도 데이터의 라이선스를 혼동하지 않는다.

수집 전 내용 적합성: 공식 Linux 사용 안내에서 다운로드 후 오프라인 열람·장소 검색·좌표 또는 지도 클릭에 의한 경로 계산을 확인했다. https://organicmaps.app/faq/linux/how-to-use-flatpak/

실제 파일/버전/무결성 및 실사용 검증 상태는 `dev/content-library/sources/지도·위치/korea-organic-maps/record.json`에 기록한다. 이 문서만으로 배포 번들 검증 완료를 뜻하지 않는다.

## 검증된 배포 조합

- 실제 채택 파일은 260527판 대한민국 북부/남부(364,970,244 bytes)이며 뷰어의 countries.txt에 기록된 SHA-1·크기를 모두 대조했다. 추가로 SHA-256을 `maps.json`에 기록해 실행 전 검사한다.
- 채택 버전의 라이선스 원문: https://github.com/organicmaps/organicmaps/blob/d81f9197c6759f67aaed60f4f9c819e834dd02cd/DATA_LICENSE.txt . `Organic-Maps-DATA-LICENSE.txt`는 이 고정 버전이다. 최신 정책도 확인했으며 화이트라벨/출처 제거는 하지 않는다.
- Linux 뷰어 Flatpak commit: `94710ca4dc483f8eb7dca05b1960e24e3d16066fce7128729e33ccb67e2a9b15` (2026.05.28-2). 원래 Organic Maps 명칭·로고·지도 출처·Help/저작권 자료를 유지한다. 지도 화면 왼쪽 하단의 OSM/Organic Maps 고지를 확인했다.
- 코드/포함 자료의 원래 고지는 `dev/runtime/maps/data/flatpak/app/app.organicmaps.desktop/x86_64/stable/active/files/share/organicmaps/data/copyright.html` 및 패키지의 원래 고지에 있다. 이를 공개 번들에서 제거하지 않는다.
- KDE/Qt/Mesa/Flatpak 및 부트스트랩 라이브러리는 앱 자체 AGPL로 재라이선스하지 않는다. Flathub 원래 패키지·메타데이터·저작권 고지를 보존한다. 부트스트랩 Ubuntu 패키지의 고지는 `bootstrap/usr/share/doc/*/copyright`에 있다. GPL/LGPL 등 개별 의존성의 대응 소스 제공 의무도 공개 이미지 제작 시 패키지별로 충족해야 한다. 현재 개발 런타임을 소스 저장소에 업로드하지 않는다.
- 취득 경로: https://flathub.org/apps/app.organicmaps.desktop 및 Ubuntu noble 공식 패키지 저장소. Flatpak 저장소 GPG 검증을 사용했다.

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
