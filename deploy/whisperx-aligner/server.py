import asyncio
import os
import tempfile
import threading
from pathlib import Path
from typing import Optional

import whisperx
from fastapi import FastAPI, File, Form, HTTPException, UploadFile

DEVICE = os.getenv("ALIGN_DEVICE", "cpu")
MODEL_DIR = os.getenv("ALIGN_MODEL_DIR", "/models")
THREADS = int(os.getenv("ALIGN_THREADS", "4"))
os.environ.setdefault("OMP_NUM_THREADS", str(THREADS))
os.environ.setdefault("MKL_NUM_THREADS", str(THREADS))

app = FastAPI(title="AutoSubs WhisperX Aligner", version="1.0.0")
_models = {}
_model_lock = threading.Lock()
_align_lock = asyncio.Lock()


def load_model(language: str, model_name: Optional[str]):
    key = (language, model_name or "")
    if key not in _models:
        with _model_lock:
            if key not in _models:
                model, metadata = whisperx.load_align_model(
                    language_code=language,
                    device=DEVICE,
                    model_name=model_name or None,
                    model_dir=MODEL_DIR,
                )
                _models[key] = (model, metadata)
    return _models[key]


def align_sync(
    path: str,
    transcript: str,
    language: str,
    model_name: Optional[str],
    start: Optional[float],
    end: Optional[float],
):
    audio = whisperx.load_audio(path)
    duration = len(audio) / 16000.0
    seg_start = max(0.0, float(start or 0.0))
    seg_end = min(duration, float(end if end is not None else duration))
    if seg_end <= seg_start:
        seg_start, seg_end = 0.0, duration
    model, metadata = load_model(language, model_name)
    result = whisperx.align(
        [{"start": seg_start, "end": seg_end, "text": transcript}],
        model,
        metadata,
        audio,
        DEVICE,
        return_char_alignments=False,
        print_progress=False,
    )
    return result.get("word_segments") or []


@app.get("/health")
def health():
    return {
        "status": "ok",
        "device": DEVICE,
        "loaded_models": [
            f"{language}:{model or 'default'}" for language, model in _models
        ],
    }


@app.post("/align")
async def align(
    file: UploadFile = File(...),
    transcript: str = Form(...),
    language: str = Form("fr"),
    model: Optional[str] = Form(None),
    start: Optional[float] = Form(None),
    end: Optional[float] = Form(None),
):
    suffix = Path(file.filename or "audio.wav").suffix or ".wav"
    fd, temporary = tempfile.mkstemp(prefix="autosubs-align-", suffix=suffix)
    os.close(fd)
    try:
        with open(temporary, "wb") as output:
            while chunk := await file.read(1024 * 1024):
                output.write(chunk)
        async with _align_lock:
            words = await asyncio.to_thread(
                align_sync,
                temporary,
                transcript,
                language,
                model,
                start,
                end,
            )
        if not words:
            raise HTTPException(status_code=422, detail="alignment returned no words")
        return {"word_segments": words}
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
