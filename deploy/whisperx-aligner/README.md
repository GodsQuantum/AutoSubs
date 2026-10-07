# Optional WhisperX alignment provider

AutoSubs does **not** require this service. Native Whisper/faster-whisper word timestamps remain the fallback.

This reference provider adds a small HTTP boundary around WhisperX forced alignment. It accepts the transcript already produced by AutoSubs and returns validated word boundaries; it does not run a second transcription or perform subtitle grouping.

## Build

```sh
docker build -t autosubs-whisperx-aligner:3.8.6 .
```

The image intentionally uses CPU-only Torch. Keep the provider separate from GPU transcription so alignment cannot evict or contend with the ASR model.

## Run

```sh
docker run --rm -p 8006:8000 \
  --read-only --tmpfs /tmp:rw,noexec,nosuid,size=512m \
  -v "$PWD/models:/models" \
  autosubs-whisperx-aligner:3.8.6
```

Configure AutoSubs with alignment enabled and the endpoint `http://<aligner-host>:8006/align`. Leave the alignment model blank to use WhisperX's language-specific default, or provide a WhisperX-compatible model name.

## API

- `GET /health`
- `POST /align` multipart fields:
  - `file`: mono WAV/audio file
  - `transcript`: accepted transcript text
  - `language`: language code such as `fr`
  - `model`: optional alignment model
  - `start`, `end`: optional speech interval in seconds

The service serializes alignment jobs within one process and caches loaded alignment models. AutoSubs independently validates word count, lexical identity and monotonic boundaries before accepting the returned timing.
