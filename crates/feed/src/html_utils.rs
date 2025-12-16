// ABOUTME: HTML utility functions for feed content processing.
// ABOUTME: Provides tag stripping and HTML entity decoding matching Go behavior.

use aho_corasick::AhoCorasick;
use once_cell::sync::Lazy;

/// Named HTML entities to decode (patterns for Aho-Corasick automaton).
const ENTITY_PATTERNS: &[&str] = &[
    "&amp;", "&lt;", "&gt;", "&quot;", "&apos;", "&#39;", "&nbsp;", "&ndash;", "&mdash;",
    "&lsquo;", "&rsquo;", "&ldquo;", "&rdquo;", "&hellip;", "&copy;", "&reg;", "&trade;", "&bull;",
    "&middot;", "&deg;", "&plusmn;", "&times;", "&divide;", "&frac12;", "&frac14;", "&frac34;",
    "&euro;", "&pound;", "&yen;", "&cent;",
];

/// Replacement strings for each entity (must match order of ENTITY_PATTERNS).
const ENTITY_REPLACEMENTS: &[&str] = &[
    "&", "<", ">", "\"", "'", "'", " ", "–", "—", "'", "'", "\u{201C}", "\u{201D}", "…", "©", "®",
    "™", "•", "·", "°", "±", "×", "÷", "½", "¼", "¾", "€", "£", "¥", "¢",
];

/// Aho-Corasick automaton for single-pass HTML entity replacement.
/// Compiled once at first use, then reused for all subsequent calls.
static ENTITY_MATCHER: Lazy<AhoCorasick> =
    Lazy::new(|| AhoCorasick::new(ENTITY_PATTERNS).expect("failed to build entity matcher"));

/// Strips HTML tags from a string, returning plain text.
/// This is a naive implementation that removes angle-bracketed content.
pub fn strip_html(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;

    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Decode entities after stripping tags
    let decoded = decode_entities(&result);

    // Collapse whitespace (multiple spaces/newlines to single space)
    collapse_whitespace(&decoded)
}

/// Decodes common HTML entities to their character equivalents.
/// Uses Aho-Corasick for O(n) single-pass named entity replacement,
/// then handles numeric entities in a second pass.
pub fn decode_entities(s: &str) -> String {
    // Fast path: no entity markers at all
    if !s.contains('&') {
        return s.to_string();
    }

    // Single-pass named entity replacement using Aho-Corasick automaton
    let named_decoded = ENTITY_MATCHER.replace_all(s, ENTITY_REPLACEMENTS);

    // Handle numeric entities (decimal and hex) - already O(n)
    decode_numeric_entities(&named_decoded)
}

/// Decodes numeric HTML entities like &#123; and &#x7B;
fn decode_numeric_entities(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '&' && chars.peek() == Some(&'#') {
            chars.next(); // consume '#'
            let mut num_str = String::new();
            let is_hex = chars.peek() == Some(&'x') || chars.peek() == Some(&'X');

            if is_hex {
                chars.next(); // consume 'x' or 'X'
            }

            while let Some(&nc) = chars.peek() {
                if nc == ';' {
                    chars.next(); // consume ';'
                    break;
                }
                if is_hex && nc.is_ascii_hexdigit() {
                    num_str.push(chars.next().unwrap());
                } else if !is_hex && nc.is_ascii_digit() {
                    num_str.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            if !num_str.is_empty() {
                let code = if is_hex {
                    u32::from_str_radix(&num_str, 16).ok()
                } else {
                    num_str.parse::<u32>().ok()
                };

                if let Some(code) = code {
                    if let Some(decoded_char) = char::from_u32(code) {
                        result.push(decoded_char);
                        continue;
                    }
                }
            }

            // Failed to decode, push original
            result.push('&');
            result.push('#');
            if is_hex {
                result.push('x');
            }
            result.push_str(&num_str);
        } else {
            result.push(c);
        }
    }

    result
}

/// Collapses multiple whitespace characters into single spaces.
fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut last_was_space = false;

    for c in s.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(c);
            last_was_space = false;
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_basic() {
        assert_eq!(strip_html("<p>Hello</p>"), "Hello");
        assert_eq!(
            strip_html("<b>Bold</b> and <i>italic</i>"),
            "Bold and italic"
        );
    }

    #[test]
    fn test_strip_html_with_entities() {
        assert_eq!(strip_html("<p>Tom &amp; Jerry</p>"), "Tom & Jerry");
        assert_eq!(strip_html("&lt;script&gt;"), "<script>");
    }

    #[test]
    fn test_strip_html_whitespace_collapse() {
        assert_eq!(strip_html("<p>Hello</p>\n\n<p>World</p>"), "Hello World");
        assert_eq!(strip_html("Multiple   spaces"), "Multiple spaces");
    }

    #[test]
    fn test_decode_entities_named() {
        assert_eq!(decode_entities("&amp;"), "&");
        assert_eq!(decode_entities("&lt;&gt;"), "<>");
        assert_eq!(decode_entities("&quot;test&quot;"), "\"test\"");
        assert_eq!(decode_entities("&nbsp;"), " ");
        assert_eq!(decode_entities("&mdash;"), "—");
    }

    #[test]
    fn test_decode_entities_numeric() {
        assert_eq!(decode_entities("&#38;"), "&");
        assert_eq!(decode_entities("&#x26;"), "&");
        assert_eq!(decode_entities("&#169;"), "©");
        assert_eq!(decode_entities("&#xA9;"), "©");
    }

    #[test]
    fn test_decode_entities_mixed() {
        assert_eq!(decode_entities("&amp;&#38;&lt;"), "&&<");
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(strip_html(""), "");
        assert_eq!(decode_entities(""), "");
    }
}
