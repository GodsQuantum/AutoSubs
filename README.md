<p align="center">
  <img src="docs/logo.svg" width="112" alt="AutoSubs logo">
</p>

<h1 align="center">AutoSubs</h1>

<p align="center">
  <b>Transcribe it. Fix it. Style it. Burn it — or automate the whole folder.</b><br>
  A self-hosted subtitle production workbench built around Rust, SvelteKit, FFmpeg/libass and your own transcription provider.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-3dd7cf" alt="MIT license">
  <img src="https://img.shields.io/badge/backend-Rust-ef7d57" alt="Rust backend">
  <img src="https://img.shields.io/badge/UI-SvelteKit-ff3e00" alt="SvelteKit UI">
  <img src="https://img.shields.io/badge/media-FFmpeg-4a9f43" alt="FFmpeg">
  <img src="https://img.shields.io/badge/UI-EN%20%2F%20FR-9d8cf5" alt="English and French UI">
  <img src="https://img.shields.io/badge/image-GHCR-54cd8a" alt="GHCR image">
</p>

<p align="center">🇫🇷 <a href="README.fr.md">README en français</a></p>

---

AutoSubs is for the part that happens **after you have a video**: get word timings from Whisper/Speaches or import an existing subtitle file, clean up the text, repair bad timing, style it once, preview the right canvas, and let FFmpeg burn it with libass. For recurring work, a Workflow watches a folder and runs the same pipeline automatically.

It is deliberately not a browser-only subtitle toy. The Rust backend owns timing normalization, segmentation, persistence, rendering and workflow state. The web UI is a client of that API, so manual jobs and automated jobs go through the same rules.

## 📸 Screenshots

<p align="center"><img src="docs/screenshot-queue.svg" width="900" alt="AutoSubs production queue"></p>
<p align="center"><i>Queue — local/resumable uploads and server-side files feed the same persistent job system.</i></p>

<p align="center"><img src="docs/screenshot-editor.svg" width="900" alt="AutoSubs subtitle editor"></p>
<p align="center"><i>Editor — seekable video preview, subtitle list, canonical regrouping, timing tools and subtitle-only exports.</i></p>

<p align="center"><img src="docs/screenshot-mobile.svg" width="900" alt="AutoSubs responsive mobile interface"></p>
<p align="center"><i>The interface is designed for desktop, tablet and phone — not just squeezed into a smaller viewport.</i></p>

## ✨ What it does

- **Video ingest without a tiny extension allow-list** — AutoSubs asks `ffprobe` whether a stable file actually contains video.
- **Resumable browser uploads** — tus 1.0-style `HEAD`/`PATCH` uploads resume after network loss; re-selecting the same file after a reload resumes from the server offset.
- **Server-side picker** — use files already mounted into the container instead of uploading them again.
- **Sidecar-first workflow** — import `.ass`, `.ssa`, `.srt` or AutoSubs JSON; attach, replace or detach a sidecar before rendering.
- **Transcription providers** — OpenAI-compatible transcription endpoints plus an optional local provider/fallback such as Speaches.
- **Optional LLM correction** — spelling/punctuation correction after import/transcription while preserving line count and timings.
- **Canonical subtitle timing engine** — one Rust implementation repairs invalid ranges, overlaps, gaps and word timings. The browser does not maintain a competing copy.
- **Unicode-aware line grouping** — grapheme counting, Unicode line-break opportunities and French no-break rules instead of UTF-8 byte counting.
- **ASS/libass styling** — Pop, Highlight, Bounce, Karaoke, Fade, Slide-up and plain subtitle modes; custom fonts, outline, shadow, placement and highlight colors.
- **Durable word timing** — the canonical word-by-word timeline survives regrouping and visual edits; timed animations use the original word timings.
- **French-safe visual layout** — French syntax-aware segmentation and hard `maxLines` enforcement use explicit line breaks, so rendered captions never gain hidden extra lines.
- **Complete job lifecycle** — edit, split, merge, delete, retranscribe and re-render jobs from the queue/editor. Deleting a job keeps its source and final output files.
- **Source geometry invariant** — Source + Preserve keeps the primary video dimensions and aspect ratio without scale, pad, crop, or black bars.
- **Real format profiles** — Source, 9:16, 16:9, 1:1, 4:5 and custom canvases. Source geometry is preserved by default; `contain`, `cover` and `stretch` are explicit choices.
- **Brands** — group logo/outro assets and choose a default preset per output format.
- **Workflows** — independent watch/output/archive folders, Brand/preset resolution, native filesystem events plus periodic reconciliation for NFS-mounted folders.
- **No archive-on-failure** — a source is archived only after the final video and subtitle sidecars have been published successfully.
- **Transactional output publication** — existing outputs are kept recoverable until the new video + SRT + ASS + JSON set is committed.
- **Persistent jobs** — SQLite keeps queue state, settings, workflows, assets and events across restarts. Active jobs become `interrupted` after an unexpected restart instead of pretending they completed.
- **Real cancellation** — waiting jobs and running FFmpeg/network work use cancellation tokens; a cancelled job does not later consume a freed encode slot.
- **Machine-readable render progress** — progress comes from FFmpeg's `-progress` protocol, not regexes against human stderr output.
- **Hardware encoder discovery** — AutoSubs probes the FFmpeg build and can select NVENC, QSV, VA-API, AMF or libx264 according to what is actually present. `auto` falls back once to libx264 if hardware launch fails.
- **Outro normalization** — main video and outro are normalized into one concat graph so different dimensions/FPS/audio layouts do not require a fragile stream-copy concat.
- **EN / FR UI** — instant browser-local language switch. This is separate from the transcription language setting.

