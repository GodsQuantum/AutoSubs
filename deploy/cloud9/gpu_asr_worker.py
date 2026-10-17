"""One request per process to avoid persistent Vulkan device-loss in Speaches."""
from __future__ import annotations
import glob
import json
import math
import sys
import time
import wave
from pathlib import Path

import numpy as np
from transcribe_cpp import Model
from cloud9_longform import profile_for_arch, run_longform

CATALOG = {
    "whisper-large-v3-turbo-q6": ("models--handy-computer--whisper-large-v3-turbo-gguf", "*Q6_K.gguf"),
    "whisper-large-v3-turbo-q8": ("models--handy-computer--whisper-large-v3-turbo-gguf", "*Q8_0.gguf"),
    "whisper-large-v3-q6": ("models--handy-computer--whisper-large-v3-gguf", "*Q6_K.gguf"),
    "canary-1b-v2-q6": ("models--handy-computer--canary-1b-v2-gguf", "*Q6_K.gguf"),
    "cohere-transcribe-03-2026-q4": ("models--handy-computer--cohere-transcribe-03-2026-gguf", "*Q4_K_M.gguf"),
}

def transcribe(path: str, model_name: str, language: str) -> dict:
    if model_name not in CATALOG:
        raise ValueError(f"model is not available: {model_name}")
    root, pattern = CATALOG[model_name]
    files = glob.glob(f"/models/{root}/snapshots/*/{pattern}")
    if not files:
        raise ValueError(f"model weights are not installed: {model_name}")
    with wave.open(path, "rb") as audio:
        if audio.getnchannels() != 1 or audio.getframerate() != 16000 or audio.getsampwidth() != 2:
            raise ValueError("expected 16 kHz mono PCM16 WAV")
        pcm = np.frombuffer(audio.readframes(audio.getnframes()), dtype="<i2").astype("float32") / 32768.
    duration = len(pcm) / 16000.
    if duration <= 0:
        raise ValueError("empty WAV")
    start = time.perf_counter()
    with Model(files[0], backend="vulkan") as engine:
        profile = profile_for_arch(engine.arch, duration)
        if profile is None:
            with engine.session(n_threads=6) as session:
                result = session.run(pcm, language=language, timestamps="segment")
        else:
            with engine.session(n_threads=6) as session:
                result = run_longform(
                    session, pcm, sample_rate=16000,
                    language=language, timestamps="none",
                    profile=profile,
                )
    seconds = round(time.perf_counter() - start, 3)
    text = result.text.strip()
    if not text or len(text.split()) < 1:
        raise ValueError("ASR generated no speech")
    segments = []
    for seg in result.segments:
        a = float(seg.t0_ms) / 1000.
        b = min(duration, float(seg.t1_ms) / 1000.)
        if not math.isfinite(a) or not math.isfinite(b) or a < 0 or b < a:
            raise ValueError("ASR returned invalid segment timestamps")
        segments.append({"text": seg.text, "start": a, "end": b})
    return {"text": text, "segments": segments, "language": language,
            "duration": duration, "model": model_name,
            "asr_device": "vulkan", "asr_seconds": seconds,
            "timestamp_granularities": ["segment"],
            "alignment_required": True,
            "longform": getattr(result, "longform", {"segments": 1, "policy": "native"})}

def main():
    try:
        data = transcribe(sys.argv[1], sys.argv[2], sys.argv[3])
        print(json.dumps({"ok": True, "result": data}, ensure_ascii=False))
    except BaseException as error:
        print(json.dumps({"ok": False, "error": f"{type(error).__name__}: {error}"}, ensure_ascii=False))
        sys.exit(2)

if __name__ == "__main__":
    main()
