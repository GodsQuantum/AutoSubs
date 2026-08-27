# AutoSubs Premium Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild AutoSubs as a production-ready Rust + SvelteKit application with V1 parity, robust subtitle timing/segmentation, Brands, multi-workflows, safe file/folder selection, and reproducible Docker/GitHub packaging.

**Architecture:** One Rust/Axum binary owns all business logic and serves a static SvelteKit client. JSON persistence stays migration-friendly, while runtime cancellation/supervision uses Tokio primitives. FFmpeg/ffprobe remain external media engines behind a typed render-plan layer.

**Tech Stack:** Rust 1.98 / edition 2024, Axum 0.8, Tokio, Reqwest, Notify, Svelte 5.56, SvelteKit 2.70, Vite 8.1, TypeScript 7, Tailwind 4.3, Node 24 LTS, FFmpeg.

**Spec:** `docs/superpowers/specs/2026-08-27-autosubs-premium-design.md`

## Global constraints

- Test-first for repaired/new backend behavior.
- Stable releases only; no SvelteKit 3 next and no experimental rsvelte compiler.
- Source geometry preserved by default.
- No source archival/deletion on pipeline failure.
- Backend is canonical for subtitle normalization and line grouping.
- Runtime container is non-root and contains no Node.js toolchain.

---

### Task 1: Repository and domain model foundation

**Files:** `Cargo.toml`, `.gitignore`, `.dockerignore`, `src/domain.rs`, `src/config.rs`, `src/persistence.rs`

- [ ] Define stable IDs and serde-compatible models for Settings, Preset, Brand, Workflow, Job, FormatProfile and FitMode.
- [ ] Add legacy preset/workflow migration helpers and tests.
- [ ] Implement atomic JSON store writes and tests around replacement/failure behavior.
- [ ] Keep persisted secrets internal and expose separate API DTOs.

### Task 2: Subtitle timing invariants

**Files:** `src/subtitle/normalize.rs`, `src/subtitle/mod.rs`

- [ ] Add failing regression tests for line overlap, word-over-line overflow, edited-token retiming, NaN/negative timestamps and cascading short windows.
- [ ] Implement seam-based adjacent-line repair.
- [ ] Clamp/redistribute word timings so every word is inside its parent line.
- [ ] Verify invariants with a reusable assertion helper across generated test cases.

### Task 3: Unicode/French segmentation engine

**Files:** `src/subtitle/segment.rs`

- [ ] Add tests for accented grapheme counts and protected tokens.
- [ ] Add French no-break tests for apostrophes and multi-word connectors (`avant de`, `afin de`, `parce que`, `bien que`, `alors que`, `ainsi que`, `tel que`, `y a`).
- [ ] Implement UAX #14 candidate breaks + protected-token regex + weighted semantic scoring.
- [ ] Replace byte-length scoring with grapheme-count scoring.

### Task 4: Subtitle parsers and ASS renderer

**Files:** `src/subtitle/srt.rs`, `src/subtitle/ass.rs`

- [ ] Add SRT/ASS round-trip tests, commas in ASS text, pop-mode duplicate handling and escaping tests.
- [ ] Preserve explicit imported line breaks until explicit regroup.
- [ ] Generate ASS for every format profile with correct PlayRes and coordinates.

### Task 5: Media probe/render plan

**Files:** `src/media/probe.rs`, `src/media/render.rs`, `src/media/process.rs`

- [ ] Unit-test source-preserve, contain, cover, custom and encoder quality arg plans.
- [ ] Implement ffprobe metadata parser.
- [ ] Implement cancellable child-process runner.
- [ ] Implement outro concat plan with normalized streams.
- [ ] Keep `.partial` strictly internal to incomplete media renders; never parse or expose it as a subtitle format.
- [ ] Support video-only ingest, video + `.srt/.ass/.json` paired ingest, and replacing/removing an attached sidecar before render.

### Task 6: Streaming transcription and LLM correction

**Files:** `src/media/transcribe.rs`, `src/subtitle/llm.rs`

