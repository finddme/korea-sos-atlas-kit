# Korea SOS Atlas Kit

인터넷이 끊긴 상황에서도 PC에서 한국어 재난·생존 자료를 찾아 읽고, 가까운 대피시설을 확인할 수 있는 휴대용 앱입니다. USB나 아무 폴더에 복사해서 설치 없이 실행합니다.

> 이 앱은 구조 요청 서비스가 아닙니다. 위급할 때는 119·112에 먼저 연락하세요. AI 답변은 실험 기능이며, 답변 아래에 붙는 원문 자료가 우선합니다.

## 할 수 있는 것

- **자료 검색과 열람**: 정전·화재·지진·응급처치 같은 재난·생존 자료를 한국어로 검색하고 읽습니다. 각 자료에는 출처와 원문이 함께 있습니다.
- **대피시설 찾기**: 행정안전부 공공데이터의 민방위 대피시설, 비상급수시설, 응급의료기관을 이름·주소로 찾고, 좌표(`@37.5662,126.9777`)를 넣으면 가까운 순서(직선거리)로 보여 줍니다.
- **로컬 AI(Atlas + AI)**: AI 추가팩을 함께 풀면, 버튼을 눌러 켤 때 PC 안에서만 도는 소형 모델(Gemma 4 E2B + llama.cpp)이 검색한 자료를 근거로 답합니다. 인터넷을 쓰지 않으며, 켜기 전에는 실행되지 않습니다.
- **지도(Windows)**: 오프라인 한국 지도(Organic Maps 수정판)로 장소 검색, 도보·차량 경로, 대피시설을 목적지로 보내기를 할 수 있습니다. `4`로 열고, 대피시설 상세에서 `m`을 누르면 목적지로 표시됩니다. 처음 열 때 Organic Maps 이용 약관 화면에서 수락을 눌러야 합니다. 지도는 인터넷에 연결하지 않으며(내려받기·업데이트·업로드 기능을 꺼 둠), 그래픽 드라이버가 없는 PC에서는 자동으로 CPU 렌더링으로 다시 엽니다(느릴 수 있음).

## 받기와 실행

배포판은 이 저장소의 Releases에 올립니다. 두 가지로 쓸 수 있습니다.

| 구성 | 받을 파일 | 크기 | 할 수 있는 것 |
|---|---|---|---|
| **Atlas** | `korea-sos-atlas-kit-atlas-windows-x64.zip` | 약 430MB | 자료 검색·열람, 대피시설 찾기, 지도 |
| **Atlas + AI** | 위 파일 + `korea-sos-atlas-kit-ai-pack-1.zip` + `korea-sos-atlas-kit-ai-pack-2.zip` | 약 3.2GB | Atlas 기능 + 로컬 AI 답변 |

1. 받은 zip을 **모두 같은 위치**에 풉니다. 세 파일 모두 안에 `KoreaSOSAtlasKit` 폴더가 들어 있어서, 같은 곳에 풀면 한 폴더로 합쳐집니다. Windows 탐색기의 "압축 풀기"는 기본 위치 끝에 zip 이름 폴더를 붙이므로, 세 파일 모두 같은 위치를 지정하세요.
2. `KoreaSOSAtlasKit\korea-sos-atlas-kit.exe`를 실행합니다. 설치는 필요 없고, 폴더째 USB에 복사해 다른 PC에서도 쓸 수 있습니다.
3. Atlas를 먼저 쓰다가 나중에 AI 추가팩만 받아 같은 위치에 풀어도 됩니다.

받은 파일이 손상되지 않았는지는 Releases의 `SHA256SUMS.txt`와 비교해 확인할 수 있습니다(PowerShell: `Get-FileHash 파일이름`).

```text
KoreaSOSAtlasKit/
  korea-sos-atlas-kit.exe       실행 파일
  README.md, LICENSING.md, LICENSES/
  data/
    assets.json                 AI 모델·엔진·지도 프로그램의 SHA-256 목록
    content-library/            자료(공개 승인 자료만)
    runtime/organicmaps-win-x64 지도 프로그램
    runtime/llama.cpp-win-x64   AI 추론 엔진 (AI 추가팩)
    models/                     AI 모델 2조각 (AI 추가팩)
```

