# Changelog

All notable released changes are documented here. AutoSubs follows semantic versioning for public releases.

## [Unreleased]

## [3.4.0] - 2026-10-07

### Added

- Optional validated forced-alignment stage for precision word boundaries, with timing provenance and safe fallback to native transcription timings.
- Authoritative FFmpeg/libass preview frames shared with the final render filter chain, plus a combined system/app font catalog and safe UI font import.
- Explicit subtitle shadow X/Y offset, blur, opacity and color controls with migration from legacy scalar shadows.
- Per-job Auto, Fast, Quality and Compact render profiles with resolved-encoder visibility and bounded ETA ranges learned from successful render history.
- Explicit visual line-break editing and constrained 10 ms word-boundary nudging without discarding canonical word timings.

### Changed

- Job/preset/brand resolution is snapshotted deterministically so preview, regrouping, exports and final render consume the same effective configuration.
- Preset continuous controls pair sliders with numeric inputs; settled previews are rendered by the backend while CSS remains transient interaction feedback only.
- App-managed fonts default to `/fonts` but the directory is configurable through `AUTOSUBS_FONTS_DIR`; writable mounts enable UI imports.
- Render fallback attempts keep profile quality settings, record the encoder that actually succeeded, and widen future ETA confidence without polluting successful throughput samples.

### Fixed

- Preview/final discrepancies in font, position, outline, shadow and format processing are eliminated by sharing the FFmpeg/libass visual filter builder.
- Manual line breaks and word-timing edits preserve timing invariants and remain bounded by adjacent words.

## [3.3.0] - 2026-10-02

### Added

- Server-side folder favorites persist in SQLite and appear as compact shortcuts in every server picker.
- Workflows can explicitly publish either **Video only** or **Video + SRT**; existing workflows migrate to Video + SRT.
- Workflow archival moves the source plus same-stem companion files as one rollback-safe bundle after successful publication.

### Added

- Runtime encoder benchmarks verify NVENC, QSV, VA-API, Vulkan and AMF with a 2160×3840 / 360-frame stress workload and a 20-second guard instead of trusting FFmpeg's compiled encoder list or a tiny one-frame probe.
- Auto H.264 ranks validated hardware backends by measured runtime; differences within 5% are treated as benchmark noise and use a stability-first tie-break, while materially faster backends (including Vulkan) win. Settings exposes the benchmark scores and selected order.
- VA-API probing records the best usable `/dev/dri/renderD*` device; Vulkan probing records the usable FFmpeg Vulkan device selector.
- The runtime image ships a reproducible 2026-09-29 Debian snapshot with FFmpeg 9.0.2 and Mesa VA-API/Vulkan 26.2.3 on all architectures; Intel media VA-API 26.2.4 is installed only on amd64 so multi-arch publication remains valid.

### Changed

- Standard social format choices are adaptive ratios rather than forced 1080p canvases: matching source ratios keep the original resolution, while ratio changes use the largest exact even canvas that fits within the source dimensions.
- Hardware encoder choices that fail the representative runtime benchmark are disabled in Settings; Auto now tries validated hardware backends in measured order before the universal libx264 fallback.
- Rust lockfile dependencies, Node 24 LTS, Svelte, Vite and the Svelte Vite plugin are updated to their current compatible September 2026 versions.

### Fixed

- French hyphen continuations such as `quand` + `-même` and `rendez-` + `vous` are reconstructed without stray spaces while speaker dashes remain separate.
- French segmentation never orphans hard-bound elisions or hyphenated compounds, while grammatical no-break preferences can relax only when line-width constraints require it.
- Live job/file UI synchronization now listens to the backend's named `job` SSE events correctly and refreshes an open server file picker as job state changes.
- Job errors retain the underlying error chain instead of collapsing to a generic top-level message such as `render video`.


## [3.2.0] - 2026-09-18

### Added

- New **Word by word** caption animation: only the currently spoken unit is displayed, using canonical word timings.
- Word-by-word grouping keeps French elisions and split compounds together (for example `l'` + `amour` and `rendez-` + `vous`) instead of flashing orphan fragments.
- Editor action to remove sentence-ending full stops in one pass while preserving commas, `!`, `?`, and ellipses, with a one-step undo before saving.

