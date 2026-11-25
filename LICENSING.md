# 라이선스 범위

자료별 출처와 이용 조건은 앱의 출처 정보(`i`)와 배포판 자료 폴더의 자료별 기록에 함께 들어 있습니다. 라이선스별 조건과 준수 방법, 자료 목록은 [LICENSES/CONTENT.md](LICENSES/CONTENT.md)에 있습니다. 원본 자료, 편집한 안내, 번역문과 앱 코드는 서로 다른 권리 조건을 가질 수 있습니다.

활성 Rust 앱과 프로젝트 자체 검증 코드는 **AGPL-3.0-only**로 제공합니다. 전문은 [LICENSES/AGPL-3.0-only.txt](LICENSES/AGPL-3.0-only.txt)에 보존합니다.

기존 검색 모듈의 유래인 [pleasereadthisforme](https://github.com/pleasereadthisforme/pleasereadthisforme)의 고지와 권리 조건은 유지합니다. 이 모듈을 처음 옮긴 이전 Python 구현은 공개 저장소에 포함하지 않으며, 그 이식 내역과 출처 명세는 [attribution/](attribution/SEARCH-NOTICE.md)에 보존합니다. 활성 앱이 이 모듈을 실행하거나 연결한다고 표시하지 않습니다. 원 프로젝트의 라이선스·브랜드 고지는 `LICENSES/pleasereadthisforme/`에도 보존합니다. 이번 정리는 허용적 재라이선스나 추가 사용 허가를 뜻하지 않습니다.

Cargo 의존성, llama.cpp, 모델 가중치, 원본 PDF와 콘텐츠는 각각의 권리 조건을 따릅니다. 앱 라이선스로 일괄 재라이선스하지 않습니다. 로컬 `dev/` 자료·모델·엔진과 `bak/`는 공개 소스 배포에 포함하지 않습니다. 바이너리·USB 이미지 공개 시에는 해당 버전에 대응하는 소스와 구성 요소별 고지·배포 조건을 함께 충족해야 합니다.

Windows 추론 엔진 폴더(`runtime/llama.cpp-win-x64`)에는 llama.cpp(MIT, `LICENSE-llama.cpp`)와 llvm-mingw의 런타임 DLL(`libc++.dll`, `libunwind.dll`, `libomp.dll`: Apache-2.0 WITH LLVM-exception / `libwinpthread-1.dll`: MinGW-w64 고지)이 들어 있으며, 각 고지를 `LICENSE-llvm-mingw.txt`로 함께 둡니다. 빌드 방법과 버전은 같은 폴더의 `BUILD-INFO.txt`와 `integrations/llama.cpp/build-windows.sh`에 있습니다.
