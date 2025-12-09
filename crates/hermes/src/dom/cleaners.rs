// ABOUTME: Go-compatible DOM cleaners for content extraction.
// ABOUTME: Implements unlikely stripping, conditional cleaning, heading fixes, br->p, and top-level rewrite.

use once_cell::sync::Lazy;
use regex::Regex;
use dom_query::{Document, Selection};

use super::scoring::{get_weight, link_density, normalize_spaces};

const KEEP_CLASS: &str = "hermes-parser-keep";

const KEEP_SELECTORS: &[&str] = &[
    r#"iframe[src^="https://www.youtube.com"]"#,
    r#"iframe[src^="https://www.youtube-nocookie.com"]"#,
    r#"iframe[src^="http://www.youtube.com"]"#,
    r#"iframe[src^="https://player.vimeo"]"#,
    r#"iframe[src^="http://player.vimeo"]"#,
    r#"iframe[src^="https://www.redditmedia.com"]"#,
];

#[allow(dead_code)]
const STRIP_OUTPUT_TAGS: &[&str] = &[
    "title", "script", "noscript", "link", "style", "hr", "embed", "iframe", "object",
];

#[allow(dead_code)]
const HEADER_TAGS: &[&str] = &["h2", "h3", "h4", "h5", "h6"];

static SPACER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)transparent|spacer|blank").unwrap());

static CANDIDATES_BLACKLIST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(ad-break|ad-banner|adbox|advert|addthis|agegate|aux|blogger-labels|combx|comment|conversation|disqus|entry-unrelated|extra|foot|header|hidden|loader|login|menu|meta|nav|outbrain|pager|pagination|predicta|presence_control_external|popup|printfriendly|related|remove|remark|rss|share|shoutbox|sidebar|sociable|sponsor|taboola|tools)").unwrap()
});
static CANDIDATES_WHITELIST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(and|article|body|blogindex|column|content|entry-content-asset|format|hfeed|hentry|hatom|main|page|posts|shadow)").unwrap()
});

#[allow(dead_code)]
const WHITELIST_ATTRS: &[&str] = &[
    "src", "srcset", "sizes", "type", "href", "class", "id", "alt", "xlink:href", "width", "height",
];
static WHITELIST_ATTRS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(src|srcset|sizes|type|href|class|id|alt|xlink:href|width|height)$").unwrap()
});

#[allow(dead_code)]
const REMOVE_EMPTY_TAGS: &[&str] = &["p"];
const CLEAN_CONDITIONALLY_TAGS_LIST: &str = "ul,ol,table,div,button,form";
const HEADER_TAG_LIST: &str = "h2,h3,h4,h5,h6";

pub fn is_unlikely_candidate(sel: &Selection) -> bool {
    if sel.is("a") {
        return false;
    }
    let class = sel.attr("class").unwrap_or_default();
    let id = sel.attr("id").unwrap_or_default();
    if class.is_empty() && id.is_empty() {
        return false;
    }
    let combo = format!("{} {}", class, id);
    if CANDIDATES_WHITELIST.is_match(&combo) {
        return false;
    }
    CANDIDATES_BLACKLIST.is_match(&combo)
}

pub fn should_remove_header(
    sel: &Selection,
    title: &str,
    has_preceding_paragraph: bool,
) -> bool {
    if !has_preceding_paragraph {
        return true;
    }
    let header_text = normalize_spaces(&sel.text());
    if !title.is_empty() && header_text == normalize_spaces(title) {
        return true;
    }
    if get_weight(sel) < 0 {
        return true;
    }
    if header_text.len() < 3 {
        return true;
    }
    false
}

pub fn process_h1_tags(html: &str) -> String {
    let doc = Document::from(html);
    let h1_count = doc.select("h1").length();

    if h1_count == 0 {
        return html.to_string();
    }

    if h1_count < 3 {
        // Remove all h1 tags
        doc.select("h1").remove();
    } else {
        // Convert h1 tags to h2
        let h1s = doc.select("h1");
        for node in h1s.nodes() {
            let sel = Selection::from(node.clone());
            let outer_html = sel.html().to_string();
            let new_html = outer_html.replacen("<h1", "<h2", 1).replacen("</h1>", "</h2>", 1);
            sel.replace_with_html(new_html.as_str());
        }
    }

    doc.html().to_string()
}

pub fn should_remove_image(sel: &Selection) -> bool {
    if let Some(src) = sel.attr("src") {
        if SPACER_RE.is_match(&src) {
            return true;
        }
    } else {
        return true;
    }
    let height = sel
        .attr("height")
        .and_then(|h| h.parse::<i32>().ok())
        .unwrap_or(20);
    let width = sel
        .attr("width")
        .and_then(|w| w.parse::<i32>().ok())
        .unwrap_or(20);
    height < 10 || width < 10
}

