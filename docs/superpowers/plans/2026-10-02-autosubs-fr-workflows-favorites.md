# AutoSubs FR + Workflow + Favorites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship French segmentation fixes, workflow artifact control, bundle archiving, persistent picker favorites, storage mapping, dependency cleanup, and a verified production deployment.

**Architecture:** Keep subtitle intelligence in Rust, add one compatible workflow output enum, archive related files by safe stem-boundary matching, persist favorites in SQLite singleton storage, and simplify the Svelte picker. Reuse current Docker/LXC storage boundaries.

**Tech Stack:** Rust 2024, Axum, SQLite/rusqlite, Svelte 5, SvelteKit 2, Vite 8, FFmpeg 9, Docker.

**Spec:** `docs/superpowers/specs/2026-10-02-autosubs-fr-workflows-favorites-design.md`

## Global Constraints
- No large NLP dependency.
- Existing workflows default to video + SRT.
- No archive before successful publish.
- Favorites must remain inside allowed roots.
- Production-active remains writable; source libraries are read-only.
- Preserve manual SRT/ASS/JSON export behavior.

## Review Focus
- Hyphen prefix/suffix tokenization and speaker dashes.
- Impossible-width French bound pairs.
- Workflow migration from JSON without the new field.
- Prefix collisions such as `clip` vs `clip2`.
- Deleted/unmounted favorites and roots.
### Task 1: French segmentation
- [ ] Add failing tests for `quand` + `-même`, `rendez-` + `vous`, bound-pair overflow and auxiliaries.
- [ ] Implement one hyphen-continuation rule shared by token fixing, spacing and boundary protection.
- [ ] Make overflow fallback refuse linguistically bound boundaries.
- [ ] Run subtitle tests and full Rust suite.

### Task 2: Workflow artifacts and bundle archive
- [ ] Add failing serialization/output-policy tests.
- [ ] Add `WorkflowOutput` with compatible default.
- [ ] Publish only selected artifacts for workflow renders.
- [ ] Add failing bundle-match tests and implement safe bundle collection/archive.
- [ ] Run job/workflow tests and full Rust suite.

### Task 3: Persistent picker favorites
- [ ] Add failing backend favorite validation tests and frontend helper tests.
- [ ] Add GET/PUT favorite API backed by SQLite singleton storage.
- [ ] Add favorite toggle, favorite shortcuts and compact root selector to PathPicker.
- [ ] Run frontend tests/check/build and Rust API tests.

### Task 4: Runtime/storage/dependency polish
- [ ] Map deployment media libraries into AutoSubs with source libraries read-only.
- [ ] Update version/toolchain/dependencies only where verified.
- [ ] Remove proven-unused direct dependencies.
- [ ] Update README/CHANGELOG/handoff.

### Task 5: Verify, integrate and deploy
- [ ] Run fmt, clippy, Rust tests, frontend tests/check/build, audits and container build/smoke.
- [ ] Commit feature branch, review diff, merge to main, push GitHub.
- [ ] Build/pull production image, recreate AutoSubs, verify health/capabilities/favorites/workflows.
- [ ] Remove worktree/build artifacts and update authoritative handoff.
