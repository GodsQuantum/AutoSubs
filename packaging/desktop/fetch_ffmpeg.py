#!/usr/bin/env python3
"""Download locked FFmpeg/FFprobe bundle for native OS; do not use latest URLs."""
from __future__ import annotations
import argparse
import hashlib
import os
import shutil
import tarfile
import urllib.request
import zipfile
from pathlib import Path

# Each OS has a separate upstream archive verified with its release digest.
# These builds must expose FFmpeg's libass/subtitles filter.
VERSION = "pinned-2026-10-18"
URLS = {
    "windows-amd64": "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-n8.1-latest-win64-gpl-8.1.zip",
    "darwin-amd64": "https://github.com/vanloctech/ffmpeg-macos/releases/download/ffmpeg-2026.06.11/ffmpeg-macos-x64.tar.gz",
    "darwin-arm64": "https://github.com/vanloctech/ffmpeg-macos/releases/download/ffmpeg-2026.06.11/ffmpeg-macos-arm64.tar.gz",
}
LOCK = {
    "windows-amd64": ("ffmpeg-n8.1-latest-win64-gpl-8.1.zip", "aa5132c64f081d2cb90ece95e6aac15a365a5dab86a4e2e042b84b68f3ce20f8"),
    "darwin-amd64": ("ffmpeg-macos-x64.tar.gz", "8317f17790749fd3502384201fd4d9d2a998ff9b8452d3b8b54b3b9af6a16e39"),
    "darwin-arm64": ("ffmpeg-macos-arm64.tar.gz", "26f5133f88d5ab1254cc03499acef42dae2dbc574dd26e0dade8c968420127e9"),
}


def download(variant: str, out: Path) -> None:
    filename, digest = LOCK[variant]
    out.mkdir(parents=True, exist_ok=True)
    archive = out / filename
    if not archive.is_file():
        request = urllib.request.Request(URLS[variant], headers={"User-Agent": "Cloud9-Desktop-Installer/1.0"})
        with urllib.request.urlopen(request, timeout=120) as response, archive.open("wb") as target:
            shutil.copyfileobj(response, target)
    got = hashlib.file_digest(archive.open("rb"), "sha256").hexdigest()
    if got != digest:
        archive.unlink(missing_ok=True)
        raise ValueError(f"FFmpeg SHA256 mismatch for {filename}: expected {digest}, got {got}")

    executable_names = ("ffmpeg.exe", "ffprobe.exe") if variant.startswith("windows") else ("ffmpeg", "ffprobe")
    if filename.endswith(".zip"):
        with zipfile.ZipFile(archive) as contents:
            for name in executable_names:
                matches = [i for i in contents.infolist() if Path(i.filename).name == name and not i.is_dir()]
                if len(matches) != 1:
                    raise ValueError(f"{name} not exactly once in {filename}")
                with contents.open(matches[0]) as source, (out / name).open("wb") as target:
                    shutil.copyfileobj(source, target)
    else:
        with tarfile.open(archive, "r:gz") as contents:
            for name in executable_names:
                matches = [i for i in contents.getmembers() if Path(i.name).name == name and i.isfile()]
                if len(matches) != 1:
                    raise ValueError(f"{name} not exactly once in {filename}")
                with contents.extractfile(matches[0]) as source, (out / name).open("wb") as target:
                    assert source is not None
                    shutil.copyfileobj(source, target)
    for name in executable_names:
        (out / name).chmod(0o755)
    print(f"FFMPEG_FFPROBE_LOCKED_OK={VERSION}/{variant} {digest}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--variant", required=True, choices=list(LOCK))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    download(args.variant, args.output)
