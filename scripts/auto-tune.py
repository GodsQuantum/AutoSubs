#!/usr/bin/env python3
"""Print conservative hardware-aware concurrency recommendations without editing settings."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path


def effective_cpus() -> int:
    affinity = getattr(os, "sched_getaffinity", None)
    count = len(affinity(0)) if affinity else (os.cpu_count() or 2)
    quota = Path("/sys/fs/cgroup/cpu.max")
    if quota.is_file():
        fields = quota.read_text().split()
        if len(fields) >= 2 and fields[0].isdigit() and fields[1].isdigit():
            count = min(count, max(1, int(fields[0]) // max(int(fields[1]), 1)))
    return max(1, count)


def available_memory_gib() -> float:
    limits = []
    for path in (Path("/sys/fs/cgroup/memory.max"), Path("/sys/fs/cgroup/memory/memory.limit_in_bytes")):
        if path.is_file():
            val = path.read_text().strip()
            if val.isdigit() and 0 < int(val) < 2**60:
                limits.append(int(val))
    try:
        pages = os.sysconf("SC_PHYS_PAGES")
        page_size = os.sysconf("SC_PAGE_SIZE")
        limits.append(pages * page_size)
    except (AttributeError, ValueError, OSError):
        pass
    return min(limits) / 2**30 if limits else 4.0


def recommend(app: str, cpus: int, memory_gib: float) -> dict[str, object]:
    # Deliberate ceiling: encoding and transcription share the same host on small machines.
    workers = 1 if cpus < 8 or memory_gib < 8 else 2
    if app == "autosubs":
        settings = {
            "AUTOSUBS_MAX_RENDER_JOBS": str(workers),
            "AUTOSUBS_MAX_TRANSCRIPTION_JOBS": str(1 if cpus < 12 else 2),
            "AUTOSUBS_MAX_QUEUED_JOBS": "256",
        }
    elif app == "scribewatch":
        settings = {
            "SCRIBEWATCH_MAX_TRANSCRIPTION_JOBS": str(workers),
            "SCRIBEWATCH_FFMPEG_THREADS": "1",
            "SCRIBEWATCH_SCAN_SECONDS": "5",
            "SCRIBEWATCH_FILE_STABILITY_MS": "2000",
        }
    else:
        raise ValueError("unknown application")
    return {
        "application": app,
        "effective_cpu_threads": cpus,
        "memory_limit_gib": round(memory_gib, 1),
        "recommended_environment": settings,
        "notes": [
            "These settings are conservative starting points, not benchmark-proven maxima.",
            "On a shared host, benchmark with a real file before raising job concurrency.",
            "For speech GPU acceleration, pair with an independently verified Speaches backend.",
            "Existing app UI/database preferences may override first-boot environment variables.",
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--env", action="store_true", help="Print Compose .env-ready settings")
    args = parser.parse_args()
    app = "AUTOSUBS" if "autosubs" in str(Path(__file__).resolve()).lower() else "SCRIBEWATCH"
    profile = recommend(app.lower(), effective_cpus(), available_memory_gib())
    if args.env:
        for key, val in profile["recommended_environment"].items():
            print(f"{key}={val}")
    else:
        print(json.dumps(profile, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