## 🚀 Install

The pre-built image is published as `ghcr.io/godsquantum/autosubs:latest`.

```bash
mkdir autosubs && cd autosubs
curl -O https://raw.githubusercontent.com/GodsQuantum/AutoSubs/main/compose.example.yaml
curl -O https://raw.githubusercontent.com/GodsQuantum/AutoSubs/main/.env.example
cp .env.example .env
mkdir -p config data fonts media
# adjust MEDIA_PATH / bind mounts as needed, then:
docker compose -f compose.example.yaml up -d
```

Open `http://<server-ip>:3051`.

### Storage rule that matters

`/config` contains `autosubs.db` and **must be local storage**. AutoSubs uses SQLite WAL and refuses known network filesystems such as NFS/CIFS/SSHFS for the database path. Your videos, watch folders and outputs can absolutely live on NFS; mount them separately under an allowed media root.

A typical layout is:

```text
/config             local SSD / host filesystem — SQLite only
/data               local or fast app working data — uploads/jobs/renders
/fonts              custom fonts, read-only is fine
/srv/media/...        large source/output/archive trees
```

## NAS / external media deployment

If your media already lives on a NAS, mount the NAS once and expose only the roots AutoSubs is allowed to browse:

```yaml
services:
  autosubs:
    image: ghcr.io/godsquantum/autosubs:latest
    container_name: autosubs
    init: true
    restart: unless-stopped
    user: "1000:1000"
    ports:
      - "3051:3000"
    environment:
      TZ: UTC
      AUTOSUBS_CONFIG_DIR: /config
      AUTOSUBS_DATA_DIR: /data
      AUTOSUBS_FONTS_DIR: /fonts
      AUTOSUBS_ALLOWED_ROOTS: /data:/srv/media
      AUTOSUBS_MAX_RENDER_JOBS: "2"
      AUTOSUBS_MAX_TRANSCRIPTION_JOBS: "2"
      AUTOSUBS_LOCAL_TRANSCRIPTION_ENABLED: "true"
      AUTOSUBS_LOCAL_TRANSCRIPTION_URL: http://transcriber:8000/v1/audio/transcriptions
    volumes:
      - ./config:/config
      - ./data:/data
      - ./fonts:/fonts:ro
      - /srv/media:/srv/media
```

Provider environment variables **bootstrap an empty database only**. After first start, Settings in the UI are authoritative. That avoids a container restart unexpectedly overwriting a key/URL you changed from the UI.

For migration, the old `SPEACHES_URL` variable is still accepted as a first-boot alias for `AUTOSUBS_LOCAL_TRANSCRIPTION_URL`.

## 🎬 Manual production flow

