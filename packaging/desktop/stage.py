#!/usr/bin/env python3
from __future__ import annotations
import argparse
import hashlib
import json
import plistlib
import shutil
import tomllib
from pathlib import Path

def digest(path: Path) -> str:
    with path.open("rb") as f:
        return hashlib.file_digest(f,"sha256").hexdigest()

def main() -> None:
    ap=argparse.ArgumentParser()
    for k in ("repo","server","launcher","ffmpeg","output"):
        ap.add_argument("--"+k,required=True,type=Path)
    ap.add_argument("--platform",required=True,choices=("windows","macos"))
    x=ap.parse_args()
    repo=x.repo.resolve(strict=True)
    manifest=tomllib.loads((repo/"Cargo.toml").read_text())
    package=manifest["package"]["name"]
    assert package in ("autosubs","scribewatch")
    app="AutoSubs" if package=="autosubs" else "ScribeWatch"
    version=manifest["package"]["version"]
    out=x.output.resolve()
    if out.exists():
        raise SystemExit("Refusing to overwrite existing staging tree "+str(out))
    win=x.platform=="windows"
    suffix=".exe" if win else ""
    resources=out if win else out/(app+".app")/"Contents"/"Resources"
    launcher_dest=out/(app+".exe") if win else out/(app+".app")/"Contents"/"MacOS"/app
    bin_dir=resources/"bin"
    bin_dir.mkdir(parents=True)
    launcher_dest.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(x.launcher.resolve(strict=True),launcher_dest)
    shutil.copy2(x.server.resolve(strict=True),bin_dir/(package+"-server"+suffix))
    for name in ("ffmpeg","ffprobe"):
        src=x.ffmpeg.resolve(strict=True)/(name+suffix)
        shutil.copy2(src,bin_dir/src.name)
    frontend=repo/"frontend"/"build"
    assert (frontend/"index.html").exists()
    shutil.copytree(frontend,resources/"frontend")
    if not win:
        plist={"CFBundleName":app,"CFBundleDisplayName":app,
               "CFBundleExecutable":app,"CFBundleIdentifier":"io.github.GodsQuantum."+app,
               "CFBundleShortVersionString":version,"CFBundleVersion":version,
               "CFBundlePackageType":"APPL","LSMinimumSystemVersion":"11.0",
               "NSHighResolutionCapable":True}
        with (resources.parent/"Info.plist").open("wb") as file:
            plistlib.dump(plist,file)
        for path in (launcher_dest,bin_dir/(package+"-server"),bin_dir/"ffmpeg",bin_dir/"ffprobe"):
            path.chmod(0o755)
    # Redistribute FFmpeg license terms and build/source provenance.
    import subprocess
    ffmpeg_license=subprocess.check_output([str(bin_dir/("ffmpeg"+suffix)),"-L"],
        text=True,errors="replace",timeout=30)
    license_file=resources/"THIRD-PARTY-FFMPEG.txt"
    license_file.write_text(
        "FFmpeg " + "9.0.2" + " — binary distributor: binmgr/ffmpeg v9.0.2\n"
        "Binary: https://github.com/binmgr/ffmpeg/releases/tag/v9.0.2\n"
        "Source: https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz\n"
        "License output from the packaged ffmpeg -L command:\n\n"
        + ffmpeg_license, encoding="utf-8")
    files=sorted(p for p in out.rglob("*") if p.is_file())
    sums={p.relative_to(out).as_posix():digest(p) for p in files}
    (out/"manifest-sha256.json").write_text(json.dumps({"app":app,"version":version,"os":x.platform,"files":sums},indent=2)+"\n")
    print("DESKTOP_STAGE_VERIFIED",app,version,x.platform,len(files))

if __name__=="__main__":
    main()
