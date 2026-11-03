#!/bin/sh
set -eu

# Immutable tagged upstream releases, digest-pinned from official GitHub
# release assets. Never swap to /continuous without a provenance/release audit.
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
    linuxdeploy_sha=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
    appimagetool_sha=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
    runtime_sha=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
    ;;
  aarch64|arm64)
    arch=aarch64
    linuxdeploy_sha=620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff
    appimagetool_sha=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158
    runtime_sha=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444
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
  "https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-$arch.AppImage" \
  "$linuxdeploy_sha"

fetch_one "appimagetool-$arch.AppImage" \
  "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-$arch.AppImage" \
  "$appimagetool_sha"

fetch_one "runtime-$arch" \
  "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-$arch" \
  "$runtime_sha"

printf '%s\n' "$cache_dir/linuxdeploy-$arch.AppImage"
printf '%s\n' "$cache_dir/appimagetool-$arch.AppImage"
printf '%s\n' "$cache_dir/runtime-$arch"
