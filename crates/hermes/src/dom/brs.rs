// ABOUTME: Go-compatible BR to paragraph conversion.
// ABOUTME: Ports BrsToPs from Go hermes internal/utils/dom/brs.go

use scraper::{Html, Selector};

/// Patterns for BR tags in various formats
const BR_PATTERNS: &[&str] = &["<br>", "<br/>", "<br />", "<BR>", "<BR/>", "<BR />"];

/// Check if we have 2+ consecutive BRs
fn has_double_br(html: &str) -> bool {
    let html_lower = html.to_lowercase();

    // Check for patterns like <br><br> or <br /><br />
    for pat1 in BR_PATTERNS {
        for pat2 in BR_PATTERNS {
            let double = format!("{}{}", pat1.to_lowercase(), pat2.to_lowercase());
            if html_lower.contains(&double) {
                return true;
            }
            // Allow whitespace between
            let double_ws = format!("{}\n{}", pat1.to_lowercase(), pat2.to_lowercase());
            if html_lower.contains(&double_ws) {
                return true;
            }
            let double_ws2 = format!("{} {}", pat1.to_lowercase(), pat2.to_lowercase());
            if html_lower.contains(&double_ws2) {
                return true;
            }
        }
    }
    false
}

/// Convert consecutive BR tags to paragraphs
/// Matches Go's BrsToPs behavior
pub fn brs_to_ps(html: &str) -> String {
    // Quick check - if no double BRs, nothing to do
    if !has_double_br(html) {
        return html.to_string();
    }

    let mut result = html.to_string();

    // Replace 2+ consecutive BRs with paragraph break marker
    // Use a unique marker to avoid conflicts
    const MARKER: &str = "<!--BR_TO_P_SPLIT-->";

    // Normalize BR tags first
    for pat in BR_PATTERNS {
        result = result.replace(pat, "<br>");
    }

    // Replace consecutive BRs (with optional whitespace) with marker
    let br_consecutive_patterns = [
        "<br><br>",
        "<br>\n<br>",
        "<br> <br>",
        "<br>\r\n<br>",
        "<br>\n\n<br>",
    ];

    for pattern in &br_consecutive_patterns {
        while result.contains(pattern) {
            result = result.replace(pattern, MARKER);
        }
    }

    // If no markers were placed, return original
    if !result.contains(MARKER) {
        return html.to_string();
    }

    // Split by marker and wrap each non-empty segment in <p>
    let parts: Vec<&str> = result.split(MARKER).collect();

    if parts.len() <= 1 {
        return html.to_string();
    }

    let mut output = String::new();
    for (i, part) in parts.iter().enumerate() {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if already wrapped in block element
        let lower = trimmed.to_lowercase();
        let is_block = lower.starts_with("<p")
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

        if is_block {
            output.push_str(trimmed);
        } else {
            output.push_str("<p>");
            output.push_str(trimmed);
            output.push_str("</p>");
        }

        if i < parts.len() - 1 {
            output.push('\n');
        }
    }

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
}
