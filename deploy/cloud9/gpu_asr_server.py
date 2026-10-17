"""OpenAI-compatible GPU-first STT API for Cloud9, using isolated Vulkan workers."""
from __future__ import annotations
import asyncio
import json
import os
import tempfile
from pathlib import Path
from fastapi import FastAPI, File, Form, HTTPException, UploadFile

CATALOG = {
    "whisper-large-v3-turbo-q6": "Whisper Large-v3-Turbo GGUF Q6_K · AMD Vulkan",
    "whisper-large-v3-turbo-q8": "Whisper Large-v3-Turbo GGUF Q8_0 · AMD Vulkan",
    "whisper-large-v3-q6": "Whisper Large-v3 GGUF Q6_K · AMD Vulkan",
    "canary-1b-v2-q6": "Canary 1B v2 GGUF Q6_K · bounded longform · AMD Vulkan",
    "cohere-transcribe-03-2026-q4": "Cohere Transcribe 03-2026 GGUF Q4_K_M · bounded longform · AMD Vulkan",
}
LIMIT = 150 * 1024 * 1024
app = FastAPI(title="AutoSubs GPU Vulkan STT", version="1.1.0")
_gate = asyncio.Semaphore(1)


@app.get("/health")
def health():
    return {"status": "ok", "backend": "vulkan-worker-isolated", "models": list(CATALOG)}


@app.get("/v1/models")
def models():
    return {"object": "list", "data": [{"id": name, "object": "model", "owned_by": "Cloud9", "description": label}
                                        for name, label in CATALOG.items()]}


@app.post("/v1/audio/transcriptions")
async def transcribe(file: UploadFile = File(...),
                     model: str = Form("whisper-large-v3-turbo-q6"),
                     language: str = Form("fr"),
                     response_format: str = Form("json")):
    model = model.strip() or "whisper-large-v3-turbo-q6"
    language = language.strip().lower()
    if model not in CATALOG:
        raise HTTPException(status_code=422, detail=f"unknown model: {model}")
    if language not in {"fr", "en", "zh"}:
        raise HTTPException(status_code=422, detail="language must be fr, en or zh")
    if response_format not in {"json", "verbose_json"}:
        raise HTTPException(status_code=422, detail="response_format must be json or verbose_json")
    fd, path = tempfile.mkstemp(prefix="autosubs-vulkan-", suffix=".wav")
    try:
        with os.fdopen(fd, "wb") as output:
            written = 0
            while chunk := await file.read(1 << 20):
                written += len(chunk)
                if written > LIMIT:
                    raise HTTPException(status_code=413, detail="input too large")
                output.write(chunk)
        async with _gate:
            child = await asyncio.create_subprocess_exec(
                "/home/ubuntu/speaches/.venv/bin/python",
                "/srv/gpu_asr_worker.py", path, model, language,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                env={**os.environ, "TRANSCRIBE_BACKEND": "vulkan"},
            )
            try:
                stdout, stderr = await asyncio.wait_for(child.communicate(), timeout=1800)
            except TimeoutError:
                child.kill()
                await child.communicate()
                raise HTTPException(status_code=504, detail="GPU worker timeout")
        rows = [r for r in stdout.decode(errors="replace").splitlines() if r.lstrip().startswith("{")]
        if child.returncode != 0 or not rows:
            # Avoid leaking full stderr, filenames, environment or GPU internals.
            raise HTTPException(status_code=503, detail="GPU worker failed; no CPU fallback was substituted")
        packet = json.loads(rows[-1])
        if not packet.get("ok"):
            raise HTTPException(status_code=422, detail=packet.get("error", "GPU recognition failed")[:300])
        transcript = packet["result"]
        return transcript if response_format == "verbose_json" else {"text": transcript["text"]}
    finally:
        try:
            os.unlink(path)
        except FileNotFoundError:
            pass
