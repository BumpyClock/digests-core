# Remaining Hermes Rust Parity Tasks

## High-priority parity gaps
- ✅ Port Go readability scorer and sibling merge (`internal/utils/dom/score_content.go`, `findTopCandidate`, `mergeSiblings`) exactly.
- ✅ Port Go cleaners (`clean.go`, `clean_h_ones.go`, `rewrite_top_level.go`, etc.) including negative/positive regex filters, link density penalties, sibling merge steps (implemented in `dom/cleaners.rs` + `dom/brs.rs`).
- ☐ Implement all Go FunctionTransform cases (partially done: noscript→span, reddit role=img→img, Gawker/YouTube lazy iframes, LATimes trb_ar_la, NatGeo lead images, CNN paragraphs/video thumb, apttherapy unwrap, deadline twitter embed, abendblatt unwrap, data-src/srcset fixes).
- ✅ Match Bluemonday sanitization policy (allowed tags/attrs/protocols) with ammonia config.
- ☐ Match turndown/markdown & text conversion rules used in Go (lists/code/link formatting).
- ☐ Date parsing: align with go-dateparser locale/relative parsing.
- ☐ Pagination: mirror collect-all-pages merge rules; propagate next_page_url from second page as Go does.
- ☐ SSRF: confirm per-hop DNS resolution matches Go behavior (reqwest redirect policy vs Go).

## Validation
- ☐ Rebuild golden fixtures from Go hermes for a broader set (including provided NPR/Engadget/Verge/Vox and additional RTL/video/pagination cases).
- ☐ Diff Rust vs Go outputs: title/author/domain/lang, word_count tolerance, leading content prefix, video/next_page fields.
- ☐ Add benchmarks against Go (throughput/memory).

## Cleanup / polish
- ✅ Remove deprecated `cargo_bin` warning in CLI tests.
- ☐ Optional FFI (cbindgen) if Go callers need Rust engine.
- ☐ Performance profiling after parity changes.

## Suggested execution order
1) Port readability + cleaners + transforms exactly from Go; rerun golden tests.
2) Align sanitization/turndown/text rules; rebuild golden fixtures.
3) Date parsing alignment; rerun golden tests.
4) Pagination merge parity; rerun golden tests.
5) Benchmarks/FFI/CLI polish.
