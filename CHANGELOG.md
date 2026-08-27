# Changelog

All notable released changes are documented here. AutoSubs follows semantic versioning for public releases.

## [Unreleased]

### Added

- Clean-room Rust/Axum + SvelteKit architecture.
- Persistent SQLite job/workflow/settings store with local-WAL guard.
- Resumable tus-style uploads and server-side file picker.
- Canonical subtitle normalization, Unicode/French segmentation and SRT/ASS/JSON interchange.
- Brands, per-format presets, independent watch-folder workflows and NFS reconciliation.
- FFmpeg/libass rendering with capability discovery, hardware selection/fallback and machine-readable progress.
- Responsive English/French UI for desktop, tablet and phone.
- Multi-architecture GHCR release pipeline with SBOM/provenance.

The first clean-room release will be tagged `v3.0.0` after CI validation.
