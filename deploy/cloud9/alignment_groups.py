"""Validated CTC forced-alignment groups preserving ASR chunk coordinates.

Never split the transcript by guessed average words-per-second: use ASR segments
with real media offsets, or refuse long inputs without them.
"""
from __future__ import annotations

import json
import math
import re
import unicodedata


def _key(text: str) -> list[str]:
    text = unicodedata.normalize("NFKD", text.casefold())
    text = "".join(ch for ch in text if not unicodedata.combining(ch))
    return re.findall(r"[\w]+", text, flags=re.UNICODE)


def groups_for_alignment(
    transcript: str, audio_duration: float, raw_segments: str | None,
    *, max_chunk_seconds: float = 38.0,
) -> list[dict]:
    if not transcript.strip() or not math.isfinite(audio_duration) or audio_duration <= 0:
        raise ValueError("nonempty transcript and finite audio duration required")
    if raw_segments is None or not raw_segments.strip():
        if audio_duration > max_chunk_seconds:
            raise ValueError(
                "long audio requires ASR segments with real audio offsets; "
                "cannot assign words to chunks by interpolation"
            )
        return [{"start": 0.0, "end": audio_duration, "text": transcript}]
    segments = json.loads(raw_segments)
    if not isinstance(segments, list) or not segments:
        raise ValueError("alignment segments must be a nonempty list")
    out = []
    group = None
    prev_start = -1.0
    for i, row in enumerate(segments):
        if not isinstance(row, dict):
            raise ValueError(f"segment {i} is not a mapping")
        try:
            a = float(row["start"])
            b = float(row["end"])
            text = str(row["text"]).strip()
        except (TypeError, ValueError, KeyError) as exc:
            raise ValueError(f"invalid segment {i}") from exc
        if (not text or not math.isfinite(a) or not math.isfinite(b)
                or a < 0 or a < prev_start or b <= a or b > audio_duration + 0.08):
            raise ValueError(f"invalid ASR segment bounds/text at {i}")
        b = min(b, audio_duration)
        if group is None and out and a < out[-1]["end"]:
            # The STT text has already removed lexical duplicates in the audio
            # overlap. Don't align those kept words twice on overlapping media.
            a = out[-1]["end"]
        if b - a > max_chunk_seconds + 0.25:
            raise ValueError(f"ASR segment {i} exceeds max acoustic alignment window")
        if group is not None and b - group["start"] <= max_chunk_seconds:
            group["end"] = b
            group["text"] += " " + text
        else:
            if group is not None:
                out.append(group)
            if out and a < out[-1]["end"]:
                a = out[-1]["end"]
            if b <= a:
                raise ValueError(f"segment {i} overlaps previous acoustic window entirely")
            group = {"start": a, "end": b, "text": text}
        prev_start = a
    if group is not None:
        out.append(group)
    if _key(" ".join(g["text"] for g in out)) != _key(transcript):
        raise ValueError(
            "ASR segments and final transcript differ: refusing to align possibly omitted words"
        )
    if len(out) > 1 and out[-1]["end"] < audio_duration - 10:
        raise ValueError("ASR segments end before media ends: likely truncated transcript")
    return out
