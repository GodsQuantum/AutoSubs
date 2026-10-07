#!/bin/sh
set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd -P)
FETCH="$ROOT/packaging/appimage/fetch-tools.sh"

requested_arch="${1:-$(uname -m)}"
case "$requested_arch" in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *)
    printf 'unsupported AppImage architecture: %s\n' "$requested_arch" >&2
    exit 64
    ;;
esac

version=$(python3 -c 'import tomllib; print(tomllib.load(open("'"$ROOT"'/Cargo.toml","rb"))["package"]["version"])')
build_dir="${AUTOSUBS_APPIMAGE_BUILD_DIR:-${TMPDIR:-/tmp}/autosubs-appimage-$arch}"
out_dir="${AUTOSUBS_APPIMAGE_OUTPUT_DIR:-$ROOT/dist}"
appdir="$build_dir/AutoSubs.AppDir"

binary="${AUTOSUBS_BINARY:-$ROOT/target/release/autosubs}"
frontend="${AUTOSUBS_FRONTEND_DIR:-$ROOT/frontend/build}"

require_file() {
  [ -f "$1" ] || {
    printf 'required file is missing: %s\n' "$1" >&2
    exit 66
  }
}

require_exec() {
  [ -x "$1" ] || {
    printf 'required executable is missing: %s\n' "$1" >&2
    exit 66
  }
}

resolve_runtime_tool() {
  env_name=$1
  tool_name=$2
  eval "value=\${$env_name:-}"
  if [ -z "$value" ]; then
    value=$(command -v "$tool_name" 2>/dev/null || true)
  fi
  [ -n "$value" ] || {
    printf 'required packaging runtime tool is missing: %s\n' "$tool_name" >&2
    exit 69
  }
  printf '%s\n' "$value"
}

require_exec "$binary"
require_file "$frontend/index.html"

ffmpeg=$(resolve_runtime_tool AUTOSUBS_FFMPEG ffmpeg)
ffprobe=$(resolve_runtime_tool AUTOSUBS_FFPROBE ffprobe)
fc_list=$(resolve_runtime_tool AUTOSUBS_FC_LIST fc-list)
fc_scan=$(resolve_runtime_tool AUTOSUBS_FC_SCAN fc-scan)
curl_bin=$(resolve_runtime_tool AUTOSUBS_CURL curl)

if [ -n "${AUTOSUBS_LINUXDEPLOY:-}" ] && [ -n "${AUTOSUBS_APPIMAGETOOL:-}" ]; then
  linuxdeploy=$AUTOSUBS_LINUXDEPLOY
  appimagetool=$AUTOSUBS_APPIMAGETOOL
else
  cache_root="${XDG_CACHE_HOME:-${HOME:?HOME is required when XDG_CACHE_HOME is unset}/.cache}"
  tool_cache="$cache_root/autosubs/appimage-tools/$arch"
  "$FETCH" "$arch" "$tool_cache" >/dev/null
  linuxdeploy="$tool_cache/linuxdeploy-$arch.AppImage"
  appimagetool="$tool_cache/appimagetool-$arch.AppImage"
fi
require_exec "$linuxdeploy"
require_exec "$appimagetool"

rm -rf -- "$build_dir"
mkdir -p -- \
  "$appdir/usr/bin" \
  "$appdir/usr/share/autosubs/frontend" \
  "$appdir/usr/share/applications" \
  "$appdir/usr/share/icons/hicolor/scalable/apps" \
  "$appdir/usr/share/metainfo" \
  "$out_dir"

cp -L -- "$binary" "$appdir/usr/bin/autosubs"
chmod +x "$appdir/usr/bin/autosubs"

copy_runtime() {
  source_path=$1
  name=$2
  cp -L -- "$source_path" "$appdir/usr/bin/$name"
  chmod +x "$appdir/usr/bin/$name"
}
copy_runtime "$ffmpeg" ffmpeg
copy_runtime "$ffprobe" ffprobe
copy_runtime "$fc_list" fc-list
copy_runtime "$fc_scan" fc-scan
copy_runtime "$curl_bin" curl

APPIMAGE_EXTRACT_AND_RUN=1 "$linuxdeploy" \
  --appdir "$appdir" \
  --executable "$appdir/usr/bin/autosubs" \
  --executable "$appdir/usr/bin/ffmpeg" \
  --executable "$appdir/usr/bin/ffprobe" \
  --executable "$appdir/usr/bin/fc-list" \
  --executable "$appdir/usr/bin/fc-scan" \
  --executable "$appdir/usr/bin/curl"

cp -a -- "$frontend/." "$appdir/usr/share/autosubs/frontend/"
cp -- "$ROOT/packaging/appimage/AppRun" "$appdir/AppRun"
chmod +x "$appdir/AppRun"

sed \
  -e "s/@VERSION@/$version/g" \
  -e "s/@ARCH@/$arch/g" \
  "$ROOT/packaging/appimage/io.github.GodsQuantum.AutoSubs.desktop" \
  > "$appdir/usr/share/applications/io.github.GodsQuantum.AutoSubs.desktop"

cp -- "$ROOT/packaging/appimage/io.github.GodsQuantum.AutoSubs.metainfo.xml" \
  "$appdir/usr/share/metainfo/io.github.GodsQuantum.AutoSubs.metainfo.xml"
cp -- "$ROOT/docs/logo.svg" "$appdir/usr/share/icons/hicolor/scalable/apps/autosubs.svg"

ln -s "usr/share/applications/io.github.GodsQuantum.AutoSubs.desktop" \
  "$appdir/io.github.GodsQuantum.AutoSubs.desktop"
ln -s "usr/share/icons/hicolor/scalable/apps/autosubs.svg" "$appdir/autosubs.svg"
ln -s "autosubs.svg" "$appdir/.DirIcon"

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$appdir/usr/share/applications/io.github.GodsQuantum.AutoSubs.desktop"
fi
if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli validate --no-net "$appdir/usr/share/metainfo/io.github.GodsQuantum.AutoSubs.metainfo.xml"
fi

if [ -z "${SOURCE_DATE_EPOCH:-}" ] && command -v git >/dev/null 2>&1; then
  SOURCE_DATE_EPOCH=$(git -C "$ROOT" log -1 --format=%ct 2>/dev/null || true)
  export SOURCE_DATE_EPOCH
fi

output="$out_dir/AutoSubs-$version-$arch.AppImage"
rm -f -- "$output" "$output.sha256"
APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$arch" VERSION="$version" \
  "$appimagetool" "$appdir" "$output"
chmod +x "$output"
(
  cd "$out_dir"
  sha256sum "$(basename "$output")" > "$(basename "$output").sha256"
)

printf 'AppImage: %s\n' "$output"
printf 'Checksum: %s.sha256\n' "$output"

if [ "${AUTOSUBS_APPIMAGE_KEEP_APPDIR:-0}" != "1" ]; then
  rm -rf -- "$build_dir"
fi
