# Changelog

All notable released changes are documented here. AutoSubs follows semantic versioning for public releases.

## [Unreleased]

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
