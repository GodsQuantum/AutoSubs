#!/usr/bin/env python3
"""Certify FFmpeg/FFprobe with actual media, plus libass overlay for AutoSubs."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import tempfile
from typing import Any


def validate_pcm_probe(data: dict[str, Any]) -> None:
    streams = data.get("streams")
    if not isinstance(streams, list) or not any(
        isinstance(s, dict) and s.get("codec_type") == "audio" and s.get("sample_rate") == "16000"
        for s in streams
    ):
        raise ValueError("FFprobe did not measure 16 kHz PCM audio sample rate")
    container = data.get("format")
    if not isinstance(container, dict):
        raise ValueError("FFprobe output has no container duration")
    try:
        duration = float(container["duration"])
    except (KeyError, TypeError, ValueError) as error:
        raise ValueError("FFprobe duration not numeric") from error
    if not 0.6 < duration < 1.2:
        raise ValueError(f"FFprobe measured unexpected 0.8 s audio duration: {duration}")


def validate_subtitle_frame(rgb: bytes | bytearray) -> None:
    expected = 320 * 180 * 3
    if len(rgb) != expected or sum(pixel >= 120 for pixel in rgb) < 400:
        raise ValueError("Real libass subtitle burn-in is missing from the RGB video frame")


def run(*args: str | Path, cwd: Path, capture_binary: bool = False) -> bytes:
    command = [str(item) for item in args]
    result = subprocess.run(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=120)
    if result.returncode:
        raise RuntimeError(f"Native FFmpeg command failed ({result.returncode}): {result.stderr.decode(errors='replace')[-1100:]}")
    return result.stdout if capture_binary else b""


def verify_real_media(ffmpeg: Path, ffprobe: Path, *, needs_subtitles: bool) -> None:
    if not (ffmpeg.is_file() and ffprobe.is_file()):
        raise FileNotFoundError("Native FFmpeg and FFprobe must exist before media checks")
    with tempfile.TemporaryDirectory(prefix="trio-native-ffmpeg-real-media-") as scratch:
        working = Path(scratch)
        clip = working / "real-pcm16-16khz.wav"
        run(
            ffmpeg, "-hide_banner", "-loglevel", "error", "-nostdin",
            "-f", "lavfi", "-i", "sine=frequency=550:duration=0.8",
            "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", "-y", clip,
            cwd=working,
        )
        raw_probe = run(
            ffprobe, "-v", "error", "-show_entries", "stream=codec_type,sample_rate",
            "-show_entries", "format=duration", "-of", "json", clip,
            cwd=working, capture_binary=True,
        )
        validate_pcm_probe(json.loads(raw_probe))
        print("REAL_NATIVE_FFMPEG_FFPROBE_PCM16_PASS", flush=True)
        if needs_subtitles:
            caption = working / "caption.srt"
            caption.write_text(
                "1\n00:00:00,000 --> 00:00:01,000\nBonjour, en français.\n",
                encoding="utf-8",
            )
            rendered = run(
                ffmpeg, "-hide_banner", "-loglevel", "error", "-nostdin",
                "-f", "lavfi", "-i", "color=c=black:s=320x180:r=1:d=1",
                "-vf", "subtitles=caption.srt", "-frames:v", "1",
                "-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1",
                cwd=working, capture_binary=True,
            )
            validate_subtitle_frame(rendered)
            print("REAL_NATIVE_FFMPEG_LIBASS_SUBTITLE_BURNIN_PASS", flush=True)
