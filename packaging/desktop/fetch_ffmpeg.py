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

VERSION = "v9.0.2"
BASE = f"https://github.com/binmgr/ffmpeg/releases/download/{VERSION}"
LOCK = {
    "windows-amd64": ("ffmpeg-windows-amd64.zip", "debe985a2d72005badb1ef88fb34d313801b114cb7de781aaba66bffcc37668e"),
    "darwin-amd64": ("ffmpeg-darwin-amd64.tar.gz", "615a1028961d45999256ef32c5342c1480260fb8d19077383d13e694283f6ab2"),
    "darwin-arm64": ("ffmpeg-darwin-arm64.tar.gz", "32bb9b7cc74d0581841bbafdd412ae7f5bc286be56f7a9aa6f9031868cfb26e6"),
}


def download(variant: str, out: Path) -> None:
    filename, digest = LOCK[variant]
    out.mkdir(parents=True, exist_ok=True)
    archive = out / filename
    if not archive.is_file():
        request = urllib.request.Request(f"{BASE}/{filename}", headers={"User-Agent": "Cloud9-Desktop-Installer/1.0"})
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
