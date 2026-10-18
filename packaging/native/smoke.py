#!/usr/bin/env python3
"""Native localhost API portability smoke; not a media/GPU test."""
import argparse
import json
import os
import platform
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--binary", required=True)
parser.add_argument("--app", required=True, choices=("autosubs", "scribewatch"))
args = parser.parse_args()
binary = Path(args.binary).resolve(strict=True)
prefix = args.app.upper()
expected_version = "3.6.3" if args.app == "autosubs" else "0.5.3"
v = subprocess.run([str(binary), "--version"], check=True, capture_output=True, text=True, timeout=15)
assert expected_version in v.stdout, (v.stdout, expected_version)
with tempfile.TemporaryDirectory(prefix=f"cloud9-native-{args.app}-") as tmp:
    root = Path(tmp)
    config, data, fonts = (root / n for n in ("config", "data", "fonts"))
    for folder in (config, data, fonts):
        folder.mkdir()
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    env = os.environ.copy()
    env.update({
        f"{prefix}_HOST": "127.0.0.1",
        f"{prefix}_PORT": str(port),
        f"{prefix}_CONFIG_DIR": str(config),
        f"{prefix}_DATA_DIR": str(data),
        f"{prefix}_DIST_DIR": str(Path("frontend/build").absolute()),
        f"{prefix}_ALLOWED_ROOTS": str(data),
    })
    if args.app == "autosubs":
        env[f"{prefix}_FONTS_DIR"] = str(fonts)
    endpoint = "health" if args.app == "autosubs" else "ready"
    log_file = root / "server.log"
    with log_file.open("w", encoding="utf-8") as log:
        proc = subprocess.Popen([str(binary)], stdout=log, stderr=subprocess.STDOUT, env=env)
        try:
            status = None
            for _ in range(180):
                if proc.poll() is not None:
                    raise RuntimeError(f"{args.app} exited ({proc.returncode}): {log_file.read_text()[-2500:]}")
                try:
                    with urllib.request.urlopen(f"http://127.0.0.1:{port}/api/v1/{endpoint}", timeout=2) as resp:
                        status = json.load(resp)
                        if resp.status == 200:
                            break
                except (OSError, ValueError, urllib.error.URLError):
                    pass
                time.sleep(0.5)
            if not status or status.get("version") != expected_version:
                raise RuntimeError(f"Bad readiness {status!r}; {log_file.read_text()[-2500:]}")
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=10) as resp:
                html = resp.read(100000).decode("utf-8")
                assert resp.status == 200 and args.app.lower() in html.lower()
            print("NATIVE_OS_STARTUP_OK", args.app, platform.system(), platform.machine(), status)
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=10)
