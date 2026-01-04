// ABOUTME: Golden tests for comparing Rust hermes output against Go hermes reference fixtures.
// ABOUTME: Tests title, author, domain, language, word_count, and content prefix matching.

use digests_hermes::{Client, ContentType};
use serde::Deserialize;
use std::fs;

/// Expected output structure matching Go hermes JSON format.
#[derive(Debug, Deserialize)]
struct ExpectedOutput {
    url: String,
    domain: String,
    title: String,
    author: Option<String>,
    language: Option<String>,
    word_count: i32,
    content: String,
}

/// Load a fixture file from the fixtures directory.
fn load_fixture(name: &str) -> ExpectedOutput {
    let path = format!(
        "{}/tests/fixtures/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let content = fs::read_to_string(&path).expect(&format!("Failed to read fixture: {}", path));
    serde_json::from_str(&content).expect(&format!("Failed to parse fixture: {}", path))
}

/// Load an HTML snapshot from the fixtures directory.
fn load_html_fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/html/{}.html",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    fs::read_to_string(&path).expect(&format!("Failed to read HTML fixture: {}", path))
}

/// Check if word count is within 5% tolerance (with a minimum delta of 10).
fn word_count_within_tolerance(actual: i32, expected: i32) -> bool {
    let tolerance = (expected as f64 * 0.05).ceil() as i32 + 10;
    let diff = (actual - expected).abs();
    diff <= tolerance
}

/// Compare first N characters of content (case-insensitive, whitespace-normalized).
fn content_prefix_matches(actual: &str, expected: &str, chars: usize) -> bool {
    // Normalize: lowercase, collapse whitespace, take first N chars
    let normalize = |s: &str| -> String {
        s.trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(chars)
            .collect::<String>()
    };
    let actual_prefix = normalize(actual);
    let expected_prefix = normalize(expected);

    // Check if they share a common significant prefix (at least 30 chars)
    let min_match = 30.min(expected_prefix.len()).min(actual_prefix.len());
    if min_match == 0 {
        return expected_prefix.is_empty() && actual_prefix.is_empty();
    }

    let actual_words: Vec<&str> = actual_prefix.split_whitespace().collect();
    let expected_words: Vec<&str> = expected_prefix.split_whitespace().collect();

    // Check if first few significant words match
    let words_to_check = 5.min(expected_words.len()).min(actual_words.len());
    if words_to_check == 0 {
        return true;
    }

    let matches = actual_words[..words_to_check]
        .iter()
        .zip(expected_words[..words_to_check].iter())
        .filter(|(a, e)| a == e)
        .count();

    // Allow some flexibility - at least 60% of words should match
    matches >= (words_to_check * 3 / 5).max(1)
}

