#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run inside org.kde.Sdk//6.10. Arguments: prepared upstream checkout, build directory.
set -euo pipefail
source_dir=$(realpath "${1:?upstream checkout required}")
build_dir=$(realpath -m "${2:?build directory required}")
patch_dir=$(cd -- "$(dirname -- "$0")" && pwd)
expected=d81f9197c6759f67aaed60f4f9c819e834dd02cd
[[ $(git -C "$source_dir" rev-parse HEAD) == "$expected" ]] || { echo 'Unexpected upstream revision' >&2; exit 1; }
if git -C "$source_dir" submodule status --recursive | grep '^[-+U]' >/dev/null; then
  echo 'Initialize submodules at the pinned revisions first' >&2
  exit 1
fi
if git -C "$source_dir" apply --check "$patch_dir/desktop-usability.patch"; then
  git -C "$source_dir" apply "$patch_dir/desktop-usability.patch"
elif ! git -C "$source_dir" apply --reverse --check "$patch_dir/desktop-usability.patch"; then
  echo 'Patch does not match the checkout' >&2
  exit 1
fi
(cd "$source_dir" && bash configure.sh)
cmake -S "$source_dir" -B "$build_dir" -GNinja -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTING=OFF -DCMAKE_UNITY_BUILD_BATCH_SIZE=8 -DUSE_CCACHE=ON
cmake --build "$build_dir" --target desktop -j"${SOS_BUILD_JOBS:-2}"
g++ -std=c++23 "$source_dir/qt/atlas_location_tests.cpp" -o "$build_dir/atlas-location-tests"
"$build_dir/atlas-location-tests"
echo "Built: $build_dir/OMaps (GUI verification required before deployment)"
