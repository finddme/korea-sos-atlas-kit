#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Run inside the viewer's private dbus-run-session, never the desktop IME session.
set -eu
runtime=$1
shift
export FCITX_CONFIG_HOME="$runtime/state/ime"
export FCITX_DATA_DIRS="$runtime/ime/usr/share/fcitx5"
export XDG_DATA_DIRS="$runtime/ime/usr/share:${XDG_DATA_DIRS:-/app/share:/usr/share}"
export LIBHANGUL_KEYBOARD_PATH="$runtime/ime/usr/share/libhangul/keyboards"
export FCITX_ADDON_DIRS="$runtime/ime/usr/lib/x86_64-linux-gnu/fcitx5"
export LD_LIBRARY_PATH="$runtime/ime/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export QT_IM_MODULE=fcitx
mkdir -p "$FCITX_CONFIG_HOME"
if [ ! -f "$FCITX_CONFIG_HOME/profile" ]; then
  cat > "$FCITX_CONFIG_HOME/profile" <<'EOF'
[Groups/0]
Name=Korean
Default Layout=us
DefaultIM=hangul

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=hangul
Layout=

[GroupOrder]
0=Korean
EOF
fi
if [ ! -f "$FCITX_CONFIG_HOME/config" ]; then
  cat > "$FCITX_CONFIG_HOME/config" <<'EOF'
[Hotkey/TriggerKeys]
0=Control+space
1=Hangul
2=Alt_R

[Behavior]
ActiveByDefault=True
EOF
fi
"$runtime/ime/usr/bin/fcitx5" -D --disable=all --enable=keyboard,hangul,dbus,dbusfrontend &
ime_pid=$!
cleanup() {
  "$runtime/ime/usr/bin/dbus-send" --session --print-reply --dest=org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1.Exit >/dev/null 2>&1 || kill "$ime_pid" 2>/dev/null || :
  wait "$ime_pid" 2>/dev/null || :
}
trap cleanup EXIT
trap 'exit 143' TERM INT
# Wait for the input service, rather than racing the first focused text field.
ready=false
for attempt in 1 2 3 4 5 6 7 8 9 10; do
  if "$runtime/ime/usr/bin/dbus-send" --session --print-reply --dest=org.fcitx.Fcitx5 /controller org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then
    ready=true
    break
  fi
  sleep 0.1
done
if [ "$ready" != true ]; then
  echo 'Korean input service failed to start' >&2
  exit 1
fi
# Qt's own input plugin uses the KDE runtime libraries, not the daemon's Ubuntu ABI.
env -u LD_LIBRARY_PATH "$@"
