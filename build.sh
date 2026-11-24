#!/bin/sh
set -eu
cd "$(dirname "$0")"
if ! command -v cargo >/dev/null 2>&1; then
    local_tools="${HOME}/.local/share/korea-sos-atlas-kit/rust"
    export CARGO_HOME="$local_tools/cargo" RUSTUP_HOME="$local_tools/rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi
exec cargo build --release --locked --offline "$@"