pub fn is_empty_paragraph(sel: &Selection) -> bool {
    let text = sel.text();
    if !text.trim().is_empty() {
        return false;
    }
    sel.select("img").length() == 0
}

fn should_keep(sel: &Selection, keep_class_selectors: &[String]) -> bool {
    if sel.has_class(KEEP_CLASS) {
        return true;
    }
    for keep_sel in keep_class_selectors {
        if sel.is(keep_sel) {
            return true;
        }
    }
    // Check if any descendant has the keep class
    sel.select(&format!(".{}", KEEP_CLASS)).length() > 0
}

fn remove_unless_content(sel: &Selection, weight: i32) -> bool {
    if sel.has_class("entry-content-asset") {
        return false;
    }
    let content = normalize_spaces(&sel.text());
    if score_commas(&content) < 10 {
        let p_count = sel.select("p").length();
        let input_count = sel.select("input, textarea, select, button").length();
        if (input_count as f64) > (p_count as f64 / 3.0) {
            return true;
        }
        let content_length = content.len();
        let img_count = sel.select("img").length();
        if content_length < 25 && img_count == 0 {
            return true;
        }
        let density = link_density(sel);
        if weight < 25 && density > 0.2 && content_length > 75 {
            return true;
        }
        if weight >= 25 && density > 0.5 {
            let tag_name = sel.nodes().first().map(|n| n.node_name().unwrap_or_default().to_lowercase()).unwrap_or_default();
            let is_list = tag_name == "ol" || tag_name == "ul";
            if is_list {
                // Check previous sibling element
                if let Some(node) = sel.nodes().first() {
                    if let Some(prev_node) = node.prev_element_sibling() {
                        let prev = Selection::from(prev_node);
                        let prev_text = normalize_spaces(&prev.text());
                        if prev_text.ends_with(':') {
                            return false;
                        }
                    }
                }
            }
            return true;
        }
        let script_count = sel.select("script").length();
        if script_count > 0 && content_length < 150 {
            return true;
        }
    }
    false
}

fn score_commas(text: &str) -> i32 {
    text.matches(',').count() as i32
}

fn strip_unlikely(doc: &mut Document, keep_selectors: &[String]) {
    let elements: Vec<_> = doc.select("*").nodes().iter().cloned().collect();

    for node in elements {
        let sel = Selection::from(node);
        if is_unlikely_candidate(&sel) && !should_keep(&sel, keep_selectors) {
            sel.remove();
        }
    }
}

fn build_keep_selectors(doc: &Document) -> Vec<String> {
    let mut keep = Vec::new();

    for sel_str in KEEP_SELECTORS {
        if doc.select(sel_str).length() > 0 {
            keep.push(sel_str.to_string());
        }
    }

    keep.push(format!(".{}", KEEP_CLASS));
    keep
}

fn clean_conditionally(doc: &mut Document, keep_selectors: &[String]) {
    let elements: Vec<_> = doc.select(CLEAN_CONDITIONALLY_TAGS_LIST).nodes().iter().cloned().collect();

    for node in elements {
        let sel = Selection::from(node);
        if should_keep(&sel, keep_selectors) {
            continue;
        }
        let weight = get_weight(&sel);
        if weight < 0 || remove_unless_content(&sel, weight) {
            sel.remove();
        }
    }
}

fn clean_headers(doc: &mut Document, title: &str) {
    let mut seen_p = false;

    // We need to iterate through all elements in document order
    // Since dom_query doesn't provide a direct way to iterate in document order,
    // we'll select all elements and check them
    let all_elements: Vec<_> = doc.select("*").nodes().iter().cloned().collect();

    for node in all_elements {
        let sel = Selection::from(node);
        let tag_name = sel.nodes().first()
            .and_then(|n| n.node_name())
            .unwrap_or_default()
            .to_lowercase();

        if tag_name == "p" {
            seen_p = true;
        }

        if HEADER_TAG_LIST.split(',').any(|t| t == tag_name) {
            if should_remove_header(&sel, title, seen_p) {
                sel.remove();
            }
        }
    }
}

fn clean_images(doc: &mut Document) {
    let images: Vec<_> = doc.select("img").nodes().iter().cloned().collect();

    for node in images {
        let sel = Selection::from(node);
        if should_remove_image(&sel) {
            sel.remove();
        }
    }
}

fn clean_empty_paragraphs(doc: &mut Document) {
    let paragraphs: Vec<_> = doc.select("p").nodes().iter().cloned().collect();

    for node in paragraphs {
        let sel = Selection::from(node);
        if is_empty_paragraph(&sel) {
            sel.remove();
        }
    }
}

