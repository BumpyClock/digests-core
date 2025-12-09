// ABOUTME: Go-compatible BR to paragraph conversion.
// ABOUTME: Ports BrsToPs from Go hermes internal/utils/dom/brs.go

use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};

// Match two or more consecutive <br> tags (any capitalization, optional whitespace)
#[allow(dead_code)]
static DOUBLE_BR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:<br[^>]*>\s*){2,}").unwrap());

#[allow(dead_code)]
fn has_double_br(html: &str) -> bool {
    DOUBLE_BR_RE.is_match(html)
}

/// Convert consecutive BR tags to paragraphs
/// Matches Go's BrsToPs behavior
pub fn brs_to_ps(html: &str) -> String {
    if !DOUBLE_BR_RE.is_match(html) {
        return html.to_string();
    }

    fn append_chunk(out: &mut String, chunk: &str) {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            return;
        }
        let lower = trimmed.to_lowercase();
        let is_block_start = lower.starts_with("<p")
            || lower.starts_with("<div")
            || lower.starts_with("<article")
            || lower.starts_with("<section")
            || lower.starts_with("<h1")
            || lower.starts_with("<h2")
            || lower.starts_with("<h3")
            || lower.starts_with("<h4")
            || lower.starts_with("<h5")
            || lower.starts_with("<h6")
            || lower.starts_with("<ul")
            || lower.starts_with("<ol")
            || lower.starts_with("<blockquote");
        let is_block_end = lower.ends_with("</p>")
            || lower.ends_with("</div>")
            || lower.ends_with("</article>")
            || lower.ends_with("</section>")
            || lower.ends_with("</ul>")
            || lower.ends_with("</ol>")
            || lower.ends_with("</blockquote>");

        if !out.is_empty() {
            out.push('\n');
        }

        if is_block_start || is_block_end {
            out.push_str(trimmed);
        } else {
            out.push_str("<p>");
            out.push_str(trimmed);
            out.push_str("</p>");
        }
    }

    let mut output = String::new();
    let mut last_end = 0;
    for mat in DOUBLE_BR_RE.find_iter(html) {
        let before = &html[last_end..mat.start()];
        append_chunk(&mut output, before);
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("<p> </p>");
        last_end = mat.end();
    }
    let tail = &html[last_end..];
    append_chunk(&mut output, tail);

    output
}

/// Rewrite top-level body/html elements to divs
/// Matches Go's RewriteTopLevel behavior
pub fn rewrite_top_level(html: &str) -> String {
    // Parse and check if root is html or body
    let doc = Html::parse_document(html);

    let html_sel = Selector::parse("html").ok();
    let body_sel = Selector::parse("body").ok();

    let mut content = html.to_string();

    // If we have a bare html element, convert to div
    if let Some(ref sel) = html_sel {
        if doc.select(sel).next().is_some() {
            // Replace <html with <div and </html> with </div>
            content = content
                .replace("<html", "<div")
                .replace("</html>", "</div>");
            content = content
                .replace("<HTML", "<div")
                .replace("</HTML>", "</div>");
        }
    }

    // If we have a bare body element, convert to div
    if let Some(ref sel) = body_sel {
        if doc.select(sel).next().is_some() {
            content = content
                .replace("<body", "<div")
                .replace("</body>", "</div>");
            content = content
                .replace("<BODY", "<div")
                .replace("</BODY>", "</div>");
        }
    }

    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_double_br() {
        assert!(has_double_br("<br><br>"));
        assert!(has_double_br("text<br/><br/>more"));
        assert!(has_double_br("text<br />\n<br />more"));
        assert!(!has_double_br("<br>single"));
        assert!(!has_double_br("no breaks here"));
    }

    #[test]
    fn test_brs_to_ps_basic() {
        let input = "First paragraph<br><br>Second paragraph";
        let output = brs_to_ps(input);

        assert!(output.contains("<p>First paragraph</p>"));
        assert!(output.contains("<p>Second paragraph</p>"));
    }

    #[test]
    fn test_brs_to_ps_with_whitespace() {
        let input = "First paragraph<br>\n<br>Second paragraph";
        let output = brs_to_ps(input);

        assert!(output.contains("<p>"));
    }

    #[test]
    fn test_brs_to_ps_preserves_blocks() {
        let input = "<div>Already block</div><br><br><p>Paragraph</p>";
        let output = brs_to_ps(input);

        // Should not double-wrap block elements
        assert!(output.contains("<div>Already block</div>"));
        assert!(output.contains("<p>Paragraph</p>"));
        assert!(!output.contains("<p><div>"));
    }

    #[test]
    fn test_brs_to_ps_no_double_br() {
        let input = "Single<br>break only";
        let output = brs_to_ps(input);

        // Should return unchanged
        assert_eq!(output, input);
    }

    #[test]
    fn test_brs_to_ps_creates_empty_paragraph_from_double_break() {
        let input = r#"<div class="article adbox"><br /><br /><p>Ooo good one</p></div>"#;
        let output = brs_to_ps(input);

        assert!(output.contains("<p> </p>"));
        assert!(output.contains("<p>Ooo good one</p>"));
    }

    #[test]
    fn test_brs_to_ps_splits_inline_text_after_double_break() {
        let input = "<p>Here is some text<br /><br />Here is more text</p>";
        let output = brs_to_ps(input);

        let doc = Html::parse_fragment(&output);
        let p_sel = Selector::parse("p").unwrap();
        let p_count = doc.select(&p_sel).count();
        assert!(p_count >= 2, "expected multiple paragraphs from double BRs");
        assert!(
            output.contains("Here is some text") && output.contains("Here is more text"),
            "should preserve text chunks"
        );
    }

    #[test]
    fn test_rewrite_top_level_html() {
        let input = "<html><div>Content</div></html>";
        let output = rewrite_top_level(input);

        assert!(output.contains("<div"));
        assert!(!output.contains("<html"));
    }

    #[test]
    fn test_rewrite_top_level_body() {
        let input = "<body><p>Content</p></body>";
        let output = rewrite_top_level(input);

        assert!(output.contains("<div"));
        assert!(!output.contains("<body"));
    }

    #[test]
    fn test_rewrite_top_level_preserves_attributes() {
        let input = r#"<html lang="en"><body class="article"><div id="content">Test</div></body></html>"#;
        let output = rewrite_top_level(input);

        let doc = Html::parse_fragment(&output);
        let lang_sel = Selector::parse("div[lang=\"en\"]").unwrap();
        assert!(doc.select(&lang_sel).next().is_some());

        let class_sel = Selector::parse("div.article").unwrap();
        assert!(doc.select(&class_sel).next().is_some());
    }
}