1. Add a local video, choose a server-side video, or pair a video with `.srt` / `.ass` / `.ssa` / `.json`.
2. AutoSubs probes the media and imports or generates subtitle word timings.
3. Optional LLM correction runs on text only.
4. The canonical Rust engine normalizes timings and grouping.
5. The job reaches **Ready**. Nothing has been re-encoded yet unless you explicitly chose immediate render.
6. Review/edit/split/merge/delete/search/replace/regroup/shift timings in Editor. The canonical word timeline remains available for later regrouping.
7. Export SRT/ASS/JSON without touching the video, or click **Render video**.
8. FFmpeg/libass renders to a `.partial` staging file. `.partial` is internal media staging and is never treated as a subtitle file.
9. Video and sidecars publish together; an optional source archive happens last. Existing jobs can be retranscribed or re-rendered from the queue.

That Ready step is intentional: correcting three words should not cost another full encode just to inspect the result.

## Brands, presets and formats

A **Preset** owns visual subtitle behavior: typography, animation, colors, placement, segmentation limits, fit mode, target format and optional outro override.

A **Brand** can own a logo, default outro and one default preset for each format. A Workflow resolves its style in this order:

```text
explicit workflow preset
        ↓
brand default for the workflow format
        ↓
global/default preset resolution
```

The Job's selected output format remains authoritative. Picking a preset does not silently turn a 16:9 job back into 9:16.

## 🔄 Watch folders

Workflows combine low-latency filesystem events with periodic reconciliation. This matters on NFS: a remote write does not necessarily produce the local inotify event you expected.

Before claiming a candidate AutoSubs checks that its size/mtime stay stable, probes it with `ffprobe`, deduplicates it persistently, then looks for matching sidecars in priority order:

```text
.ass / .ssa  →  .srt  →  .json  →  transcription
```

If any preparation or render step fails, the original source stays where it was.

## FFmpeg and hardware acceleration

The runtime image includes FFmpeg with libass. On startup AutoSubs probes filters, hardware accelerators and H.264 encoders. The Settings page shows what this particular container can actually use.

For Intel/AMD Linux acceleration, expose `/dev/dri` to the container and add the host video/render groups as required by your distro. For NVIDIA, use the NVIDIA Container Toolkit and expose the GPU in your Compose stack. Hardware access is intentionally not enabled by default in the example Compose.

`auto` is conservative: if a selected hardware encoder fails to launch, that render is retried once with `libx264`. It does not silently loop through six encoders.

## ⚙️ Configuration

Core runtime variables:

| Variable | Default | Purpose |
|---|---|---|
| `AUTOSUBS_PORT` | `3000` | HTTP port inside the container. |
| `AUTOSUBS_CONFIG_DIR` | `/config` | Local SQLite/config directory. |
| `AUTOSUBS_DATA_DIR` | `/data` | Upload/job/render working data. |
| `AUTOSUBS_FONTS_DIR` | `/fonts` | Custom font directory. |
| `AUTOSUBS_ALLOWED_ROOTS` | `/data:/media` in the image | Colon-separated roots exposed by the server picker/workflows. If omitted outside Docker, AutoSubs falls back to its data directory. |
| `AUTOSUBS_MAX_RENDER_JOBS` | `2` | Concurrent expensive render slots. |
| `AUTOSUBS_MAX_TRANSCRIPTION_JOBS` | `2` | Concurrent transcription slots. |
| `AUTOSUBS_MAX_QUEUED_JOBS` | `256` | Maximum number of active queued/processing jobs admitted at once. |
| `AUTOSUBS_WORKFLOW_SCAN_SECONDS` | `5` | Periodic workflow reconciliation interval (also covers NFS writes missed by native events). |
| `AUTOSUBS_FILE_STABILITY_MS` | `2000` | Required stable-size/mtime window before a watched file is accepted. |
| `AUTOSUBS_MAX_UPLOAD_BYTES` | `53687091200` | Maximum resumable upload size (50 GiB). |

First-database bootstrap variables:

