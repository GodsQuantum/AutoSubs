"""Cloud9 long-form ASR: bounded, speech-aware chunks, safe overlap merging.

Use this for decoder architectures that have a shorter *usable* length than
their advertised input length. No synthetic per-word acoustic timestamps.
The same tested algorithm is shared with AutoSubs GPU worker.
"""
from __future__ import annotations

import math
import os
import re
import unicodedata
from dataclasses import dataclass
from types import SimpleNamespace
from typing import Any

import numpy as np


@dataclass(frozen=True)
class ChunkProfile:
    window_seconds: float
    overlap_seconds: float
    # Snap boundaries to measured quiet points inside this search radius.
    silence_search_seconds: float = 1.5


def profile_for_arch(arch: str, duration: float, *, force_all: bool = False) -> ChunkProfile | None:
    override = os.getenv("CLOUD9_LONGFORM_POLICY", "auto").strip().lower()
    if override not in {"auto", "all", "off"}:
        raise ValueError(f"Invalid CLOUD9_LONGFORM_POLICY {override}")
    if override == "off":
        return None
    # Profile values were tested on the same 105 s French reel. Limits in
    # transcribe.cpp may advertise 400 s but Cohere clips above ~50 s.
    profiles = {
        "cohere_asr": ChunkProfile(34.0, 1.0),
        "cohere": ChunkProfile(34.0, 1.0),
        "canary": ChunkProfile(30.0, 0.0),
        "canary_qwen": ChunkProfile(30.0, 0.0),
        "qwen3_asr": ChunkProfile(30.0, 0.0),
        "funasr_nano": ChunkProfile(30.0, 0.0),
        "voxtral": ChunkProfile(30.0, 0.0),
        "granite": ChunkProfile(30.0, 0.0),
    }
    if force_all or override == "all":
        profile = profiles.get(arch, ChunkProfile(30.0, 0.0))
    else:
        profile = profiles.get(arch)
    if profile is None or duration <= profile.window_seconds:
        return None
    return profile


