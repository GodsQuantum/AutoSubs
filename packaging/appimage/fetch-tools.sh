#!/bin/sh
set -eu

verify_sha256() {
  file=$1
  expected=$2
  actual=$(sha256sum "$file" | awk '{print $1}')
  if [ "$actual" != "$expected" ]; then
    printf 'SHA-256 mismatch for %s\nexpected: %s\nactual:   %s\n' "$file" "$expected" "$actual" >&2
    return 1
  fi
}

if [ "${1:-}" = "--verify" ]; then
  [ "$#" -eq 3 ] || {
    printf 'usage: %s --verify FILE SHA256\n' "$0" >&2
    exit 64
  }
  verify_sha256 "$2" "$3"
  exit
fi

arch="${1:-$(uname -m)}"
case "$arch" in
  x86_64|amd64)
    arch=x86_64
    linuxdeploy_sha=8aea8da0f7f7039d2a2cecb14657d752a222a5e1d3825caeef186c82f751cdd1
    appimagetool_sha=95cbe7cce9717fce90c484e34052ee7c7f1d7635b33c12525b4776826a7d29b6
    runtime_sha=156f4bdbde9c52d01814600013e0a273f0118dc2de98975f3c8c63427ec79074
    ;;
  aarch64|arm64)
    arch=aarch64
    linuxdeploy_sha=5c1fddf96066891e829831cac0d84424690f3b22846c7f8f1bb9990a5c6c73f4
    appimagetool_sha=a595ea34cd6136c7f595e9dcbb16f3e9725d7610efb9e43b38c3c6e86cafc270
    runtime_sha=b4ff0030242d0c3bb12ce40541828303cf167493f4793456f0436edd6255c39d
    ;;
  *)
    printf 'unsupported AppImage architecture: %s\n' "$arch" >&2
    exit 64
    ;;
esac

if [ "$#" -ge 2 ]; then
  cache_dir=$2
else
  cache_root="${XDG_CACHE_HOME:-${HOME:?HOME is required when XDG_CACHE_HOME is unset}/.cache}"
  cache_dir="$cache_root/autosubs/appimage-tools/$arch"
fi
mkdir -p -- "$cache_dir"

fetch_one() {
  name=$1
  url=$2
  expected=$3
  dest="$cache_dir/$name"

  if [ -f "$dest" ]; then
    if verify_sha256 "$dest" "$expected" >/dev/null 2>&1; then
      chmod +x "$dest"
      return 0
    fi
    rm -f -- "$dest"
  fi

  tmp="$dest.partial.$$"
  rm -f -- "$tmp"
  trap 'rm -f -- "$tmp"' EXIT HUP INT TERM
  curl --fail --location --retry 3 --retry-delay 1 \
    --proto '=https' --tlsv1.2 \
    "$url" -o "$tmp"
  verify_sha256 "$tmp" "$expected"
  chmod +x "$tmp"
  mv -f -- "$tmp" "$dest"
  trap - EXIT HUP INT TERM
}

fetch_one "linuxdeploy-$arch.AppImage" \
  "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage" \
  "$linuxdeploy_sha"

fetch_one "appimagetool-$arch.AppImage" \
  "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage" \
  "$appimagetool_sha"

fetch_one "runtime-$arch" \
  "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-$arch" \
  "$runtime_sha"

printf '%s\n' "$cache_dir/linuxdeploy-$arch.AppImage"
printf '%s\n' "$cache_dir/appimagetool-$arch.AppImage"
printf '%s\n' "$cache_dir/runtime-$arch"
