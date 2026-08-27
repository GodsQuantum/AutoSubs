# Contributing

AutoSubs is intentionally opinionated: one Rust backend owns media/subtitle rules, and the SvelteKit UI stays a client of that API. Changes should preserve that boundary.

## Before opening a PR

1. Open an issue first for large behavioral changes.
2. Keep generated output, sample media, credentials and personal filesystem paths out of the repository.
3. Add or update tests for behavior changes.
4. Run the full verification set:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all

cd frontend
npm ci
npm test
npm run check
npm run build

cd ..
docker build --pull -t autosubs:dev .
```

## Media invariants

Changes to rendering, workflows or file handling must preserve these rules:

- processing failure never archives or deletes the source;
- `.partial` is internal render staging, never a subtitle interchange format;
- source geometry is preserved unless an explicit output profile requests otherwise;
- subtitle normalization lives in Rust, not in a second browser implementation;
- filesystem paths are canonicalized against configured allowed roots;
- SQLite `/config` stays local and is not supported on NFS/CIFS/SSHFS.

## Style

- Rust: `rustfmt`, no Clippy warnings.
- Svelte/TypeScript: keep components focused; user-visible copy must exist in both EN and FR dictionaries.
- API: new application routes belong under `/api/v1`.
- Commits: short imperative/conventional messages are preferred.

By contributing, you agree that your contribution is licensed under the repository's MIT license.
