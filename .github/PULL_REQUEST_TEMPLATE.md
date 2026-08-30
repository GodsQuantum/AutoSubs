## What changed

Describe the user-visible or technical change.

## Verification

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo test --all --locked`
- [ ] `cd frontend && npm ci && npm test && npm run check && npm run build`
- [ ] Docker build / relevant media smoke test

## Media behavior

If this changes transcription, subtitle timing, rendering, FFmpeg arguments, uploads, or workflows, describe the regression case and expected result.
