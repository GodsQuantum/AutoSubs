#!/usr/bin/env python3
"""Cloud9 stand-up corpus: lossless source preservation, aligned word sidecars, searchable index.
Uses the existing AutoSubs Docker FFmpeg and Cloud9-Speaches/WhisperX. Python stdlib only.
"""
import argparse
import datetime as dt
import json
import math
import os
from pathlib import Path
import re
import sqlite3
import subprocess
import sys
import time
import unicodedata
import urllib.error
import urllib.request
import uuid

ROOT = Path("/srv/storage/production/active/Montage/Sources/Stand-Up")
DB = Path("/srv/storage/nvme/DEV/Projets/Ssker Content Production/AutoSubs/standup-corpus.sqlite3")
TMP_HOST = Path("/srv/lxc/travail/appdata/autosubs/data/work/standup_corpus")
TMP_CONTAINER = "/data/work/standup_corpus"
STT = "http://192.168.1.219:8005/v1/audio/transcriptions"
ALIGN = "http://192.168.1.219:8006/align"
MODEL = "deepdml/faster-whisper-large-v3-turbo-ct2"
EXTS = {".mp4", ".mkv", ".mov", ".m4v", ".avi", ".webm"}
CHUNK_SECONDS = 240

def db_open():
    db = sqlite3.connect(DB, timeout=60)
    db.execute("PRAGMA journal_mode=WAL")
    db.execute("CREATE TABLE IF NOT EXISTS chunks (identity TEXT, part INTEGER, words_json TEXT NOT NULL, PRIMARY KEY(identity, part))")
    db.execute("CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, size INTEGER, mtime_ns INTEGER, duration REAL, words INTEGER, status TEXT, detail TEXT)")
    db.execute("CREATE VIRTUAL TABLE IF NOT EXISTS text_index USING fts5(path UNINDEXED, start UNINDEXED, end UNINDEXED, text, tokenize='unicode61 remove_diacritics 2')")
    db.commit()
    os.chown(DB,1000,1000)
    return db

def docker_cmd(*args, timeout=120):
    return subprocess.run(["pct", "exec", "300", "--", "docker", "exec", "autosubs", *args],
                          text=True, capture_output=True, timeout=timeout, check=True).stdout

def duration_seconds(video):
    source = "/data/production-active/" + video.relative_to(Path("/srv/storage/production/active")).as_posix()
    data = docker_cmd("ffprobe", "-v", "error", "-show_entries", "format=duration",
                      "-of", "default=noprint_wrappers=1:nokey=1", source)
    value = float(data.strip())
    if not math.isfinite(value) or value <= 0: raise ValueError("invalid duration")
    return value

