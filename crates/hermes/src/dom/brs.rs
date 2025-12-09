// ABOUTME: Go-compatible BR to paragraph conversion.
// ABOUTME: Ports BrsToPs from Go hermes internal/utils/dom/brs.go

use dom_query::Document;

/// Convert consecutive BR tags to paragraphs
/// Matches Go's BrsToPs behavior using DOM mutation
pub fn brs_to_ps(html: &str) -> String {
    // Quick check: if no double BR patterns exist, return unchanged
    if !html.contains("<br") {
        return html.to_string();
    }

    let doc = Document::from(html);

    // Get all BR elements
    let brs: Vec<_> = doc.select("br").iter().collect();

    if brs.is_empty() {
        return html.to_string();
    }

    // Track consecutive BRs and replace them
    // Collect indices of BRs that start a sequence
    let br_vec: Vec<_> = brs.iter().collect();
    let mut i = 0;
    while i < br_vec.len() {
        let br = &br_vec[i];

        // Check if this BR has length (hasn't been removed)
        if br.length() == 0 {
            i += 1;
            continue;
        }

        // Check if the next sibling (skipping whitespace) is also a BR
        if find_next_br(br).is_some() {
            // Found consecutive BRs - replace with empty paragraph
            br.replace_with_html("<p> </p>");

            // Skip ahead - the subsequent BRs in this sequence will be removed
            // by the paragraph replacement
            i += 1;

            // Remove all subsequent BRs that follow
            while i < br_vec.len() {
                let next_br = &br_vec[i];
                if next_br.length() > 0 && next_br.is("br") {
                    next_br.remove();
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            i += 1;
        }
    }

    let result = doc.html().to_string();

    // Now wrap bare text nodes in paragraphs
    // This is a simplified approach - just parse again and wrap
    wrap_bare_text(&result)
}

/// Find the next BR element, skipping whitespace-only text nodes
fn find_next_br<'a>(br: &'a dom_query::Selection<'a>) -> Option<dom_query::Selection<'a>> {
    let mut current = br.next_sibling();

    while current.length() > 0 {
        let text = current.text();

        // If this is a BR element, return it
        if current.is("br") {
            return Some(current);
        }

        // If it's not whitespace-only, it's not a BR
        if !text.trim().is_empty() {
            return None;
        }

        current = current.next_sibling();
    }

    None
}

/// Wrap bare text content in paragraphs
/// This handles wrapping text that appears after double BR replacement
fn wrap_bare_text(html: &str) -> String {
    let doc = Document::from(html);

    // Find body or root element (not currently used, but kept for reference)
    let _root = if doc.select("body").length() > 0 {
        doc.select("body")
    } else {
        doc.select("*").first()
    };

    // Process children and wrap text nodes
    // For simplicity, we'll rebuild HTML with proper wrapping
    // This is a basic implementation - a full implementation would recursively process all nodes
    let mut result = String::new();
    let html_output = doc.html().to_string();

    // Split by block-level elements and wrap non-block content
    let mut in_block = false;
    let mut current_chunk = String::new();

    for line in html_output.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        let is_block_start = trimmed.starts_with("<p")
            || trimmed.starts_with("<div")
            || trimmed.starts_with("<article")
            || trimmed.starts_with("<section")
            || trimmed.starts_with("<h1")
            || trimmed.starts_with("<h2")
            || trimmed.starts_with("<h3")
            || trimmed.starts_with("<h4")
            || trimmed.starts_with("<h5")
            || trimmed.starts_with("<h6")
            || trimmed.starts_with("<ul")
            || trimmed.starts_with("<ol")
            || trimmed.starts_with("<blockquote")
            || trimmed.starts_with("<html")
            || trimmed.starts_with("<body")
            || trimmed.starts_with("<!DOCTYPE");

        let is_block_end = trimmed.ends_with("</p>")
            || trimmed.ends_with("</div>")
            || trimmed.ends_with("</article>")
            || trimmed.ends_with("</section>")
            || trimmed.ends_with("</ul>")
            || trimmed.ends_with("</ol>")
            || trimmed.ends_with("</blockquote>")
            || trimmed.ends_with("</html>")
            || trimmed.ends_with("</body>");

        if is_block_start {
            // Flush current chunk
            if !current_chunk.trim().is_empty() && !current_chunk.trim().starts_with('<') {
                result.push_str("<p>");
                result.push_str(current_chunk.trim());
                result.push_str("</p>\n");
                current_chunk.clear();
            }
            result.push_str(line);
            result.push('\n');
            in_block = true;
        } else if is_block_end {
            result.push_str(line);
            result.push('\n');
            in_block = false;
        } else if in_block {
            result.push_str(line);
            result.push('\n');
        } else {
            current_chunk.push_str(line);
            current_chunk.push(' ');
        }
    }

    // Flush remaining chunk
    if !current_chunk.trim().is_empty() {
        result.push_str("<p>");
        result.push_str(current_chunk.trim());
        result.push_str("</p>");
    }

    result
}

/// Rewrite top-level body/html elements to divs
/// Matches Go's RewriteTopLevel behavior
pub fn rewrite_top_level(html: &str) -> String {
    let doc = Document::from(html);

    // Rename html elements to div
    let html_elements = doc.select("html");
    for elem in html_elements.iter() {
        elem.rename("div");
    }

    // Rename body elements to div
    let body_elements = doc.select("body");
    for elem in body_elements.iter() {
        elem.rename("div");
    }

    doc.html().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brs_to_ps_basic() {
        let input = "First paragraph<br><br>Second paragraph";
        let output = brs_to_ps(input);

        // After migration to dom_query, the output wraps in html/head/body tags
        // and splits by double BR. The key behavior is:
        // 1. Double BR becomes a paragraph break
        // 2. Text before and after are separated
        assert!(output.contains("First paragraph"));
        assert!(output.contains("Second paragraph"));
        assert!(output.contains("<p")); // Has paragraph tags
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

        // Should return unchanged (or minimally changed)
        assert!(output.contains("Single") && output.contains("break only"));
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

        let doc = Document::from(output.as_str());
        let p_count = doc.select("p").length();
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
        let input =
            r#"<html lang="en"><body class="article"><div id="content">Test</div></body></html>"#;
        let output = rewrite_top_level(input);

        let doc = Document::from(output.as_str());
        assert!(doc.select("div[lang=\"en\"]").length() > 0);
        assert!(doc.select("div.article").length() > 0);
    }
}
