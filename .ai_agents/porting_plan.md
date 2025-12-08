# Feed Parsing Port Plan (digests-api → Rust core)

## Scope (phase 1)
- Deliver Rust feed parser that produces the stable ABI structs in `.ai_agents/structs.md`.
- Inputs: already-fetched feed bytes + feed URL.
- Outputs: `DFeed` + items with media/thumbnails, language, timings, iTunes metadata.
- Out of scope for now: Hermes ReaderView port, metadata-only extractor, HTTP fetching/caching, category enrichment, storage, hash/refresh logic.

## Source-of-truth behaviors from digests-api (Go)
- Parser: `gofeed` default parser (`core/feed/service.go`).
- There is a gofeed alternative crate for Rust. it's called feed-rs
- Feed fields:
  - `Title`, `Description`, `Link` → map to feed title/description/home URL.
  - `URL` preserved as feed_url.
  - Language from feed.Language.
  - Image: feed.Image or iTunes image; Subtitle from iTunes.
  - Categories: joined string; not present in FFI feed struct (item categories only).
  - Dates: `UpdatedParsed` > `PublishedParsed` > now; published uses `PublishedParsed` or `ParseFlexibleTime`.
  - FeedType detection: podcast if feed.ITunesExt present, else majority of first 5 items having iTunes or audio/video enclosure; otherwise article.
- Item fields:
  - ID: GUID, fallback to Link.
  - Published: `PublishedParsed` else `ParseFlexibleTime(Published)`; Created mirrors Published.
  - Author precedence: iTunes author > item.Author.Name.
  - Content: if Content present, `Content` (plain) = `StripHTML`, `ContentEncoded` = raw Content; else fall back to Description similarly.
  - Categories: passthrough array.
  - Enclosures: URL, Length, Type copied; no size parsing.
  - Primary media URL: prefer audio enclosures by priority (`audio/mpeg`, `audio/mp3`, `audio/mp4`, `audio/aac`), else first enclosure.
  - Thumbnail selection priority: (1) iTunes image; (2) image enclosure; (3) item.Image; (4) first `<img>` in Content then Description (normalized to absolute); no feed-level fallback.
  - iTunes extras: duration parsed to seconds string; episode, season, episodeType, subtitle, summary, image.
- Utilities to replicate:
  - `StripHTML` (tag stripping + entity decode, naive).
  - `ParseFlexibleTime` with multiple RSS/Atom formats.
  - `duration.ParseToSeconds` supporting seconds int, Go duration strings, HH:MM:SS, MM:SS.
  - `ExtractFirstImage` using goquery-like traversal; `isValidImageURL` skips tracking pixels, 1x1 gifs, spacer patterns; supports relative URL resolution against item link.

## Target Rust design (feed-only slice)
- Workspace crates (initial):
  - `crates/feed` – pure Rust parser returning internal structs mirroring ABI.
  - `crates/ffi` – C ABI over arenas for feeds; exposes `digests_parse_feed`, `digests_feed_result`, `digests_free_feed`.
  - (ReaderView/meta crates stubbed later).
- Dependencies:
  - `feed-rs` (or `atom_syndication` + `rss` if needed) for parsing.
  - `scraper` + `kuchiki`/`lol-html` or `markup5ever` for HTML image scraping; `url` for resolution.
  - `chrono` for time, custom flexible parser table matching Go formats.
  - `bumpalo` or `typed-arena` for arena allocations in FFI layer.
- Data mapping to ABI:
  - `DFeed.title` = feed.Title; `home_url` = feed.Link; `feed_url` = input URL.
  - `published_ms`/`updated_ms` from parsed dates (ms since epoch, 0 if unknown).
  - Items: map plain content to `summary`, encoded HTML to `content`; `guid` = ID; `primary_media_url` & `thumbnail_url` from heuristics above; `feed_type` set per detection; duration → `duration_seconds` (u32) parsed from string; explicit flag currently unset in Go—consider populate from iTunes explicit if present (behavioral delta to call out).
  - Categories array from item categories; enclosures mapped 1:1; author name/email when available.

## Implementation steps (ordered)
1) Scaffold Rust workspace with `crates/feed` and `crates/ffi`; add `cbindgen` config (header path `dist/digests.h`).
2) Implement flexible time parser covering Go formats; add tests with sample strings.
3) Implement duration parsing equivalent to `ParseToSeconds`; store as u32 seconds.
4) Port HTML helpers: simple strip-html (for summary), entity decode, first-image extraction with validity filter + absolute URL resolution.
5) Feed parser adapter:
   - Parse with `feed-rs` (or fallback per feed type) from bytes.
   - Build internal Feed/Item structs applying the Go precedence rules above.
   - Implement feed_type detection heuristic identical to Go (ITunes flag or majority audio/iTunes in first 5 items).
   - Media/thumbnail selection logic replicated.
6) FFI layer:
   - Arena allocator per call; expose C ABI matching `.ai_agents/structs.md`.
   - Convert strings to `DString` slices into arena; enforce version constant.
7) Tests:
   - Golden tests generated from a small set of public feeds (podcast + article) and sample fixtures from digests-api outputs once captured.
   - Unit tests for time/duration parsing, thumbnail selection, media selection, feed_type detection.
8) Docs: add README in `crates/feed` and update this plan with any behavioral deltas discovered during implementation.

## Open points / deltas to flag
- Go code does not set explicit flag; ABI has `explicit_flag`. Proposal: set from iTunes `Explicit` when present (behavior change—acceptable per “structs stable, behavior can diverge”). Confirm before implementing.
- Need fixture corpus: pull a handful of feeds used in production (podcast + article) to lock outputs.
- Error surface: Rust should mirror Go’s error buckets as `DErrorCode` (parse vs invalid vs timeout handled upstream); define mapping.

## Next action
- Complete fixture capture from `digests-api` (feed inputs + current JSON outputs) to lock expectations, then start workspace scaffolding in `digests-core`.