- AI를 켤 때 모델과 엔진 파일의 SHA-256을 확인합니다. AI 추가팩이 없으면 받는 방법을 안내합니다.
- 메모리: AI는 답하는 동안 약 1.6GB를 씁니다. 사용 가능한 메모리가 3GB 이상이면 바로 켜고, 1.5~3GB이면 느려질 수 있다고 알린 뒤 `y`로 켤지 묻고, 1.5GB 미만이면 켜지 않습니다. 실행 중 0.5GB 아래로 떨어지면 AI를 끄고 모델을 메모리에서 내립니다. 앱 창을 닫으면 AI도 함께 종료됩니다.
- 지원 운영체제: Windows 10/11 x64(별도 설치 필요 없음).

## 사용법

| 키 | 동작 |
|---|---|
| `/` | 검색 (여러 단어는 모두 포함된 자료) |
| `Tab` | 분류 바꾸기 |
| 방향키, 마우스 | 이동, 선택 |
| `Enter` | 열기 |
| `i` | 출처 정보 |
| `o` | 원문 파일 열기 |
| `c` | 복사(AI 칸이 열려 있으면 답변, 아니면 열린 문서) |
| `q` | 종료 |

**AI에게 묻기:** `2`(또는 `[2 Show AI]`)를 누르면 AI 칸 아래쪽 질문칸에 바로 입력할 수 있습니다. AI가 꺼져 있어도 질문을 쓰고 Enter를 누르면 AI를 켠 뒤 준비되는 대로 답합니다(답변까지 보통 1분 안팎). `s`를 누르면 답변의 근거 원문을 왼쪽에 모아 스크롤로 읽을 수 있고, 다시 누르면 다음 근거로 건너뜁니다. `p`는 AI 켜기·끄기입니다. "근처 대피소"처럼 시설 위치를 묻는 질문에는 앱의 대피시설 검색 방법을 바로 안내합니다.

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

앱은 **AGPL-3.0-only**입니다. 자료, 지도 데이터, 모델, 엔진은 각각의 조건을 따르며 앱 라이선스로 바뀌지 않습니다. [라이선스 범위](LICENSING.md)와 [LICENSES/](LICENSES/)를 확인하세요.

**자료(공개 승인 105건 + 미군 교범 2종 299항목)**: 재배포·번역·편집이 모두 허용된 자료만 넣었습니다. 자료마다 출처·이용 조건·편집 내역이 앱의 출처 정보(`i`)와 자료 폴더의 `NOTICE.txt`에 있습니다.

| 이용 조건 | 예 |
|---|---|
| 영국 정부 Open Government Licence v3.0 | NHS, UKHSA, 영국 외교부 |
| CC BY 4.0 / 3.0 | EU 집행위원회, 뉴질랜드 민방위청, 남호주 소방청 |
| CC BY-SA 4.0 | Wikibooks, Wikivoyage. 한국어 편집본도 CC BY-SA 4.0 |
| 공공누리 제0·1유형 | 질병관리청, 소방청, 행정안전부 등 |
| 미국 연방정부 저작물(퍼블릭 도메인) | FDA, NPS, NWS, 국토안보부, 미군 교범 |
| 공공데이터포털 "이용허락범위 제한 없음" | 민방위 대피·급수시설, 응급의료기관 |

한국어 편집본은 원 기관의 공식 번역이나 승인본이 아니며, 원 기관이 보증하지 않습니다. 사진·로고·제3자 자료는 넣지 않았습니다.

**지도**: Organic Maps 수정판(Apache-2.0)과 OpenStreetMap 기반 지도 데이터(ODbL, Organic Maps Binary Data License). 지도 프로그램 폴더의 `licenses/`에 Qt, Mesa, Visual C++ 런타임 등의 고지가 함께 있습니다. **AI**: 모델 Gemma 4 E2B, 엔진 llama.cpp(MIT). [LICENSES/MODELS.md](LICENSES/MODELS.md)를 확인하세요.
