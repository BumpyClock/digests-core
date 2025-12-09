## Coding conventions
- Follow ~/.claude/docs/writing-code.md: prefer simple/maintainable solutions; minimal changes; never rewrite wholesale without permission; preserve comments; no mock modes; avoid 'new/improved' naming; plan before coding; never bypass tests.
- All code files start with two ABOUTME lines (`// ABOUTME: ...`) describing the file (observed in Rust sources).
- Match surrounding style/formatting; keep comments evergreen (no temporal phrasing).
- Use ASCII unless file already uses non-ASCII.

## Project-specific norms
- Rust workspace uses 2021 edition; public APIs re-exported in `crates/hermes/src/lib.rs`.
- CLI/binary uses clap; errors via anyhow/thiserror; tests use pretty_assertions/httpmock/assert_cmd.
- Avoid mock implementations; use real data.

## Directory practices
- `.ai_agents` contains planning/parity docs; read before parity work.
- FFI structs stable per `.ai_agents/structs.md`; avoid breaking ABI without coordination.