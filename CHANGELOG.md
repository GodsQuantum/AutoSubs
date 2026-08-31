# Changelog

All notable released changes are documented here. AutoSubs follows semantic versioning for public releases.

## [Unreleased]

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