def _quiet_boundary(samples: np.ndarray, sample_rate: int, desired: int, radius: int) -> int:
    """Find a nearby 100-ms low-RMS span; otherwise keep the original boundary."""
    start = max(0, desired - radius)
    end = min(len(samples), desired + radius)
    frame = max(1, round(sample_rate * 0.10))
    best_pos = desired
    best_energy = float("inf")
    # Restrict to 100 ms aligned windows; avoid picking a zero-crossing as silence.
    for pos in range(start, end - frame + 1, max(1, frame // 2)):
        data = np.asarray(samples[pos:pos + frame], dtype=np.float32)
        rms = float(np.sqrt(np.mean(data * data)))
        score = rms + abs(pos - desired) / sample_rate * 0.004
        if score < best_energy:
            best_energy = score
            best_pos = pos + frame // 2
    # Only move if a genuinely quiet portion is found.
    return best_pos if best_energy < 0.012 else desired


def chunk_windows(audio: np.ndarray, sample_rate: int, profile: ChunkProfile) -> list[tuple[int, int]]:
    if sample_rate <= 0 or len(audio) == 0:
        raise ValueError("Audio is empty or sample_rate invalid")
    width = round(profile.window_seconds * sample_rate)
    overlap = round(profile.overlap_seconds * sample_rate)
    radius = round(profile.silence_search_seconds * sample_rate)
    if width < sample_rate or overlap >= width // 3 or overlap < 0:
        raise ValueError("Invalid longform window/overlap")
    windows = []
    start = 0
    while start < len(audio):
        nominal_end = min(len(audio), start + width)
        if nominal_end < len(audio):
            end = _quiet_boundary(audio, sample_rate, nominal_end, radius)
            end = min(max(end, start + sample_rate), len(audio))
        else:
            end = len(audio)
        if end <= start:
            raise ValueError("Longform splitter made no progress")
        windows.append((start, end))
        if end == len(audio):
            break
        next_start = end - overlap
        if next_start <= start:
            raise ValueError("Longform overlap would make no progress")
        start = next_start
    # Invariant: cover complete input without silent gaps.
    if windows[0][0] != 0 or windows[-1][1] != len(audio):
        raise ValueError("Longform audio coverage incomplete")
    for left, right in zip(windows, windows[1:]):
        if right[0] > left[1] or right[0] <= left[0]:
            raise ValueError("Longform audio coverage/monotonicity invalid")
    return windows


def _key(word: str) -> str:
    v = unicodedata.normalize("NFKD", word.casefold())
    return "".join(c for c in v if c.isalnum() and not unicodedata.combining(c))


def overlap_drop(previous: str, current: str, max_tokens: int = 22) -> int:
    """Only drop an overlap backed by >=2 exact normalized tokens near a seam."""
    prev = previous.split()
    following = current.split()
    if not prev or not following:
        return 0
    tail = [_key(x) for x in prev[-max_tokens:]]
    head = [_key(x) for x in following[:max_tokens]]
    candidates: list[tuple[int, int]] = []
    for i in range(max(0, len(tail) - 16), len(tail)):
        for j in range(min(8, len(head))):
            n = 0
            while i + n < len(tail) and j + n < len(head) and tail[i + n] and tail[i + n] == head[j + n]:
                n += 1
            if n >= 2:
                candidates.append((n, j + n))
    if not candidates:
        return 0
    return max(candidates)[1]


def run_longform(
    session: Any, samples: np.ndarray, *,
    sample_rate: int, language: str | None,
    timestamps: str, profile: ChunkProfile,
    prompt: str | None = None,
) -> Any:
    windows = chunk_windows(samples, sample_rate, profile)
    pieces: list[str] = []
    segments: list[SimpleNamespace] = []
    word_rows: list[SimpleNamespace] = []
    seam_checks = []
    for index, (start, end) in enumerate(windows):
        result = session.run(
            samples[start:end], language=language,
            timestamps=timestamps, prompt=prompt,
        )
        words_text = str(getattr(result, "text", "") or "").strip()
        # Never silently return partial long-form audio for a non-silent chunk.
        chunk_samples = np.asarray(samples[start:end], dtype=np.float32)
        speech_rms = float(np.sqrt(np.mean(chunk_samples * chunk_samples)))
        if not words_text and speech_rms > 0.008:
            raise RuntimeError(f"ASR omitted speech in long-form window #{index}")
        prior = pieces[-1] if pieces else ""
        cut = overlap_drop(prior, words_text) if profile.overlap_seconds > 0 else 0
        remaining = " ".join(words_text.split()[cut:])
        if index and profile.overlap_seconds > 0:
            seam_checks.append({"index": index, "trimmed_words": cut, "seam_confirmed": cut >= 2})
        if remaining:
            pieces.append(remaining)
        # When a decoder has no segment-level timings, the window's actual
        # time bounds are the best truthful bounds (NOT word timestamps).
        offset_ms = round(start * 1000 / sample_rate)
        t0 = offset_ms
        t1 = round(end * 1000 / sample_rate)
        native = getattr(result, "segments", ()) or ()
        native_time_valid = bool(native) and all(
            getattr(s, "t1_ms", 0) > getattr(s, "t0_ms", 0) >= 0
            and getattr(s, "t1_ms", 0) <= round((end - start) * 1000 / sample_rate) + 80
            for s in native
        )
        if native_time_valid and cut == 0:
            segments.extend(
                SimpleNamespace(text=s.text, t0_ms=s.t0_ms + offset_ms,
                                t1_ms=s.t1_ms + offset_ms)
                for s in native
            )
        elif remaining:
            segments.append(SimpleNamespace(text=remaining, t0_ms=t0, t1_ms=t1))
        if native_time_valid and cut == 0:
            word_rows.extend(
                SimpleNamespace(text=w.text, t0_ms=w.t0_ms + offset_ms,
                                t1_ms=w.t1_ms + offset_ms)
                for w in (getattr(result, "words", ()) or ())
            )
    text = " ".join(pieces).strip()
    if not text:
        raise RuntimeError("ASR returned no long-form transcript")
    return SimpleNamespace(
        text=text, segments=tuple(segments),
        words=tuple(word_rows),
        language=language,
        longform={"segments": len(windows), "seams": seam_checks,
                  "covered_samples": len(samples), "overlap_seconds": profile.overlap_seconds},
    )