fn filter_attributes(doc: &mut Document) {
    let elements: Vec<_> = doc.select("*").nodes().iter().cloned().collect();

    for node in elements {
        let sel = Selection::from(node.clone());
        let attrs: Vec<String> = sel.nodes().first()
            .map(|n| {
                n.attrs()
                    .iter()
                    .filter(|attr| !WHITELIST_ATTRS_RE.is_match(&attr.name.local))
                    .map(|attr| attr.name.local.to_string())
                    .collect()
            })
            .unwrap_or_default();

        for attr in attrs {
            sel.remove_attr(&attr);
        }
    }
}

pub fn clean_article(html: &str, title: &str) -> String {
    let converted = {
        let doc = Document::from(html);
        convert_divs_to_paragraphs(&doc)
    };

    let converted_h1 = process_h1_tags(&converted);
    let converted = if converted_h1.trim().is_empty() {
        converted
    } else {
        converted_h1
    };

    let mut doc = Document::from(converted.as_str());
    let keep_selectors = build_keep_selectors(&doc);

    strip_unlikely(&mut doc, &keep_selectors);
    clean_conditionally(&mut doc, &keep_selectors);
    clean_headers(&mut doc, title);
    clean_images(&mut doc);
    clean_empty_paragraphs(&mut doc);
    filter_attributes(&mut doc);

    let cleaned = doc.html();
    let br_fixed = crate::dom::brs::brs_to_ps(&cleaned);
    crate::dom::brs::rewrite_top_level(&br_fixed)
}

fn convert_divs_to_paragraphs(doc: &Document) -> String {
    let html_str = doc.html().to_string();
    let result = Document::from(html_str.as_str());

    // Find all divs and spans that don't contain block-level elements
    let block_tags = "a,blockquote,dl,div,img,p,pre,table";
    let divs_and_spans: Vec<_> = result.select("div, span").nodes().iter().cloned().collect();

    for node in divs_and_spans {
        let sel = Selection::from(node);
        let tag_name = sel.nodes().first()
            .and_then(|n| n.node_name())
            .unwrap_or_default()
            .to_lowercase();

        if tag_name != "div" && tag_name != "span" {
            continue;
        }

        // Check if it contains any block-level elements
        let has_block = block_tags.split(',')
            .any(|tag| sel.select(tag).length() > 0);

        if !has_block {
            // Convert to <p>
            let inner = sel.inner_html();
            let attrs = sel.nodes().first()
                .map(|n| {
                    n.attrs()
                        .iter()
                        .map(|attr| format!("{}=\"{}\"", attr.name.local, escape_attr(&attr.value)))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();

            let new_html = if attrs.is_empty() {
                format!("<p>{}</p>", inner)
            } else {
                format!("<p {}>{}</p>", attrs, inner)
            };
            sel.replace_with_html(new_html.as_str());
        }
    }

    result.html().to_string()
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn is_void_element(tag: &str) -> bool {
    matches!(
        tag.to_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_h1_tags_removes_when_less_than_three() {
        let html = r#"<div><h1>Look at this!</h1><p>Body</p><h1>Another</h1></div>"#;
        let cleaned = process_h1_tags(html);

        assert!(!cleaned.to_lowercase().contains("<h1"));
        assert!(cleaned.contains("<p>Body</p>"));
    }

    #[test]
    fn test_process_h1_tags_converts_when_three_or_more() {
        let html = r#"<div>
            <h1 id="heading1" class="main-title" data-test="value">First</h1>
            <h1 class="secondary">Second</h1>
            <h1>Third</h1>
            <p>Content</p>
        </div>"#;

        let cleaned = process_h1_tags(html);
        let doc = Document::from(cleaned.as_str());

        assert_eq!(doc.select("h1").length(), 0);
        assert_eq!(doc.select("h2").length(), 3);

        let first = doc.select("h2").first();
        assert_eq!(first.attr("id"), Some("heading1".into()));
        assert_eq!(first.attr("class"), Some("main-title".into()));
        assert_eq!(first.attr("data-test"), Some("value".into()));
        assert_eq!(normalize_spaces(&first.text()), "First");
    }

    #[test]
    fn test_clean_article_respects_unlikely_candidates_and_conditionals() {
        let html = r#"
            <div class="content">
                <div class="sidebar">Short sidebar text</div>
                <div class="article">
                    <p>This is substantial article content that should be preserved because it has enough text and doesn't match negative patterns.</p>
                </div>
            </div>
        "#;
        let cleaned = clean_article(html, "");
        assert!(cleaned.contains("substantial article content"));
        assert!(!cleaned.contains("sidebar text"));
    }
}
