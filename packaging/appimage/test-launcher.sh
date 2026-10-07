#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
APPRUN="$ROOT/packaging/appimage/AppRun"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1" needle="$2"
  grep -F -- "$needle" "$file" >/dev/null || fail "$file does not contain: $needle"
}

assert_not_exists() {
  [[ ! -e "$1" ]] || fail "unexpected path exists: $1"
}

[[ -x "$APPRUN" ]] || fail "AppRun missing or not executable: $APPRUN"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

make_fixture() {
  local dir="$1"
  mkdir -p "$dir/usr/bin" "$dir/usr/share/autosubs/frontend"

  cat > "$dir/usr/bin/autosubs" <<'EOF'
#!/bin/sh
{
  printf 'CONFIG=%s\n' "${AUTOSUBS_CONFIG_DIR-}"
  printf 'DATA=%s\n' "${AUTOSUBS_DATA_DIR-}"
  printf 'FONTS=%s\n' "${AUTOSUBS_FONTS_DIR-}"
  printf 'DIST=%s\n' "${AUTOSUBS_DIST_DIR-}"
  printf 'ALLOWED=%s\n' "${AUTOSUBS_ALLOWED_ROOTS-}"
  printf 'HOST=%s\n' "${AUTOSUBS_HOST-}"
  printf 'PORT=%s\n' "${AUTOSUBS_PORT-}"
  printf 'STATE=%s\n' "${AUTOSUBS_APPIMAGE_STATE_DIR-}"
  printf 'PATH=%s\n' "${PATH-}"
  printf 'ARGS='
  printf '%s|' "$@"
  printf '\n'
} > "${AUTOSUBS_TEST_CAPTURE}"
if [ -n "${AUTOSUBS_TEST_SERVER_MARKER-}" ]; then
  : > "$AUTOSUBS_TEST_SERVER_MARKER"
fi
sleep "${AUTOSUBS_TEST_SLEEP:-0}"
EOF
  chmod +x "$dir/usr/bin/autosubs"

  cat > "$dir/usr/bin/curl" <<'EOF'
#!/bin/sh
if [ "${AUTOSUBS_TEST_FOREIGN:-0}" = "1" ]; then
  printf '{"status":"ok","service":"something-else"}\n'
  exit 0
fi
if [ "${AUTOSUBS_TEST_EXISTING:-0}" = "1" ] || { [ -n "${AUTOSUBS_TEST_SERVER_MARKER-}" ] && [ -e "$AUTOSUBS_TEST_SERVER_MARKER" ]; }; then
  printf '{"status":"ok","version":"test","ffmpegReady":true,"libass":true}\n'
  exit 0
fi
exit 22
EOF
  chmod +x "$dir/usr/bin/curl"

  cat > "$dir/usr/bin/xdg-open" <<'EOF'
#!/bin/sh
printf '%s\n' "$1" >> "${AUTOSUBS_TEST_OPEN_CAPTURE}"
EOF
  chmod +x "$dir/usr/bin/xdg-open"

  for name in ffmpeg ffprobe fc-list fc-scan; do
    cat > "$dir/usr/bin/$name" <<'EOF'
#!/bin/sh
exit 0
EOF
    chmod +x "$dir/usr/bin/$name"
  done
}

run_clean() {
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_STATE_HOME \
      -u AUTOSUBS_CONFIG_DIR -u AUTOSUBS_DATA_DIR -u AUTOSUBS_FONTS_DIR \
      -u AUTOSUBS_DIST_DIR -u AUTOSUBS_ALLOWED_ROOTS -u AUTOSUBS_HOST \
      -u AUTOSUBS_PORT -u AUTOSUBS_USE_SYSTEM_MEDIA_TOOLS \
      "$@"
}

APPDIR1="$TMP/app dir"
HOME1="$TMP/Home With Space é"
mkdir -p "$HOME1"
make_fixture "$APPDIR1"
CAP1="$TMP/defaults.capture"
OPEN1="$TMP/defaults.open"
run_clean env APPDIR="$APPDIR1" HOME="$HOME1" USER="tester" \
  AUTOSUBS_TEST_CAPTURE="$CAP1" AUTOSUBS_TEST_OPEN_CAPTURE="$OPEN1" \
  AUTOSUBS_APPIMAGE_FORCE_STDIO=1 "$APPRUN" --no-browser --max-render-jobs 3

