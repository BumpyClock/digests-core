## Project purpose
- Rust workspace "digests-core" providing shared parsing/extraction primitives plus C ABI for multi-platform apps.
- Crates: feed (RSS/Atom/podcast parsing), hermes (ReaderView/article extraction port of Go Hermes), ffi (C ABI for feed/reader/meta arenas), cli (developer CLI wrappers).

## Tech stack
- Rust 2021 workspace, primary deps: reqwest+tokio, scraper/ego-tree for DOM, ammonia for sanitization, htmd for markdown/text, serde/serde_json, regex, chrono/dateparser, bump arenas in ffi, clap for CLI.
- FFI via cbindgen config `cbindgen.toml`.

## Codebase layout (top level)
- `crates/feed`: feed parser + tests.
- `crates/hermes`: ReaderView extraction, DOM cleaners/transforms, CLI binary `hermes`.
- `crates/ffi`: exposes C ABI for feed/reader/meta arenas.
- `crates/cli`: small Rust CLI for feed parsing.
- `output1.md`: sample output produced by Rust hermes.

## Build/test basics
- Workspace managed via Cargo; `Cargo.toml` lists members feed/ffi/cli/hermes.
- Default encoding UTF-8 per project config.

## Notable docs
- `.ai_agents/remaining_hermes.md` lists parity gaps vs Go Hermes.
- `.ai_agents/porting_plan.md` describes feed port plan.
- `.ai_agents/structs.md` defines stable FFI structs for feed/reader/meta.
- Root README gives FFI usage and build commands.