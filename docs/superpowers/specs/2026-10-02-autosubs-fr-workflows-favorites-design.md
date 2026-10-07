# AutoSubs — French segmentation, workflows, archives and picker favorites

## Goal
Make AutoSubs robust for French short-form production: no stray spaces around hyphenated compounds, no orphaned French function words at caption boundaries, explicit workflow output artifacts, bundle archiving after success, persistent folder favorites, and clean access to mounted media storage.

## Constraints
- Existing manual editor exports remain available.
- Existing workflows migrate compatibly to video + SRT.
- Workflow output choice is only video, or video + SRT.
- Source bundles archive only after successful publish.
- Bundle matching must not turn `clip.mp4` into a match for `clip2.mp4`.
- Favorites are server-side persistent and restricted to allowed roots.
- Picker shows favorites prominently and collapses many roots into a compact storage selector.
- No large NLP dependency; extend the current Rust segmentation engine.
- Existing mounts and paths are not renamed; production-active stays RW, source libraries stay RO.
- Keep AutoSubs resource usage low and update only dependencies/toolchain justified by verification.

## Storage
Expose production-active, production-sources, cloud-arezki, cloud-family, cloud-fella, cloud-apps, media, and downloads. Backup-local remains excluded because it is backup infrastructure, not a production source library.

## Verification
Use RED→GREEN regression tests, full Rust/frontend suites, clippy/fmt, frontend check/build, container build/smoke, live workflow/picker checks, and post-deploy health/resource verification.