assert_contains "$CAP1" "CONFIG=$HOME1/.config/autosubs"
assert_contains "$CAP1" "DATA=$HOME1/.local/share/autosubs"
assert_contains "$CAP1" "FONTS=$HOME1/.local/share/autosubs/fonts"
assert_contains "$CAP1" "STATE=$HOME1/.local/state/autosubs"
assert_contains "$CAP1" "DIST=$APPDIR1/usr/share/autosubs/frontend"
assert_contains "$CAP1" "HOST=127.0.0.1"
assert_contains "$CAP1" "PORT=3051"
assert_contains "$CAP1" "ALLOWED=$HOME1/.local/share/autosubs:$HOME1"
assert_contains "$CAP1" "PATH=$APPDIR1/usr/bin:"
assert_contains "$CAP1" "ARGS=--max-render-jobs|3|"
assert_not_exists "$OPEN1"

CAP2="$TMP/existing.capture"
OPEN2="$TMP/existing.open"
run_clean env APPDIR="$APPDIR1" HOME="$HOME1" USER="tester" \
  AUTOSUBS_TEST_CAPTURE="$CAP2" AUTOSUBS_TEST_OPEN_CAPTURE="$OPEN2" \
  AUTOSUBS_TEST_EXISTING=1 AUTOSUBS_APPIMAGE_FORCE_STDIO=1 "$APPRUN"
assert_not_exists "$CAP2"
assert_contains "$OPEN2" "http://127.0.0.1:3051/"

CAP_VERSION="$TMP/version.capture"
run_clean env APPDIR="$APPDIR1" HOME="$HOME1" USER="tester" \
  AUTOSUBS_TEST_CAPTURE="$CAP_VERSION" AUTOSUBS_TEST_OPEN_CAPTURE="$TMP/version.open" \
  AUTOSUBS_TEST_EXISTING=1 AUTOSUBS_APPIMAGE_FORCE_STDIO=1 "$APPRUN" --version
assert_contains "$CAP_VERSION" "ARGS=--version|"

CAP_FOREIGN="$TMP/foreign.capture"
run_clean env APPDIR="$APPDIR1" HOME="$HOME1" USER="tester" \
  AUTOSUBS_TEST_CAPTURE="$CAP_FOREIGN" AUTOSUBS_TEST_OPEN_CAPTURE="$TMP/foreign.open" \
  AUTOSUBS_TEST_FOREIGN=1 AUTOSUBS_APPIMAGE_FORCE_STDIO=1 "$APPRUN" --no-browser
assert_contains "$CAP_FOREIGN" "HOST=127.0.0.1"

CAP3="$TMP/background.capture"
OPEN3="$TMP/background.open"
MARK3="$TMP/background.ready"
run_clean env APPDIR="$APPDIR1" HOME="$HOME1" USER="tester" \
  AUTOSUBS_TEST_CAPTURE="$CAP3" AUTOSUBS_TEST_OPEN_CAPTURE="$OPEN3" \
  AUTOSUBS_TEST_SERVER_MARKER="$MARK3" AUTOSUBS_TEST_SLEEP=0.3 \
  AUTOSUBS_APPIMAGE_FORCE_STDIO=1 "$APPRUN" --background
assert_contains "$CAP3" "HOST=127.0.0.1"
assert_not_exists "$OPEN3"

SYSBIN="$TMP/system-bin"
mkdir -p "$SYSBIN"
for name in ffmpeg ffprobe fc-list fc-scan curl xdg-open; do
  cat > "$SYSBIN/$name" <<'EOF'
#!/bin/sh
exit 22
EOF
  chmod +x "$SYSBIN/$name"
done
CAP4="$TMP/system.capture"
run_clean env APPDIR="$APPDIR1" HOME="$HOME1" USER="tester" \
  PATH="$SYSBIN:/usr/bin:/bin" AUTOSUBS_USE_SYSTEM_MEDIA_TOOLS=1 \
  AUTOSUBS_TEST_CAPTURE="$CAP4" AUTOSUBS_APPIMAGE_FORCE_STDIO=1 \
  "$APPRUN" --no-browser
assert_contains "$CAP4" "PATH=$SYSBIN:/usr/bin:/bin:$APPDIR1/usr/bin"

if env -u HOME APPDIR="$APPDIR1" AUTOSUBS_APPIMAGE_FORCE_STDIO=1 "$APPRUN" --no-browser >/dev/null 2>&1; then
  fail "AppRun unexpectedly accepted a missing HOME"
fi

printf 'PASS: AppImage launcher contract\n'