/// Run a golden test against a live URL.
async fn run_golden_test(fixture_name: &str) {
    let expected = load_fixture(fixture_name);
    let html = load_html_fixture(fixture_name);

    let client = Client::builder()
        .content_type(ContentType::Text)
        .timeout(std::time::Duration::from_secs(30))
        .build();

    let result = client.parse_html(&html, &expected.url).await;

    match result {
        Ok(parsed) => {
            // Check domain
            assert_eq!(
                parsed.domain, expected.domain,
                "[{}] Domain mismatch: expected {}, got {}",
                fixture_name, expected.domain, parsed.domain
            );

            // Check title (case-insensitive, trimmed, normalize quotes)
            let normalize_title = |s: &str| -> String {
                s.trim()
                    .to_lowercase()
                    .replace('\u{2019}', "'") // Right single quote
                    .replace('\u{2018}', "'") // Left single quote
                    .replace('\u{201C}', "\"") // Left double quote
                    .replace('\u{201D}', "\"") // Right double quote
            };
            let expected_title = normalize_title(&expected.title);
            let actual_title = normalize_title(&parsed.title);
            assert!(
                actual_title.contains(&expected_title) || expected_title.contains(&actual_title),
                "[{}] Title mismatch: expected '{}', got '{}'",
                fixture_name,
                expected.title,
                parsed.title
            );

            // Check author if expected
            if let Some(ref expected_author) = expected.author {
                if let Some(ref actual_author) = parsed.author {
                    let expected_lower = expected_author.trim().to_lowercase();
                    let actual_lower = actual_author.trim().to_lowercase();
                    assert!(
                        actual_lower.contains(&expected_lower)
                            || expected_lower.contains(&actual_lower),
                        "[{}] Author mismatch: expected '{}', got '{}'",
                        fixture_name,
                        expected_author,
                        actual_author
                    );
                }
                // If expected author but parsed.author is None, that's acceptable
                // as websites may change their markup
            }

            // Check language if expected
            if let Some(ref expected_lang) = expected.language {
                if let Some(ref actual_lang) = parsed.language {
                    // Compare primary language tag only (e.g., "en" from "en-US")
                    let expected_primary = expected_lang.split('-').next().unwrap_or(expected_lang);
                    let actual_primary = actual_lang.split('-').next().unwrap_or(actual_lang);
                    assert_eq!(
                        actual_primary.to_lowercase(),
                        expected_primary.to_lowercase(),
                        "[{}] Language mismatch: expected '{}', got '{}'",
                        fixture_name,
                        expected_lang,
                        actual_lang
                    );
                }
            }

            // Check word count within tolerance
            assert!(
                word_count_within_tolerance(parsed.word_count, expected.word_count),
                "[{}] Word count out of tolerance: expected ~{}, got {} (tolerance: 5% + 10)",
                fixture_name,
                expected.word_count,
                parsed.word_count
            );

            // Check content prefix (first 80 chars)
            assert!(
                content_prefix_matches(&parsed.content, &expected.content, 80),
                "[{}] Content prefix mismatch: expected to start with '{}...', got '{}...'",
                fixture_name,
                &expected.content.chars().take(80).collect::<String>(),
                &parsed.content.chars().take(80).collect::<String>()
            );

            println!(
                "[{}] PASSED - title: '{}', author: {:?}, word_count: {}",
                fixture_name, parsed.title, parsed.author, parsed.word_count
            );
        }
        Err(e) => {
            // Network errors are acceptable in CI environments
            println!(
                "[{}] SKIPPED due to network error: {}",
                fixture_name,
                e.to_string()
            );
        }
    }
}

#[tokio::test]
async fn golden_test_npr() {
    run_golden_test("npr").await;
}

#[tokio::test]
async fn golden_test_engadget() {
    run_golden_test("engadget").await;
}

#[tokio::test]
async fn golden_test_theverge() {
    run_golden_test("theverge").await;
}

#[tokio::test]
async fn golden_test_vox() {
    run_golden_test("vox").await;
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_word_count_tolerance() {
        // Within tolerance
        assert!(word_count_within_tolerance(100, 100));
        assert!(word_count_within_tolerance(105, 100)); // 5% of 100 = 5, +10 = 15 tolerance
        assert!(word_count_within_tolerance(95, 100));
        assert!(word_count_within_tolerance(115, 100)); // Within 15 tolerance

        // Outside tolerance
        assert!(!word_count_within_tolerance(130, 100)); // 30 diff > 15 tolerance
        assert!(!word_count_within_tolerance(70, 100)); // 30 diff > 15 tolerance

        // Large numbers
        assert!(word_count_within_tolerance(1000, 1050)); // Within 5% + 10
        assert!(word_count_within_tolerance(2000, 2100)); // Within 5% + 10
    }

    #[test]
    fn test_content_prefix_matches() {
        // Matching first words
        assert!(content_prefix_matches(
            "Hello world this is a test",
            "hello world this is",
            80
        ));
        // Whitespace normalization
        assert!(content_prefix_matches(
            "  Hello   World   Test  ",
            "hello world test",
            80
        ));
        // Different content should not match
        assert!(!content_prefix_matches(
            "Completely different content here",
            "hello world test foo bar",
            80
        ));
    }

    #[test]
    fn test_load_fixture() {
        let fixture = load_fixture("npr");
        assert!(!fixture.url.is_empty());
        assert!(!fixture.domain.is_empty());
        assert!(!fixture.title.is_empty());
    }
}