### Changed

- Preset and live video previews now mirror the word-by-word grouping used by the ASS/libass renderer.

## [3.1.2] - 2026-09-17

### Fixed

- OpenAI-compatible `/v1` provider URLs now resolve `/models` for discovery and `/audio/transcriptions` for transcription, while full transcription URLs remain accepted.
- Multipart asset uploads larger than Axum's default body limit are streamed correctly and remain capped by `AUTOSUBS_MAX_UPLOAD_BYTES`.

### Security

- Updated `rustls` to 0.23.45 and `chacha20` to 0.10.2 so the locked dependency set passes the current RustSec audit.

## [3.1.1] - 2026-09-02

### Added

- Rendered videos can be downloaded directly from the Files view.
- Every supported font file mounted in `/fonts` appears as an individual preset choice and is served to browser previews.
- Preview playback uses native video controls with play, pause, seeking, volume and fullscreen support.

### Changed

- Applying a preset now applies its output format, fit mode and subtitle segmentation limits to the job.
- Preview typography and subtitle sizing scale from the effective video canvas for closer parity with rendered output.
- The responsive interface has clearer actions and improved desktop, tablet and mobile layouts.

### Fixed

- Source-format previews preserve the exact source aspect ratio without stretching or black bars.
- French segmentation enforces one- or two-line limits, preserves word spacing and favors natural bottom-heavy line breaks.
- Successful job deletion no longer reports an empty-response error.

## [3.1.0] - 2026-09-01

### Added

- Custom fonts are discovered recursively from the fixed internal `/fonts` mount, exposed to browser previews, and available to libass.
- Canonical word timing is durable across regrouping and visual text edits.
- French-aware segmentation and hard `maxLines` enforcement keep rendered captions within their configured visual line limit.
- Editor actions restore Split, Merge previous/next, and Delete subtitle block.
- Existing jobs can be retranscribed or re-rendered; jobs can be deleted without removing source media or final output.

### Fixed

- Corrected Pop, Highlight, Bounce, Karaoke, Fade, Slide-up, and None animations to use consistent timed-word or block semantics in preview and ASS output.
- Source format with Preserve never adds black bars: the primary source geometry is not scaled, padded, or cropped.

## [3.0.1] - 2026-08-31

### Security

- Canonicalize and validate server-side paths before filesystem access.
- Reject path-component injection in managed asset and upload storage.
- Revalidate persisted workflow directories before watcher and render use.
- Harden brand outro resolution against traversal through persisted values.
- Replace privileged `workflow_run` image publishing with trusted `main` push validation.

## [3.0.0] - 2026-08-31

### Added

- Clean-room Rust/Axum + SvelteKit architecture.
- Persistent SQLite job/workflow/settings store with local-WAL guard.
- Resumable tus-style uploads and server-side file picker.
- Canonical subtitle normalization, Unicode/French segmentation and SRT/ASS/JSON interchange.
- Brands, per-format presets, independent watch-folder workflows and NFS reconciliation.
- FFmpeg/libass rendering with capability discovery, hardware selection/fallback and machine-readable progress.
- Responsive English/French UI for desktop, tablet and phone.
- Multi-architecture GHCR release pipeline with SBOM, provenance and registry attestations.
- Accurate Source, 9:16, 16:9, 1:1, 4:5 and custom output previews.
- Advisory Generic, TikTok, Reels and Shorts safe-zone guides.
- Pointer and keyboard subtitle-position editing in preset previews.

### Fixed

- Explicit output formats no longer retain the invalid `preserve` fit mode.
- Custom output dimensions are validated before rendering.
- Editor preview now follows contain, cover and stretch rendering semantics.
- ASS Slide Up animation no longer combines conflicting `\\pos` and `\\move` tags.
- ASS export now uses the effective job output format and source resolution.
- SSA sidecar selection is available consistently in the editor.
- Rendering no longer continues after subtitle or job-option persistence fails.
- Native WebVTT caption tracks are retained for preview accessibility.