def multipart(fields, filename, contents, url, timeout=600):
    boundary = "autosubs-" + uuid.uuid4().hex
    blocks = []
    for key, value in fields.items():
        blocks.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{key}"\r\n\r\n{value}\r\n'.encode())
    blocks.append(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{filename}"\r\nContent-Type: audio/wav\r\n\r\n'.encode())
    blocks.append(contents)
    blocks.append(f'\r\n--{boundary}--\r\n'.encode())
    request = urllib.request.Request(url, data=b"".join(blocks),
                                     headers={"Content-Type": f"multipart/form-data; boundary={boundary}"}, method="POST")
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return json.load(response)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(f"{url}: HTTP {exc.code}: {exc.read(900)!r}") from exc

def get_words(segment, start, n, language):
    clip_host = TMP_HOST / f"clip-{os.getpid()}-{uuid.uuid4().hex}.wav"
    clip_docker = f"{TMP_CONTAINER}/{clip_host.name}"
    source = "/data/production-active/" + segment.relative_to(Path("/srv/storage/production/active")).as_posix()
    try:
        docker_cmd("ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
                   "-ss", f"{start:.3f}", "-i", source, "-t", f"{n:.3f}", "-vn",
                   "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", clip_docker, timeout=180)
        sound = clip_host.read_bytes()
        transcript = multipart({"model": MODEL, "language": language, "response_format": "verbose_json",
                                "timestamp_granularities[]": "word"}, "clip.wav", sound, STT)
        raw_text = str(transcript.get("text") or "").strip()
        if not raw_text:
            # The model may legitimately detect a silent clip; do not fabricate words.
            return []
        aligned = multipart({"transcript": raw_text, "language": language},
                            "clip.wav", sound, ALIGN)
        output = aligned.get("result", aligned)
        rows = output.get("word_segments") or output.get("words") or []
        if not rows: raise ValueError("forced alignment returned no words")
        words = []
        last_end = -1.
        for row in rows:
            word = str(row.get("word") or row.get("text") or "").strip()
            b, e = float(row["start"]), float(row["end"])
            if not word or not math.isfinite(b) or not math.isfinite(e) or b < 0 or e <= b or e > n+0.08 or b < last_end - .015:
                raise ValueError(f"invalid forced alignment: {row!r}")
            # Tiny overlap tolerance due to float rounding, do not invent timestamp uniformity.
            b = max(b, last_end) if last_end >= 0 else b
            words.append({"word": word, "start": round(start+b, 3), "end": round(start+e, 3)})
            last_end = e
        return words
    finally:
        clip_host.unlink(missing_ok=True)

def assemble(words, max_chars=79, max_seconds=6.0, max_words=17):
    out, pack = [], []
    for word in words:
        if pack and ((word["end"]-pack[0]["start"] > max_seconds)
                     or len(pack) >= max_words or len(" ".join(w["word"] for w in pack+[word])) > max_chars
                     or word["start"]-pack[-1]["end"] > 1.15):
            out.append(pack); pack = []
        pack.append(word)
        if re.search(r"[.!?…]$", word["word"]) and len(pack) >= 4:
            out.append(pack); pack = []
    if pack: out.append(pack)
    return out

def timecode(sec, srt=False):
    ms = max(0, round(sec*1000))
    hrs, ms = divmod(ms, 3600000)
    mins, ms = divmod(ms, 60000)
    secs, ms = divmod(ms, 1000)
    return f"{hrs:02d}:{mins:02d}:{secs:02d}{',' if srt else '.'}{ms:03d}"

def srt_output(words):
    parts=[]
    for i, line in enumerate(assemble(words), 1):
        text = " ".join(x["word"] for x in line)
        middle = max(1, len(line)//2)
        if len(text)>49:
            a = " ".join(x["word"] for x in line[:middle])
            b = " ".join(x["word"] for x in line[middle:])
            text = a+"\n"+b
        parts.append(f"{i}\n{timecode(line[0]['start'], True)} --> {timecode(line[-1]['end'], True)}\n{text}\n")
    return "\n".join(parts)+"\n" if parts else ""

def markdown_output(video, words, duration, lang):
    relative = video.relative_to(ROOT).as_posix()
    safe_relative = relative.replace('"', '\\"')
    rows=[f"# {video.stem}", "",
          f"- **Fichier** : `{safe_relative}`",
          f"- **Durée** : {timecode(duration)}",
          f"- **Langue** : {lang}",
          f"- **Moteur** : {MODEL}; alignement WhisperX mot par mot",
          "- **Horodatages** : temps absolu de la vidéo source",
          "- **Statut** : transcription automatique à vérifier avant publication",
          "", "## Transcription", ""]
    pack=[]
    for w in words:
        if pack and (w["start"]-pack[0]["start"]>24 or len(pack)>=65 or w["start"]-pack[-1]["end"]>3):
            rows.extend([f"### [{timecode(pack[0]['start'])}]", "", " ".join(x["word"] for x in pack), ""]); pack=[]
        pack.append(w)
    if pack: rows.extend([f"### [{timecode(pack[0]['start'])}]", "", " ".join(x["word"] for x in pack), ""])
    if not words: rows.extend(["*Aucune parole détectée.*", ""])
    return "\n".join(rows)

def atomic_text(target, data, force=False):
    if target.exists() and not force: raise FileExistsError(f"refusing to overwrite {target}")
    temp = target.with_name(target.name + f".tmp-{uuid.uuid4().hex}")
    try:
        temp.write_text(data, encoding="utf-8")
        os.chown(temp,1000,1000)
        if target.exists() and not force: raise FileExistsError(str(target))
        os.replace(temp, target)
    finally: temp.unlink(missing_ok=True)

def export(db, video, words, duration, lang):
    stem = video.with_suffix("")
    paths=[Path(str(stem)+".md"), Path(str(stem)+".srt"), Path(str(stem)+".words.json")]
    payloads=[markdown_output(video, words, duration, lang), srt_output(words),
              json.dumps({"source": str(video), "duration":duration, "language":lang, "model":MODEL,
                          "alignment":"whisperx", "words":words}, ensure_ascii=False, indent=2)+"\n"]
    if all(path.exists() for path in paths):
        print(f"SKIP sidecars already present: {video.name}", flush=True); return
    for path, data in zip(paths, payloads):
        if not path.exists(): atomic_text(path, data)
    rel = video.relative_to(ROOT).as_posix()
    groups=assemble(words,max_chars=500,max_seconds=30,max_words=70)
    with db:
        db.execute("DELETE FROM text_index WHERE path=?", (rel,))
        db.executemany("INSERT INTO text_index(path,start,end,text) VALUES(?,?,?,?)",
                       [(rel,w[0]["start"],w[-1]["end"]," ".join(x["word"] for x in w)) for w in groups])

def run(db, limit=0, needle="", smoke=False):
    TMP_HOST.mkdir(mode=0o770, parents=True, exist_ok=True)
    os.chown(TMP_HOST,1000,1000)
    TMP_HOST.chmod(0o2770)
    items=sorted((p for p in ROOT.rglob("*") if p.is_file() and p.suffix.lower() in EXTS),
                 key=lambda p: (p.stat().st_size, str(p).lower()))
    items=[p for p in items if needle.casefold() in str(p).casefold()]
    failures=0
    completed=0
    for video in items:
        if limit and completed>=limit: break
        stat=video.stat()
        stem=video.with_suffix("")
        if not smoke and all(Path(str(stem)+ext).exists() for ext in (".md",".srt",".words.json")):
            continue
        ident=f"{video.resolve()}:{stat.st_size}:{stat.st_mtime_ns}"
        language = "en" if "anglais" in video.name.lower() else "fr"
        try:
            duration=duration_seconds(video)
            print(f"BEGIN {video.name} | {duration:.1f}s | {stat.st_size/1e6:.1f}MB",flush=True)
            all_words=[]
            total = math.ceil(duration/CHUNK_SECONDS)
            for i in range(total):
                start=i*CHUNK_SECONDS
                n=min(CHUNK_SECONDS,duration-start)
                cached=db.execute("SELECT words_json FROM chunks WHERE identity=? AND part=?",(ident,i)).fetchone()
                if cached and not smoke: chunk=json.loads(cached[0])
                else:
                    chunk=get_words(video,start,n,language)
                    if not smoke:
                        with db: db.execute("INSERT OR REPLACE INTO chunks VALUES(?,?,?)",
                                            (ident,i,json.dumps(chunk,ensure_ascii=False)))
                all_words.extend(chunk)
                print(f"CHUNK {i+1}/{total} words={len(chunk)} cumulative={len(all_words)}",flush=True)
                if smoke: break
            if smoke: return
            for index,w in enumerate(all_words):
                if w["start"] < 0 or w["end"] <= w["start"] or w["end"] > duration+0.15 or (index and w["start"] < all_words[index-1]["end"]-.015):
                    raise ValueError(f"word timing violates media bounds at {index}")
            export(db,video,all_words,duration,language)
            with db: db.execute("INSERT OR REPLACE INTO files VALUES(?,?,?,?,?,?,?)",
                                (video.relative_to(ROOT).as_posix(),stat.st_size,stat.st_mtime_ns,duration,len(all_words),"done",""))
            print(f"DONE {video.name} | words={len(all_words)}",flush=True)
            completed+=1
            failures=0
        except Exception as e:
            failures+=1
            with db: db.execute("INSERT OR REPLACE INTO files VALUES(?,?,?,?,?,?,?)",
                                (video.relative_to(ROOT).as_posix(),stat.st_size,stat.st_mtime_ns,0,0,"error",str(e)[:900]))
            print(f"ERROR {video}: {e}",file=sys.stderr,flush=True)
            if smoke or failures>=3: raise

def search(db, term):
    query=" ".join('"' + token.replace('"', "") + '"' for token in term.split())
    try:
        rows=db.execute("SELECT path,start,snippet(text_index,3,'[',']',' … ',26) FROM text_index WHERE text_index MATCH ? ORDER BY rank LIMIT 80",(query,)).fetchall()
    except sqlite3.OperationalError as e: raise ValueError(f"invalid FTS search: {e}") from e
    for path,start,snippet in rows: print(f"{path}  [{timecode(start)}]  {snippet}")
    print(f"{len(rows)} occurrences (limit 80)")

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation",choices=["run","smoke","search","stats"])
    parser.add_argument("query",nargs="?",default="")
    parser.add_argument("--limit",type=int,default=0)
    parser.add_argument("--filter",default="")
    args=parser.parse_args()
    if not ROOT.is_dir(): parser.error("stand-up root not mounted")
    with db_open() as db:
        if args.operation=="search": search(db,args.query)
        elif args.operation=="stats":
            for row in db.execute("SELECT status,count(*),sum(words) FROM files GROUP BY status"): print(*row)
            print("Indexed segments:",db.execute("SELECT count(*) FROM text_index").fetchone()[0])
        else: run(db,limit=args.limit,needle=args.filter,smoke=args.operation=="smoke")

if __name__=="__main__": main()
