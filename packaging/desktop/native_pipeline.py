#!/usr/bin/env python3
"""Native CI: assemble FFmpeg-inclusive installer; install/mount and test it."""
from __future__ import annotations
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
APP=tomllib.loads((ROOT/"Cargo.toml").read_text())["package"]["name"]
APPNAME="AutoSubs" if APP=="autosubs" else "ScribeWatch"
VERSION=tomllib.loads((ROOT/"Cargo.toml").read_text())["package"]["version"]
WINDOWS=sys.platform=="win32"
OS="windows" if WINDOWS else "macos"
EXE=".exe" if WINDOWS else ""
SERVER=ROOT/"target"/"release"/(APP+EXE)
LAUNCHER=ROOT/"packaging"/"desktop"/"launcher"/"target"/"release"/(APPNAME+EXE)
DIST=ROOT/"dist"/"desktop"
if sys.platform=="darwin":
    VARIANT="darwin-"+("arm64" if platform.machine().lower() in ("arm64","aarch64") else "amd64")
elif WINDOWS:
    assert platform.machine().lower() in ("amd64","x86_64")
    VARIANT="windows-amd64"
else:
    raise SystemExit("Desktop CI targets macOS and Windows only")
PORT="39061" if APP=="autosubs" else "39062"


def run(*args: str | Path, env: dict[str,str] | None=None) -> None:
    print("RUN",*(str(a) for a in args),flush=True)
    subprocess.run([str(x) for x in args],env=env,check=True,timeout=900)


def main() -> None:
    print("NATIVE_INSTALLER_BUILD",APPNAME,VERSION,platform.platform(),VARIANT,flush=True)
    FF=Path(tempfile.mkdtemp(prefix="trio-ffmpeg-"))
    stage=Path(tempfile.mkdtemp(prefix="trio-stage-parent-"))/APPNAME
    try:
        run(sys.executable,ROOT/"packaging"/"desktop"/"fetch_ffmpeg.py",
            "--variant",VARIANT,"--output",FF)
        ffmpeg=FF/("ffmpeg"+EXE)
        ffprobe=FF/("ffprobe"+EXE)
        run(ffmpeg,"-version")
        run(ffprobe,"-version")
        filters=subprocess.check_output([str(ffmpeg),"-hide_banner","-filters"],text=True,timeout=30)
        if APP=="autosubs":
            assert "subtitles" in filters and "ass" in filters, "FFmpeg libass/subtitles is mandatory for AutoSubs"
        run(sys.executable,ROOT/"packaging"/"desktop"/"stage.py",
            "--repo",ROOT,"--platform",OS,"--server",SERVER,
            "--launcher",LAUNCHER,"--ffmpeg",FF,"--output",stage)
        DIST.mkdir(parents=True,exist_ok=True)
        env=os.environ.copy()
        env[APP.upper()+"_PORT"]=PORT
        env["NO_PROXY"]="localhost,127.0.0.1"
        if WINDOWS:
            run("choco","install","nsis","--yes","--no-progress")
            candidates=[shutil.which("makensis"),
                r"C:\Program Files (x86)\NSIS\makensis.exe",
                r"C:\Program Files\NSIS\makensis.exe"]
            installer=next((x for x in candidates if x and Path(x).exists()),None)
            assert installer, "NSIS makensis unavailable"
            output=DIST/(APPNAME+"-"+VERSION+"-windows-amd64-setup.exe")
            run(installer,"/V2","/DAPP_NAME="+APPNAME,"/DAPP_VERSION="+VERSION,
                "/DSTAGING="+str(stage),"/DOUTPUT="+str(output),
                ROOT/"packaging"/"desktop"/"installer.nsi")
            assert output.is_file()
            installed=Path(tempfile.gettempdir())/("trio-installed-"+APPNAME)
            assert not installed.exists(), "Installer test directory already exists"
            run(output,"/S","/D="+str(installed))
            binary=installed/(APPNAME+".exe")
            assert binary.is_file(), "NSIS did not install launcher"
            run(binary,"--check-install",env=env)
            run(binary,"--smoke",env=env)
            run(installed/"Uninstall.exe","/S")
            # NSIS invokes an uninstaller copy from %TEMP%. The outer process
            # can return before the actual removal process finishes.
            for _ in range(120):
                if not binary.exists():
                    break
                time.sleep(0.25)
            assert not binary.exists(), "NSIS uninstall did not remove launcher after 30s"
            print("WINDOWS_NSIS_INSTALL_UNINSTALL_AND_NATIVE_APP_SMOKE_OK",APPNAME)
        else:
            bundle=stage/(APPNAME+".app")
            # Ad hoc signing is only for testing; it is not Apple notarization.
            for file in (bundle/"Contents"/"Resources"/"bin").iterdir():
                if file.is_file():
                    run("codesign","--force","--sign","-","--timestamp=none",file)
            run("codesign","--force","--sign","-","--timestamp=none",bundle)
            run("codesign","--verify","--deep","--strict",bundle)
            output=DIST/(APPNAME+"-"+VERSION+"-"+VARIANT+".dmg")
            run("hdiutil","create","-volname",APPNAME,"-format","UDZO","-srcfolder",stage,output)
            mount=Path(tempfile.mkdtemp(prefix="trio-dmg-mount-"))
            try:
                run("hdiutil","attach","-quiet","-readonly","-nobrowse",
                    "-mountpoint",mount,output)
                binary=mount/(APPNAME+".app")/"Contents"/"MacOS"/APPNAME
                run(binary,"--check-install",env=env)
                try:
                    run(binary,"--smoke",env=env)
                except subprocess.CalledProcessError:
                    server_log=Path.home()/"Library"/"Application Support"/APPNAME/"desktop.log"
                    if server_log.is_file():
                        print("NATIVE_STARTUP_LOG_TAIL:\n"+server_log.read_text(errors="replace")[-5000:],flush=True)
                    raise
            finally:
                run("hdiutil","detach","-quiet",mount)
                mount.rmdir()
            print("MACOS_DMG_MOUNT_AND_NATIVE_APP_SMOKE_OK",APPNAME)
        import hashlib
        with output.open("rb") as source:
            digest=hashlib.file_digest(source,"sha256").hexdigest()
        (DIST/(output.name+".sha256")).write_text(digest+"  "+output.name+"\n")
        print("NATIVE_INSTALLER_CANDIDATE_OK",output,digest,flush=True)
        print("NOTARIZATION_AND_AUTHENTICODE_NOT_CERTIFIED_WITHOUT_REAL_CREDENTIALS")
    finally:
        shutil.rmtree(FF,ignore_errors=True)
        shutil.rmtree(stage.parent,ignore_errors=True)


if __name__=="__main__":
    main()
