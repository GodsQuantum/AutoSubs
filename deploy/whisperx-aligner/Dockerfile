FROM python:3.12.15-slim-trixie

ENV PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PYTHONUNBUFFERED=1 \
    ALIGN_DEVICE=cpu \
    ALIGN_MODEL_DIR=/models \
    ALIGN_THREADS=4

RUN apt-get update \
 && apt-get install -y --no-install-recommends ffmpeg ca-certificates \
 && rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir torch==2.8.0 torchaudio==2.8.0 \
      --index-url https://download.pytorch.org/whl/cpu \
 && pip install --no-cache-dir "numpy>=2.1" "pandas>=2.2.3" "nltk>=3.9.1" "transformers>=4.48.0" \
 && pip install --no-cache-dir whisperx==3.8.6 --no-deps \
 && python -m nltk.downloader -q punkt punkt_tab \
 && pip install --no-cache-dir "fastapi==0.119.0" "uvicorn[standard]==0.37.0" "python-multipart==0.0.20"

WORKDIR /srv/aligner
COPY server.py /srv/aligner/server.py

EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/health', timeout=3).read()"

CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8000", "--workers", "1", "--no-access-log"]