| Variable | Purpose |
|---|---|
| `AUTOSUBS_TRANSCRIPTION_LANGUAGE` | Initial transcription language. |
| `AUTOSUBS_TRANSCRIPTION_URL` | Initial external transcription endpoint. |
| `AUTOSUBS_TRANSCRIPTION_MODEL` | Initial external model. |
| `AUTOSUBS_LOCAL_TRANSCRIPTION_ENABLED` | Enable the local provider initially. |
| `AUTOSUBS_LOCAL_TRANSCRIPTION_URL` | Initial Speaches/local endpoint. |
| `AUTOSUBS_LOCAL_TRANSCRIPTION_MODEL` | Initial local model. |
| `AUTOSUBS_LOCAL_FALLBACK_ENABLED` | Allow local fallback after external failure. |
| `AUTOSUBS_TRANSCRIPTION_API_KEY` | Optional initial external-provider key. |
| `AUTOSUBS_LOCAL_TRANSCRIPTION_API_KEY` | Optional initial local-provider key. |

Provider keys can be bootstrapped through environment variables or configured later in Settings. Stored secrets are never echoed back to the browser.

## API

The current API is versioned under `/api/v1`:

```text
GET          /api/v1/health
GET          /api/v1/capabilities
GET          /api/v1/events                         SSE

GET/POST     /api/v1/jobs
GET/PUT/DELETE /api/v1/jobs/{id}                      Delete keeps source/final media
GET          /api/v1/jobs/{id}/media                Range-aware video stream
POST         /api/v1/jobs/{id}/cancel
POST         /api/v1/jobs/{id}/render
POST         /api/v1/jobs/{id}/retranscribe
PUT/DELETE   /api/v1/jobs/{id}/sidecar
GET/PUT      /api/v1/jobs/{id}/subtitles
POST         /api/v1/jobs/{id}/subtitles/regroup
POST         /api/v1/jobs/{id}/subtitles/shift
GET          /api/v1/jobs/{id}/subtitles/export

GET          /api/v1/fonts                             Detected custom font catalog
GET          /api/v1/fonts/css                         Browser @font-face stylesheet
GET          /api/v1/fonts/{id}/content                Safe font content endpoint

POST         /api/v1/uploads                        tus create
HEAD/PATCH   /api/v1/uploads/{id}                   tus resume/upload
DELETE       /api/v1/uploads/{id}

GET/POST     /api/v1/presets
GET/PUT/DEL  /api/v1/presets/{id}
GET/POST     /api/v1/brands
GET/PUT/DEL  /api/v1/brands/{id}
GET/POST     /api/v1/workflows
GET/PUT/DEL  /api/v1/workflows/{id}
GET/PUT      /api/v1/settings
GET          /api/v1/files/roots
GET          /api/v1/files/browse
GET/POST     /api/v1/assets
DELETE       /api/v1/assets/{id}
```

The server-side picker canonicalizes requested paths after symlink resolution and rejects paths outside `AUTOSUBS_ALLOWED_ROOTS`.

## Development

The release build uses Node only as a frontend build stage. The runtime image contains no Node.js toolchain.

```bash
# Rust
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features

# Frontend
cd frontend
npm ci
npm run check
npm test
npm run build

# Full image
docker build -t autosubs:dev .
```

`main` is gated by CI for Rust, SvelteKit and Docker. Dependency updates cover Cargo, npm, Docker and GitHub Actions. Tagged releases publish multi-architecture GHCR images with SBOM/provenance metadata.

Contributions are welcome — see [.github/CONTRIBUTING.md](.github/CONTRIBUTING.md). For usage help, see [.github/SUPPORT.md](.github/SUPPORT.md).

## Project layout

```text
src/api/          HTTP/SSE/tus endpoints
src/jobs.rs       persistent queue + job runner
src/media/        ffprobe, transcription, FFmpeg plans and rendering
src/subtitle/     SRT/ASS, timing normalization, segmentation, LLM correction
src/workflows.rs  watcher supervisor + periodic reconciliation
frontend/         SvelteKit static client + frontend tests
docs/             docs and versioned UI previews

Rust regression tests live next to the modules they exercise under `#[cfg(test)]`.
```

## 🔒 Security

AutoSubs can read and write mounted media paths and can invoke FFmpeg on them. Do not expose it directly to the public Internet. Put it behind your normal authenticated reverse proxy/VPN and mount only the directories it actually needs.

See [security policy](.github/SECURITY.md) for vulnerability reporting and deployment notes.

## License

MIT — see [LICENSE](LICENSE).