- [ ] Stream WAV into Reqwest multipart instead of reading entire files into memory.
- [ ] Support OpenAI/Speaches and WhisperX request shapes.
- [ ] Use cancellation token during network work.
- [ ] Preserve line count and timing through optional LLM correction.

### Task 7: Persisted job manager and V1 automation parity

**Files:** `src/jobs.rs`, `src/api/jobs.rs`, `src/api/automation.rs`

- [ ] Add job transition/cancellation tests.
- [ ] Persist jobs and convert active states to interrupted on startup.
- [ ] Restore `/api/trigger`, job lookup, ASS retrieval and corrected re-burn.
- [ ] Restore real cancellation semantics.

### Task 8: Workflow supervisor and watcher safety

**Files:** `src/workflows.rs`, `src/api/workflows.rs`

- [ ] Add reconcile tests: create, edit, enable, disable, delete.
- [ ] Implement supervised watcher cancellation/restart.
- [ ] Add event dedupe, polling fallback and file stability checks.
- [ ] Restore `.ass/.srt/.json` companions, `_words.json`, keyword/name preset matching and archive-only-after-success.

### Task 9: Safe picker and assets

**Files:** `src/api/files.rs`, `src/api/assets.rs`

- [ ] Test traversal and symlink escapes against allowed roots.
- [ ] Implement directory/file/any selection modes and extension filters.
- [ ] Support existing-file selection plus upload for outros.

### Task 10: Brands and preset resolution

**Files:** `src/api/brands.rs`, `src/api/presets.rs`, `src/domain.rs`

- [ ] Test brand default preset lookup for every format.
- [ ] Test workflow explicit preset override vs brand default.
- [ ] Implement brand CRUD and preset assignment.

### Task 11: Axum API and static SvelteKit serving

**Files:** `src/api/mod.rs`, `src/error.rs`, `src/state.rs`, `src/main.rs`

- [ ] Add API smoke tests for health, structured errors and key routes.
- [ ] Mount `/api/v1` and compatibility aliases.
- [ ] Serve SvelteKit static output with SPA fallback and immutable asset caching.
- [ ] Keep CORS same-origin by default.

### Task 12: SvelteKit frontend foundation

**Files:** `frontend/package.json`, `frontend/svelte.config.js`, `frontend/vite.config.ts`, `frontend/src/**`

- [ ] Create SvelteKit static app using Svelte 5 runes and typed API client.
- [ ] Implement Queue, Brands, Presets, Workflows and Settings sections as focused components.
- [ ] Build one reusable PathPicker supporting files and folders.
- [ ] Implement aspect-correct FormatPreview for source and every explicit profile.
- [ ] Remove all React/Framer dependencies and generated build output from Git tracking.

### Task 13: Editor correctness

**Files:** `frontend/src/lib/components/SubtitleEditor.svelte`, `src/api/subtitles.rs`

- [ ] Editor saves through canonical Rust normalize endpoint.
- [ ] Add explicit regroup action using the Unicode/French engine.
- [ ] Preserve timings for text-only edits when token count/content allows it.
- [ ] Surface repaired overlap count/warnings rather than silently changing timestamps.

### Task 14: Docker, CI and maintenance

**Files:** `Dockerfile`, `compose.yaml`, `.github/workflows/ci.yml`, `.github/dependabot.yml`

- [ ] Multi-stage Node 24 + Rust 1.98 build.
- [ ] Minimal non-root FFmpeg runtime.
- [ ] CI gates: Svelte check/build; cargo fmt/clippy/test; Docker build.
- [ ] Dependabot for npm/Cargo/Actions/Docker.

### Task 15: Documentation

**Files:** `README.md`, `README.fr.md`

- [ ] English README with concept/features/API/Docker/workflows/brands.
- [ ] French README with equivalent operational content.
- [ ] Ensure every advertised behavior maps to code or an automated test.

### Task 16: Final verification

- [ ] `cargo fmt --check`.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] `cargo test --all`.
- [ ] `npm ci && npm run check && npm run build` in `frontend/`.
- [ ] clean Docker build and healthcheck.
- [ ] verify no `node_modules`, frontend build output, `target`, secrets or personal absolute paths are tracked.
