"""GPU ROCm WhisperX word aligner for Cloud9 AutoSubs.

The endpoint never silently interpolates missing word boundaries. Every word
keeps its acoustic CTC score; a low score is marked for review, not a proof of
incorrect timing. Alignment with missing/invalid boundaries fails the request.
"""
from __future__ import annotations

import asyncio
from alignment_groups import groups_for_alignment
import math
import os
import tempfile
import threading
from pathlib import Path
from typing import Optional

import torch
import whisperx
import whisperx.alignment as whisper_alignment
from fastapi import FastAPI, File, Form, HTTPException, UploadFile

if not torch.cuda.is_available():
    raise RuntimeError("ROCm GPU unavailable: refusing CPU fallback in GPU-only aligner")

# WhisperX otherwise uses nearest interpolation and forward/back filling of
# NaN timestamps, making inferred positions indistinguishable from CTC evidence.
whisper_alignment.interpolate_nans = lambda x, method="nearest": x

DEVICE = "cuda"  # PyTorch ROCm advertises AMD via the CUDA API.
MODEL_DIR = os.environ.get("ALIGN_MODEL_DIR", "/models")
MAX_FILE_BYTES = int(os.environ.get("ALIGN_MAX_UPLOAD_BYTES", 150 * 1024 * 1024))
LOW_SCORE = float(os.environ.get("ALIGN_REVIEW_SCORE_THRESHOLD", "0.20"))
_lock = asyncio.Lock()
_load_lock = threading.Lock()
_models: dict[tuple[str, str], tuple] = {}
app = FastAPI(title="AutoSubs GPU Acoustic Aligner", version="1.1.0")


def get_model(language: str, name: Optional[str]):
    key = (language, name or "")
    if key not in _models:
        with _load_lock:
            if key not in _models:
                _models[key] = whisperx.load_align_model(
                    language_code=language,
                    device=DEVICE,
                    model_name=name or None,
                    model_dir=MODEL_DIR,
                )
    return _models[key]


def align_sync(path: str, transcript: str, language: str, model: Optional[str],
               start: Optional[float], end: Optional[float],
               segments_json: Optional[str] = None) -> dict:
    audio = whisperx.load_audio(path)
    duration = len(audio) / 16000.0
    if duration <= 0:
        raise ValueError("audio must contain nonempty speech waveform")
    groups = groups_for_alignment(transcript, duration, segments_json)
    if segments_json is None and (start is not None or end is not None):
        left = max(0.0, float(start) if start is not None else 0.0)
        right = min(duration, float(end) if end is not None else duration)
        if not (0 <= left < right <= duration):
            raise ValueError("segment bounds outside audio")
        groups = [{"start": left, "end": right, "text": transcript}]
    aligner, metadata = get_model(language, model)
    words = []
    for chunk_index, group in enumerate(groups):
        aligned = whisperx.align(
            [group], aligner, metadata, audio, DEVICE,
            return_char_alignments=False, print_progress=False,
        )
        chunk_words = aligned.get("word_segments") or []
        if not chunk_words:
            raise ValueError(f"acoustic alignment returned no words in chunk {chunk_index}")
        words.extend(chunk_words)
    if not words:
        raise ValueError("acoustic alignment returned no words")

    clean = []
    last_start = -1.0
    out_of_bounds = 0
    review = []
    for i, item in enumerate(words):
        text = item.get("word", "")
        a, b = item.get("start"), item.get("end")
        if not text or a is None or b is None:
            raise ValueError(f"missing acoustic boundary for word #{i}")
        a, b = float(a), float(b)
        if not math.isfinite(a) or not math.isfinite(b):
            raise ValueError(f"nonfinite acoustic boundary for word #{i}")
        if a < 0 or b <= a or a < last_start:
            raise ValueError(f"invalid or non-monotonic acoustic boundary at word #{i}")
        if b > duration:
            if b > duration + 0.06:
                raise ValueError(f"acoustic boundary exceeds media by >60ms at word #{i}")
            b = duration
            out_of_bounds += 1
        if b <= a:
            raise ValueError(f"acoustic boundary vanishes after media clamp at word #{i}")
        score = item.get("score")
        if score is not None:
            score = float(score)
            if not math.isfinite(score):
                raise ValueError(f"nonfinite CTC confidence at word #{i}")
            if score < LOW_SCORE:
                review.append({"index": i, "word": text, "score": round(score, 4)})
        clean.append({"word": str(text), "start": round(a, 6), "end": round(b, 6),
                      **({"score": round(score, 6)} if score is not None else {})})
        last_start = a

    return {"word_segments": clean,
            "quality": {"method": "CTC-forced-alignment-GPU", "backend": "rocm",
                        "interpolation": False, "low_confidence_words": review,
                        "clipped_to_media": out_of_bounds,
                        "audio_duration": duration, "language": language,
                        "alignment_chunks": len(groups)}}


@app.get("/health")
def health():
    return {"status": "ok", "device": "rocm", "gpu": torch.cuda.get_device_name(0),
            "loaded_models": [f"{k[0]}:{k[1] or 'default'}" for k in _models]}


@app.post("/align")
async def align(file: UploadFile = File(...), transcript: str = Form(...),
                language: str = Form("fr"), model: Optional[str] = Form(None),
                start: Optional[float] = Form(None), end: Optional[float] = Form(None),
                segments_json: Optional[str] = Form(None)):
    language = language.strip().lower()
    if language not in {"fr", "en", "zh"}:
        raise HTTPException(status_code=422, detail="unsupported alignment language: fr/en/zh")
    if not transcript.strip():
        raise HTTPException(status_code=422, detail="empty transcript")
    suffix = Path(file.filename or "audio.wav").suffix or ".wav"
    fd, temp = tempfile.mkstemp(prefix="autosubs-gpu-align-", suffix=suffix)
    try:
        with os.fdopen(fd, "wb") as output:
            total = 0
            while chunk := await file.read(1 << 20):
                total += len(chunk)
                if total > MAX_FILE_BYTES:
                    raise HTTPException(status_code=413, detail="audio file too large")
                output.write(chunk)
        async with _lock:
            try:
                return await asyncio.to_thread(
                    align_sync, temp, transcript, language, model, start, end, segments_json)
            except (ValueError, RuntimeError, KeyError) as error:
                raise HTTPException(status_code=422, detail=str(error)) from error
    finally:
        try:
            os.unlink(temp)
        except FileNotFoundError:
            pass
