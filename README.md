# Korea SOS Atlas Kit

인터넷이 끊긴 상황에서도 PC에서 한국어 재난·생존 자료를 찾아 읽고, 가까운 대피시설을 확인할 수 있는 휴대용 앱입니다. USB나 아무 폴더에 복사해서 설치 없이 실행합니다.

> 이 앱은 구조 요청 서비스가 아닙니다. 위급할 때는 119·112에 먼저 연락하세요. AI 답변은 실험 기능이며, 답변 아래에 붙는 원문 자료가 우선합니다.

## 할 수 있는 것

- **자료 검색과 열람**: 정전·화재·지진·응급처치 같은 재난·생존 자료를 한국어로 검색하고 읽습니다. 각 자료에는 출처와 원문이 함께 있습니다.
- **대피시설 찾기**: 행정안전부 공공데이터의 민방위 대피시설, 비상급수시설, 응급의료기관을 이름·주소로 찾고, 좌표(`@37.5662,126.9777`)를 넣으면 가까운 순서(직선거리)로 보여 줍니다.
- **로컬 AI(선택)**: 버튼을 눌러 켜면 PC 안에서만 도는 소형 모델(Gemma 4 E2B + llama.cpp)이 검색한 자료를 근거로 답합니다. 인터넷을 쓰지 않으며, 켜기 전에는 실행되지 않습니다.
- **지도**: 오프라인 지도(Organic Maps 수정판)를 Windows·macOS용으로 준비하고 있습니다.

## 받기와 실행

배포판은 준비 중입니다. 준비되면 이 저장소의 Releases에 Windows·macOS용 휴대용 폴더를 올립니다.

1. Releases에서 운영체제에 맞는 압축 파일을 받아 USB나 원하는 폴더에 풉니다.
2. `korea-sos-atlas-kit`(Windows는 `korea-sos-atlas-kit.exe`)을 실행합니다.

```text
KoreaSOSAtlasKit/
  korea-sos-atlas-kit(.exe)     실행 파일
  data/
    content-library/            자료
    assets.json                 AI 모델·엔진 목록과 SHA-256 (Full만)
    models/                     AI 모델 (Full만)
    runtime/                    운영체제별 추론 엔진·지도
```

- **Full**은 AI 모델과 엔진을 포함하고, **Light**는 빼서 작습니다. Light에서도 자료 검색·열람·대피시설 찾기는 모두 됩니다.
- AI를 켤 때 모델과 엔진 파일의 SHA-256을 확인합니다. 가용 메모리가 약 4GB보다 적으면 AI를 켜지 않습니다.
- 지원 운영체제: Windows 10/11 x64(별도 설치 필요 없음), macOS(준비 중).

## 사용법

| 키 | 동작 |
|---|---|
| `/` | 검색 (여러 단어는 모두 포함된 자료) |
| `Tab` | 분류 바꾸기 |
| 방향키, 마우스 | 이동, 선택 |
| `Enter` | 열기 |
| `i` | 출처 정보 |
| `o` | 원문 파일 열기 |
| `q` | 종료 |

새 PC에서 AI가 실제로 도는지 화면 없이 확인하려면 `korea-sos-atlas-kit --ai-check`를 실행합니다.

## 소스에서 빌드

Rust(Cargo)가 필요합니다.

```sh
./build.sh
```

배포판에 들어가는 구성 요소의 빌드 방법은 다음에 있습니다.

- Windows용 llama.cpp 엔진: [integrations/llama.cpp/build-windows.sh](integrations/llama.cpp/build-windows.sh)
- Organic Maps 수정판: [integrations/organicmaps/](integrations/organicmaps/README.md)

자료·모델·지도 데이터는 크기와 권리 조건 때문에 이 저장소에 넣지 않고 배포판으로 제공합니다.

## 라이선스

앱은 **AGPL-3.0-only**입니다. 자료, 지도 데이터, 모델, 엔진은 각각의 조건을 따릅니다. [라이선스 범위](LICENSING.md)와 [LICENSES/](LICENSES/)를 확인하세요.
