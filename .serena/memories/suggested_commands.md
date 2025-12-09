## Core commands
- `cargo test -q` – run all workspace tests.
- `cargo build -p digests-ffi --release` – build FFI library.
- `cargo run -p digests-hermes --bin hermes -- <url>` – run Hermes reader on URL (formats: `-f html|markdown|text`, `-o output.md`, `--follow-next`).
- `./target/debug/hermes ...` – use built Hermes binary directly.

## File inspection
- `rg <pattern>` – fast search; `rg --files` to list.

## Environment
- Workspace root: /Users/adityasharma/Projects/digests-core; sandbox full access.

## Docs/refs
- `.ai_agents/remaining_hermes.md` for parity TODOs; `.ai_agents/structs.md` for ABI schema; `.ai_agents/porting_plan.md` feed plan.