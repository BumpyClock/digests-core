## When finishing a task
- Run `cargo test -q` (or targeted crate tests) to ensure no regressions.
- If working on Hermes parity, regenerate and compare outputs against Go fixtures/golden files when available.
- Keep notes of behavioral deltas vs Go; do not break FFI structs.
- Summarize changes and remaining risks for the user; suggest next steps if applicable.
- Do not commit with `--no-verify`; ensure comments and ABOUTME headers remain intact.