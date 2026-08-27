# AutoSubs Premium Design — 2026-08-27

## Product goal

AutoSubs is a self-hosted, API-first subtitle production tool. It ingests arbitrary FFmpeg-readable video, reuses or generates subtitle timing, lets the user correct and style subtitles, renders them into the original or a target format, and automates the same pipeline through independent watch-folder workflows.

The rewrite must satisfy **V1 functional parity + current multi-workflow behavior + the August 2026 master plan + the refinements below**.

## Hard constraints

- Backend/core: Rust stable 1.98, edition 2024.
- Frontend: Svelte 5 + stable SvelteKit 2, not React and not SvelteKit 3 pre-release.
- UI is a client of the API. Business rules do not live only in the browser.
- Runtime image does not contain Node.js.
- FFmpeg/ffprobe are the media capability boundary. Do not hard-code a tiny list of “supported video formats”.
- Preserve source geometry by default. Cropping/reframing is explicit.
- Failed transcription/render must never archive or delete the source.
- All persisted writes are atomic and errors are surfaced.

## 1. Subtitle correctness engine

### Canonical backend engine

The browser no longer owns a second implementation of overlap correction. Every import, transcription, edit save, regroup, automation job and render passes through the same Rust normalization pipeline.

### Timing invariants

For every subtitle line:

- finite timestamps only;
- `0 <= start < end`;
- minimum duration configurable, default 80 ms;
- adjacent lines have at least a configurable gap, default 10 ms;
- every word satisfies `line.start <= word.start < word.end <= line.end`;
- word timestamps are monotonic and non-overlapping;
- editing text invalidates word timing only when tokenization actually changed.

When two lines overlap, repair uses a seam inside the overlap so timing distortion is shared between the two lines instead of always truncating one side. If the available window is smaller than two minimum-duration lines, the engine cascades forward deterministically.

### Better subtitle segmentation

A single regex is not sufficient for high-quality line breaks. The engine combines:

