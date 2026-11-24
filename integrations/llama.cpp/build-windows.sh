#!/bin/sh
# Build llama.cpp b12375 (the same commit as the Linux engine) for Windows x64
# with llvm-mingw, and install llama-server plus every DLL it needs into
# dev/runtime/llama.cpp-win-x64. The official Windows zip needs the Microsoft
# Visual C++ runtime, which a freshly installed Windows does not have; this
# build ships llvm-mingw's own open-source runtime DLLs instead, so the engine
# runs on Windows 10/11 without installing anything.
#
# Tools (project-local, no system install):
#   ~/.local/share/korea-sos-atlas-kit/llvm-mingw-20261104-ucrt-ubuntu-22.04-x86_64
#   ~/.local/share/korea-sos-atlas-kit/cmake-4.4.5-linux-x86_64
#   ~/.local/share/korea-sos-atlas-kit/src/llama.cpp-b12375  (git tag b12375)
set -eu
TOOLS="${HOME}/.local/share/korea-sos-atlas-kit"
MINGW="$TOOLS/llvm-mingw-20261104-ucrt-ubuntu-22.04-x86_64"
CMAKE="$TOOLS/cmake-4.4.5-linux-x86_64/bin/cmake"
SRC="$TOOLS/src/llama.cpp-b12375"
COMMIT=9d25e0014c4b4b316873c01f4938c0e3c687287d
PROJECT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$PROJECT/dev/runtime/llama.cpp-win-x64"
BUILD="$TOOLS/build/llama.cpp-b12375-win-x64"

[ "$(git -C "$SRC" rev-parse HEAD)" = "$COMMIT" ] || { echo "source is not $COMMIT" >&2; exit 1; }
export PATH="$MINGW/bin:$PATH"

"$CMAKE" -S "$SRC" -B "$BUILD" -G "Unix Makefiles" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_SYSTEM_PROCESSOR=x86_64 \
    -DCMAKE_C_COMPILER=x86_64-w64-mingw32-clang \
    -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-clang++ \
    -DCMAKE_RC_COMPILER=x86_64-w64-mingw32-windres \
    -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF \
    -DLLAMA_BUILD_SERVER=ON -DLLAMA_BUILD_TOOLS=ON \
    -DLLAMA_BUILD_NUMBER=12375 \
    -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_UI=OFF -DLLAMA_USE_PREBUILT_UI=OFF \
    -DBUILD_SHARED_LIBS=ON -DGGML_CCACHE=OFF \
    "-DCMAKE_C_FLAGS=-D_WIN32_WINNT=0x0A00" "-DCMAKE_CXX_FLAGS=-D_WIN32_WINNT=0x0A00" \
    -DGGML_NATIVE=OFF -DGGML_BACKEND_DL=ON -DGGML_CPU_ALL_VARIANTS=ON \
    -DGGML_OPENMP=ON
"$CMAKE" --build "$BUILD" -j"$(nproc)" --target llama-server
# Backend variants are loaded at run time, so the server target does not pull them in.
"$CMAKE" --build "$BUILD" -j"$(nproc)" --target $(
    "$CMAKE" --build "$BUILD" --target help | sed -n 's/^\.\.\. \(ggml-cpu-[a-z0-9]*\)$/\1/p' | tr '\n' ' ')

rm -rf "$OUT.new"
mkdir -p "$OUT.new"
cp "$BUILD"/bin/llama-server.exe "$BUILD"/bin/*.dll "$OUT.new/"
for dll in libc++.dll libunwind.dll libomp.dll libwinpthread-1.dll; do
    cp "$MINGW/x86_64-w64-mingw32/bin/$dll" "$OUT.new/"
done
cp "$SRC/LICENSE" "$OUT.new/LICENSE-llama.cpp"
cp "$MINGW/LICENSE.TXT" "$OUT.new/LICENSE-llvm-mingw.txt"
cat > "$OUT.new/BUILD-INFO.txt" <<EOF
llama.cpp b12375 (commit $COMMIT), built by integrations/llama.cpp/build-windows.sh
toolchain: llvm-mingw 20261104 (ucrt), cmake 4.4.5
options: _WIN32_WINNT=0x0A00 (Windows 10+) LLAMA_BUILD_UI=OFF (no built-in web UI) BUILD_SHARED_LIBS=ON GGML_NATIVE=OFF GGML_BACKEND_DL=ON GGML_CPU_ALL_VARIANTS=ON GGML_OPENMP=ON LLAMA_OPENSSL=OFF
runtime DLLs from llvm-mingw: libc++.dll libunwind.dll libomp.dll libwinpthread-1.dll
EOF

# Every DLL the engine imports must be in this folder or part of Windows itself.
missing=""
for f in "$OUT.new"/*.exe "$OUT.new"/*.dll; do
    for dep in $(llvm-objdump -p "$f" | sed -n 's/^ *DLL Name: //p'); do
        [ -e "$OUT.new/$dep" ] && continue
        case "$(echo "$dep" | tr 'A-Z' 'a-z')" in
            kernel32.dll|user32.dll|advapi32.dll|ws2_32.dll|ntdll.dll|bcrypt.dll|bcryptprimitives.dll|shell32.dll|ole32.dll|crypt32.dll|api-ms-win-*) ;;
            *) missing="$missing $(basename "$f"):$dep" ;;
        esac
    done
done
[ -z "$missing" ] || { echo "unresolved imports:$missing" >&2; exit 1; }

rm -rf "$OUT"
mv "$OUT.new" "$OUT"
echo "installed $OUT"
