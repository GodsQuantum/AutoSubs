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

grep -F "c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d" "$FETCH" >/dev/null
grep -F "620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff" "$FETCH" >/dev/null
grep -F "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0" "$FETCH" >/dev/null
grep -F "f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158" "$FETCH" >/dev/null
grep -F "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d" "$FETCH" >/dev/null
grep -F "00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444" "$FETCH" >/dev/null
for release in 1-alpha-20251107-1 1.9.1 20251108; do
  grep -F "/releases/download/$release/" "$FETCH" >/dev/null
done
if grep -F "/releases/download/continuous/" "$FETCH"; then
  fail "mutable continuous AppImage download URL"
fi

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
printf '%s\n' '-----BEGIN CERTIFICATE-----' 'fixture' '-----END CERTIFICATE-----' > "$TMP/runtime/ca-certificates.crt"
printf 'fake jack\n' > "$TMP/runtime/libjack.so.0"
printf 'fake runtime\n' > "$TMP/runtime/runtime-x86_64"
printf 'fake runtime\n' > "$TMP/runtime/runtime-aarch64"

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
printf '%s\n' "$@" > "${AUTOSUBS_TEST_LINUXDEPLOY_ARGS:?}"
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
  AUTOSUBS_CA_CERT_FILE="$TMP/runtime/ca-certificates.crt" \
  AUTOSUBS_LIBJACK="$TMP/runtime/libjack.so.0" \
  AUTOSUBS_APPIMAGE_RUNTIME="$TMP/runtime/runtime-$expected_arch" \
  AUTOSUBS_TEST_LINUXDEPLOY_ARGS="$TMP/linuxdeploy-$slot.args" \
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
  [[ -f "$appdir/usr/share/autosubs/ca-certificates.crt" ]] || fail "CA bundle missing"
  [[ -f "$appdir/usr/share/metainfo/io.github.GodsQuantum.AutoSubs.appdata.xml" ]] || fail "AppStream metadata missing"
  [[ -f "$appdir/usr/lib/libjack.so.0" ]] || fail "libjack runtime compatibility library missing"
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