1. Unicode grapheme counting (not UTF-8 byte length);
2. Unicode line-break opportunities (UAX #14);
3. protected-token regexes for URLs, e-mail addresses, @mentions, #hashtags, decimal/version numbers, abbreviations and apostrophe-bound French forms;
4. weighted French semantic penalties and bonuses;
5. sentence punctuation as a strong block boundary;
6. balanced visual line width as a secondary objective.

French rules strongly avoid breaks inside or directly after articles, pronouns, prepositions, negation pairs and common multi-word connectors (`parce que`, `bien que`, `avant de`, `afin de`, `alors que`, `ainsi que`, `tel que`, `y a`, etc.). Explicit line breaks imported from SRT/ASS are preserved unless the user requests regrouping.

## 2. Formats and Brands

### Format profile

A preset targets one format profile:

- `source` (original dimensions/aspect ratio);
- `portrait_9_16`;
- `landscape_16_9`;
- `square_1_1`;
- `portrait_4_5`;
- `custom` width/height.

Each profile also has a fit mode: `preserve`, `contain`, `cover`, or `stretch`; `source + preserve` is default.

### Brand

A Brand is a first-class object grouping presets and assets for one identity. It contains:

- stable id + name;
- optional description;
- default outro asset;
- optional logo/watermark asset metadata for future-safe extension;
- default preset id per format profile;
- all preset ids belonging to the brand.

A workflow can select `brand + format` and automatically resolve the brand’s default preset for that format, or explicitly override with a specific preset.

## 3. Preview correctness

Every style preview uses the preset’s actual aspect ratio through CSS `aspect-ratio`, not a fixed Reels frame. The preview displays the actual target resolution and maps subtitle X/Y percentages into that coordinate space. `source` preview uses probed media dimensions when a media item is active; otherwise it uses a neutral 16:9 placeholder clearly labelled “source preview”.

## 4. File and directory selection

The backend exposes a safe picker API restricted to configured browse roots.

The picker returns directories and files with metadata. Callers specify selection mode:

- `directory` for workflow watch/output/archive locations;
- `file` for existing outro/media asset selection;
- `any` for future tooling.

File filters can restrict extensions. Paths are canonicalized before root checks, preventing `..` traversal and symlink escape. The UI uses one reusable picker modal for both files and folders.

## 5. Jobs, cancellation and persistence

Jobs have stable UUIDs and persisted metadata. Active jobs also own a `CancellationToken`.

Cancellation must:

- abort transcription HTTP work by dropping the request future;
- terminate active FFmpeg/ffprobe child processes;
- mark the persisted job `cancelled`;
- never report `done` after cancellation wins a race.

On restart, jobs previously persisted as active become `interrupted` rather than pretending to still run.

## 6. Workflow supervisor

A supervisor reconciles configured workflows with running watcher tasks.

- create/enable => start watcher;
- edit a running workflow => cancel and restart only that watcher;
- disable/delete => cancel watcher;
- app shutdown => cancel all.

Each watcher combines native filesystem notifications with periodic directory scans for NAS/NFS reliability. Files are deduplicated and must pass a stability check (size and mtime unchanged across samples) before processing. A workflow serializes processing by default; concurrency is configurable later without changing the public model.

Companion priority: `.ass` > `.srt` > `.json` > transcription. Raw transcription words are persisted as `_words.json` when generated.

Archival is a transaction boundary: output render + sidecars must exist successfully before source/companions are moved.

## 7. Media rendering

FFprobe detects source streams, dimensions, duration, FPS and codecs. Render plans are generated as typed data then translated to FFmpeg args, which makes them unit-testable.

- source/preserve: no crop/scale filter beyond subtitle burn;
- contain: scale + pad;
- cover: scale + crop;
- custom: explicit width/height;
- audio copy when possible; AAC normalization when outro concat requires homogeneous streams;
- encoder-specific quality options: x264/x265 CRF, NVENC CQ, future VAAPI/QSV mappings;
- outro concatenation is implemented, not just exposed in JSON.

Render staging files use an internal-only suffix such as `.partial`; they are never subtitle documents and are never exposed as a user interchange format. Subtitle work products remain explicit `.srt`, `.ass` and `.json` sidecars. The manual ingest flow accepts a video by itself or a video paired with one subtitle sidecar; a subtitle sidecar can also be attached/replaced after the video has already been uploaded. This preserves the useful workflow of importing or removing subtitle tracks without confusing incomplete media renders with subtitle files.

## 8. API

Primary API is `/api/v1`. Legacy routes from V1/current builds remain as compatibility aliases where practical.

Core surface:

- `/health`, `/capabilities`;
- jobs create/list/get/cancel/events;
- subtitle normalize/regroup;
- burn and batch burn;
- presets CRUD/import/export;
- brands CRUD;
- workflows CRUD;
- settings + model discovery;
- picker/browse;
- fonts/outros;
- automation trigger, job ASS and corrected re-burn.

Errors share a JSON envelope `{ "error": { "code", "message", "details?" } }`.

Settings GET does not echo stored API secrets. Secret patches support keep/replace/clear semantics.

## 9. SvelteKit UI

Stack (stable August 27, 2026):

- Svelte 5.56.x;
- SvelteKit 2.70.x;
- official Svelte Vite plugin 6.1.x;
- Vite 8.1 / Rolldown;
- TypeScript 7;
- Tailwind CSS 4.3;
- adapter-static with precompression.

Main screens:

1. Queue — ingest, status, edit, render, batch actions.
2. Brands — brand assets and format-specific preset map.
3. Presets — typography, animation, format, matching rules and live aspect-correct preview.
4. Workflows — multiple independent watch/output/archive pipelines with file/folder picker.
5. Settings — transcription primary/fallback, LLM correction, encoding, safe browse roots/capabilities.

## 10. Docker/GitHub

- Node 24 LTS frontend build stage.
- Rust 1.98 backend build stage.
- Debian slim runtime with FFmpeg, CA certs, tzdata and fonts only; non-root user.
- no `COPY .` into runtime;
- BuildKit cache mounts;
- healthcheck;
- `.dockerignore` excludes target/node_modules/build.
- compose has generic relative volumes/env overrides, never personal machine paths.
- GitHub Actions: frontend check/build, rust fmt/clippy/test, Docker build.
- Dependabot for Cargo + npm + Actions + Docker.

## Documentation

- `README.md` English.
- `README.fr.md` French.
- Direct SyncBridge-style structure: concept, features, API examples, Docker install, workflow configuration, brand/preset configuration.
- No claims without implementation/tests.
