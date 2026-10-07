#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
FETCH="$ROOT/packaging/appimage/fetch-tools.sh"
BUILD="$ROOT/packaging/appimage/build.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -x "$FETCH" ]] || fail "fetch-tools.sh missing or not executable"
[[ -x "$BUILD" ]] || fail "build.sh missing or not executable"

grep -F "8aea8da0f7f7039d2a2cecb14657d752a222a5e1d3825caeef186c82f751cdd1" "$FETCH" >/dev/null
grep -F "5c1fddf96066891e829831cac0d84424690f3b22846c7f8f1bb9990a5c6c73f4" "$FETCH" >/dev/null
grep -F "95cbe7cce9717fce90c484e34052ee7c7f1d7635b33c12525b4776826a7d29b6" "$FETCH" >/dev/null
grep -F "a595ea34cd6136c7f595e9dcbb16f3e9725d7610efb9e43b38c3c6e86cafc270" "$FETCH" >/dev/null

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

printf 'tool payload\n' > "$TMP/tool"
GOOD_SHA="$(sha256sum "$TMP/tool" | awk '{print $1}')"
"$FETCH" --verify "$TMP/tool" "$GOOD_SHA"
if "$FETCH" --verify "$TMP/tool" "$(printf '0%.0s' {1..64})" >/dev/null 2>&1; then
  fail "checksum verifier accepted an incorrect digest"
fi

mkdir -p "$TMP/frontend" "$TMP/runtime" "$TMP/bin"
printf '<!doctype html><title>AutoSubs package test</title>\n' > "$TMP/frontend/index.html"

cat > "$TMP/autosubs" <<'EOF'
#!/bin/sh
printf 'autosubs fake\n'
EOF
chmod +x "$TMP/autosubs"

for tool in ffmpeg ffprobe fc-list fc-scan curl; do
  cat > "$TMP/runtime/$tool" <<EOF
#!/bin/sh
printf '%s fake\n' "$tool"
EOF
  chmod +x "$TMP/runtime/$tool"
done

cat > "$TMP/bin/linuxdeploy" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$TMP/bin/linuxdeploy"

cat > "$TMP/bin/appimagetool" <<'EOF'
#!/bin/sh
last=
for arg in "$@"; do last="$arg"; done
printf 'fake appimage\n' > "$last"
chmod +x "$last"
EOF
chmod +x "$TMP/bin/appimagetool"

run_build() {
  local requested_arch="$1" expected_arch="$2" slot="$3"
  local build_dir="$TMP/build-$slot"
  local out_dir="$TMP/out-$slot"
  AUTOSUBS_BINARY="$TMP/autosubs" \
  AUTOSUBS_FRONTEND_DIR="$TMP/frontend" \
  AUTOSUBS_FFMPEG="$TMP/runtime/ffmpeg" \
  AUTOSUBS_FFPROBE="$TMP/runtime/ffprobe" \
  AUTOSUBS_FC_LIST="$TMP/runtime/fc-list" \
  AUTOSUBS_FC_SCAN="$TMP/runtime/fc-scan" \
  AUTOSUBS_CURL="$TMP/runtime/curl" \
  AUTOSUBS_LINUXDEPLOY="$TMP/bin/linuxdeploy" \
  AUTOSUBS_APPIMAGETOOL="$TMP/bin/appimagetool" \
  AUTOSUBS_APPIMAGE_BUILD_DIR="$build_dir" \
  AUTOSUBS_APPIMAGE_OUTPUT_DIR="$out_dir" \
  AUTOSUBS_APPIMAGE_KEEP_APPDIR=1 \
  "$BUILD" "$requested_arch"

  local version
  version="$(python3 -c 'import tomllib; print(tomllib.load(open("'"$ROOT"'/Cargo.toml","rb"))["package"]["version"])')"
  local image="$out_dir/AutoSubs-$version-$expected_arch.AppImage"
  [[ -x "$image" ]] || fail "missing executable artifact: $image"
  [[ -s "$image.sha256" ]] || fail "missing checksum: $image.sha256"
  (cd "$out_dir" && sha256sum -c "$(basename "$image").sha256")

  local appdir="$build_dir/AutoSubs.AppDir"
  [[ -x "$appdir/AppRun" ]] || fail "AppRun missing"
  [[ -x "$appdir/usr/bin/autosubs" ]] || fail "backend missing"
  [[ -x "$appdir/usr/bin/ffmpeg" ]] || fail "ffmpeg missing"
  [[ -x "$appdir/usr/bin/ffprobe" ]] || fail "ffprobe missing"
  [[ -x "$appdir/usr/bin/fc-list" ]] || fail "fc-list missing"
  [[ -x "$appdir/usr/bin/fc-scan" ]] || fail "fc-scan missing"
  [[ -x "$appdir/usr/bin/curl" ]] || fail "curl missing"
  [[ -f "$appdir/usr/share/autosubs/frontend/index.html" ]] || fail "frontend missing"
  [[ -f "$appdir/usr/share/metainfo/io.github.GodsQuantum.AutoSubs.metainfo.xml" ]] || fail "metainfo missing"
  [[ -L "$appdir/io.github.GodsQuantum.AutoSubs.desktop" ]] || fail "root desktop symlink missing"
  [[ -L "$appdir/autosubs.svg" ]] || fail "root icon symlink missing"
  [[ -L "$appdir/.DirIcon" ]] || fail ".DirIcon missing"
  [[ "$(find "$appdir" -maxdepth 1 -type f -name '*.desktop' -o -type l -name '*.desktop' | wc -l)" -eq 1 ]] || fail "AppDir must expose exactly one root desktop entry"
  if grep -R "@VERSION@\|@ARCH@" "$appdir/usr/share/applications/io.github.GodsQuantum.AutoSubs.desktop"; then
    fail "desktop metadata still contains build placeholders"
  fi
  grep -F "X-AppImage-Arch=$expected_arch" "$appdir/usr/share/applications/io.github.GodsQuantum.AutoSubs.desktop" >/dev/null
}

run_build amd64 x86_64 x86
run_build arm64 aarch64 arm

printf 'PASS: AppImage packaging contract\n'
