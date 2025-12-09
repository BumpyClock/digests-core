// ABOUTME: Go-compatible DOM cleaners for content extraction.
// ABOUTME: Ports CleanTags, CleanHeaders, CleanHOnes, CleanImages, RemoveEmpty, StripUnlikelyCandidates.

use ego_tree::NodeId;
use html5ever::LocalName;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;

use super::scoring::{get_weight, link_density, normalize_spaces};

// Keep class for elements that should be preserved
const KEEP_CLASS: &str = "hermes-parser-keep";

// Selectors to mark as keep (YouTube, Vimeo, Reddit iframes)
const KEEP_SELECTORS: &[&str] = &[
    r#"iframe[src^="https://www.youtube.com"]"#,
    r#"iframe[src^="https://www.youtube-nocookie.com"]"#,
    r#"iframe[src^="http://www.youtube.com"]"#,
    r#"iframe[src^="https://player.vimeo"]"#,
    r#"iframe[src^="http://player.vimeo"]"#,
    r#"iframe[src^="https://www.redditmedia.com"]"#,
];

// Tags to strip from output
const STRIP_OUTPUT_TAGS: &[&str] = &[
    "title", "script", "noscript", "link", "style", "hr", "embed", "iframe", "object",
];

// Header tags (excluding h1 which is handled separately)
const HEADER_TAGS: &[&str] = &["h2", "h3", "h4", "h5", "h6"];

// Spacer image pattern
static SPACER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)transparent|spacer|blank").unwrap());

// Unlikely candidate patterns
static CANDIDATES_BLACKLIST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(ad-break|ad-banner|adbox|advert|addthis|agegate|aux|blogger-labels|combx|comment|conversation|disqus|entry-unrelated|extra|foot|header|hidden|loader|login|menu|meta|nav|outbrain|pager|pagination|predicta|presence_control_external|popup|printfriendly|related|remove|remark|rss|share|shoutbox|sidebar|sociable|sponsor|taboola|tools)").unwrap()
});
static CANDIDATES_WHITELIST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(and|article|body|blogindex|column|content|entry-content-asset|format|hfeed|hentry|hatom|main|page|posts|shadow)").unwrap()
});

// Whitelist attributes to keep during cleaning
const WHITELIST_ATTRS: &[&str] = &[
    "src",
    "srcset",
    "sizes",
    "type",
    "href",
    "class",
    "id",
    "alt",
    "xlink:href",
    "width",
    "height",
];
static WHITELIST_ATTRS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(src|srcset|sizes|type|href|class|id|alt|xlink:href|width|height)$").unwrap()
});

// Attributes to always remove
const REMOVE_ATTRS: &[&str] = &["style", "align"];
const REMOVE_EMPTY_TAGS: &[&str] = &["p"];
const DIV_TO_P_BLOCK_TAGS: &[&str] = &["a", "blockquote", "dl", "div", "img", "p", "pre", "table"];
const NON_TOP_CANDIDATE_TAGS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(br|b|i|label|hr|area|base|basefont|input|img|link|meta)$").unwrap()
});
const HNEWS_CONTENT_SELECTORS: &[(&str, &str)] = &[
    (".hentry", ".entry-content"),
    ("entry", ".entry-content"),
    (".entry", ".entry_content"),
    (".post", ".postbody"),
    (".post", ".post_body"),
    (".post", ".post-body"),
];
const DIV_TO_P_BLOCK_TAGS_LIST: &str = "a,blockquote,dl,div,img,p,pre,table";
const CLEAN_CONDITIONALLY_TAGS_LIST: &str = "ul,ol,table,div,button,form";
const HEADER_TAG_LIST: &str = "h2,h3,h4,h5,h6";

/// Count commas in text
fn score_commas(text: &str) -> i32 {
    text.matches(',').count() as i32
}

/// Check if an element has the keep class or contains elements with keep class
fn should_keep(element: &ElementRef, keep_ids: &HashSet<ego_tree::NodeId>) -> bool {
    if keep_ids.contains(&element.id()) {
        return true;
    }
    if element.value().classes().any(|c| c == KEEP_CLASS) {
        return true;
    }
    let keep_sel = Selector::parse(&format!(".{}", KEEP_CLASS)).unwrap();
    element.select(&keep_sel).next().is_some()
}

/// Remove an element unless it has sufficient content
/// Returns true if element should be removed
fn remove_unless_content(element: &ElementRef, weight: i32) -> bool {
    // Keep entry-content-asset tagged elements
    if element
        .value()
        .classes()
        .any(|c| c == "entry-content-asset")
    {
        return false;
    }

    let content = normalize_spaces(&element.text().collect::<String>());

    // Only apply strict filtering if low comma count
    if score_commas(&content) < 10 {
        let p_count = Selector::parse("p")
            .ok()
            .map(|s| element.select(&s).count())
            .unwrap_or(0);
        let input_count = Selector::parse("input, textarea, select, button")
            .ok()
            .map(|s| element.select(&s).count())
            .unwrap_or(0);

        // Too many inputs relative to paragraphs
        if (input_count as f64) > (p_count as f64 / 3.0) {
            return true;
        }

        let content_length = content.len();
        let img_count = Selector::parse("img")
            .ok()
            .map(|s| element.select(&s).count())
            .unwrap_or(0);

        // Too short with no images
        if content_length < 25 && img_count == 0 {
            return true;
        }

        let density = link_density(element);

        // High link density with low weight
        if weight < 25 && density > 0.2 && content_length > 75 {
            return true;
        }

        // High link density even with decent weight
        if weight >= 25 && density > 0.5 {
            let tag_name = element.value().name().to_lowercase();
            let is_list = tag_name == "ol" || tag_name == "ul";

            if is_list {
                // Check if previous sibling ends with colon (indicates this is content)
                if let Some(prev) = element.prev_sibling() {
                    if let Some(prev_el) = ElementRef::wrap(prev) {
                        let prev_text = normalize_spaces(&prev_el.text().collect::<String>());
                        if prev_text.ends_with(':') {
                            return false;
                        }
                    }
                }
            }
            return true;
        }

        let script_count = Selector::parse("script")
            .ok()
            .map(|s| element.select(&s).count())
            .unwrap_or(0);

        // Scripts with little content
        if script_count > 0 && content_length < 150 {
            return true;
        }
    }

    false
}

/// Clean conditionally based on content (matches Go CleanTags)
pub fn clean_tags_filter(element: &ElementRef, keep_ids: &HashSet<ego_tree::NodeId>) -> bool {
    if should_keep(element, keep_ids) {
        return false;
    }

    // Get weight
    let weight = get_weight(element);

    // Remove if negative weight
    if weight < 0 {
        return true;
    }

    // Check content-based removal
    remove_unless_content(element, weight)
}

/// Check if element should be stripped as unlikely candidate
pub fn is_unlikely_candidate(element: &ElementRef) -> bool {
    // Don't strip links
    if element.value().name().eq_ignore_ascii_case("a") {
        return false;
    }

    let class = element.value().attr("class").unwrap_or("");
    let id = element.value().attr("id").unwrap_or("");

    if class.is_empty() && id.is_empty() {
        return false;
    }

    let class_and_id = format!("{} {}", class, id);

    // Check whitelist first
    if CANDIDATES_WHITELIST.is_match(&class_and_id) {
        return false;
    }

    // Check blacklist
    CANDIDATES_BLACKLIST.is_match(&class_and_id)
}

/// Check if header should be removed
pub fn should_remove_header(
    element: &ElementRef,
    title: &str,
    has_preceding_paragraph: bool,
) -> bool {
    // Remove if no preceding paragraphs and there are paragraphs in document
    if !has_preceding_paragraph {
        return true;
    }

    // Remove if matches title
    let header_text = normalize_spaces(&element.text().collect::<String>());
    if !title.is_empty() && header_text == normalize_spaces(title) {
        return true;
    }

    // Remove if negative weight
    if get_weight(element) < 0 {
        return true;
    }

    // Remove very short headers
    if header_text.len() < 3 {
        return true;
    }

    false
}

/// Clean H1 tags based on count (Go CleanHOnes logic)
/// If < 3 H1s, remove them all (they're likely titles)
/// If >= 3 H1s, convert them to H2s (they're section headers)
pub fn process_h1_tags(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let h1_selector = Selector::parse("h1").unwrap();

    let h1_count = fragment.select(&h1_selector).count();

    if h1_count == 0 {
        return html.to_string();
    }

    // Collect h1 node IDs
    let h1_ids: HashSet<_> = fragment.select(&h1_selector).map(|el| el.id()).collect();

    let mut output = String::new();
    serialize_h1_processed(fragment.root_element(), &h1_ids, h1_count, &mut output);

    output
}

fn serialize_h1_processed(
    node: ElementRef,
    h1_ids: &HashSet<ego_tree::NodeId>,
    h1_count: usize,
    output: &mut String,
) {
    for child in node.children() {
        match child.value() {
            scraper::Node::Text(text) => {
                output.push_str(&**text);
            }
            scraper::Node::Element(el) => {
                let is_h1 = h1_ids.contains(&child.id());
                let child_el = ElementRef::wrap(child).unwrap();

                if is_h1 {
                    if h1_count < 3 {
                        // Remove H1s
                        continue;
                    } else {
                        // Convert to H2
                        output.push_str("<h2");
                        for (name, value) in el.attrs() {
                            output.push(' ');
                            output.push_str(name);
                            output.push_str("=\"");
                            output.push_str(&escape_attr(value));
                            output.push('"');
                        }
                        output.push('>');
                        serialize_h1_processed(child_el, h1_ids, h1_count, output);
                        output.push_str("</h2>");
                    }
                } else {
                    output.push('<');
                    output.push_str(el.name());
                    for (name, value) in el.attrs() {
                        output.push(' ');
                        output.push_str(name);
                        output.push_str("=\"");
                        output.push_str(&escape_attr(value));
                        output.push('"');
                    }
                    if is_void_element(el.name()) {
                        output.push_str(" />");
                    } else {
                        output.push('>');
                        serialize_h1_processed(child_el, h1_ids, h1_count, output);
                        output.push_str("</");
                        output.push_str(el.name());
                        output.push('>');
                    }
                }
            }
            scraper::Node::Comment(comment) => {
                output.push_str("<!--");
                output.push_str(&**comment);
                output.push_str("-->");
            }
            _ => {}
        }
    }
}

/// Check if image should be removed (spacer or too small)
pub fn should_remove_image(element: &ElementRef) -> bool {
    // Check src for spacer patterns
    if let Some(src) = element.value().attr("src") {
        if SPACER_RE.is_match(src) {
            return true;
        }
    } else {
        // No src attribute
        return true;
    }

    // Check size
    let height = element
        .value()
        .attr("height")
        .and_then(|h| h.parse::<i32>().ok())
        .unwrap_or(20);
    let width = element
        .value()
        .attr("width")
        .and_then(|w| w.parse::<i32>().ok())
        .unwrap_or(20);

    height < 10 || width < 10
}

/// Check if paragraph is empty (no text content and no images)
pub fn is_empty_paragraph(element: &ElementRef) -> bool {
    let text = element.text().collect::<String>();
    if !text.trim().is_empty() {
        return false;
    }

    // Check for images
    let img_selector = Selector::parse("img").unwrap();
    element.select(&img_selector).next().is_none()
}

/// Convert shallow divs to paragraphs unless they contain block-level tags
fn convert_divs_to_paragraphs(fragment: &Html) -> String {
    let mut output = String::new();
    let block_selector = Selector::parse(DIV_TO_P_BLOCK_TAGS_LIST).unwrap();

    for child in fragment.root_element().children() {
        serialize_div_to_p(child, &block_selector, &mut output);
    }

    output
}

fn serialize_div_to_p(
    node: ego_tree::NodeRef<scraper::Node>,
    block_selector: &Selector,
    out: &mut String,
) {
    match node.value() {
        scraper::Node::Text(text) => out.push_str(&**text),
        scraper::Node::Element(el) => {
            let name = el.name().to_lowercase();
            let is_div = name == "div";

            // Decide tag to emit
            let use_p = if is_div {
                // If div has any block children, keep div
                let has_block = node
                    .children()
                    .filter_map(scraper::ElementRef::wrap)
                    .any(|c| block_selector.matches(&c));
                !has_block
            } else {
                false
            };

            let tag = if use_p { "p" } else { el.name() };

            out.push('<');
            out.push_str(tag);
            for (k, v) in el.attrs() {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(&escape_attr(v));
                out.push('"');
            }

            if is_void_element(tag) {
                out.push_str(" />");
                return;
            }

            out.push('>');
            for child in node.children() {
                serialize_div_to_p(child, block_selector, out);
            }
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        scraper::Node::Comment(c) => {
            out.push_str("<!--");
            out.push_str(&**c);
            out.push_str("-->");
        }
        _ => {}
    }
}

/// Remove unlikely candidates (blacklist vs whitelist)
fn strip_unlikely(
    fragment: &Html,
    keep_ids: &HashSet<ego_tree::NodeId>,
) -> HashSet<ego_tree::NodeId> {
    let mut skip = HashSet::new();
    let selector = Selector::parse("*").unwrap();
    for el in fragment.select(&selector) {
        if is_unlikely_candidate(&el) && !keep_ids.contains(&el.id()) {
            skip.insert(el.id());
        }
    }
    skip
}

fn build_keep_ids(fragment: &Html) -> HashSet<ego_tree::NodeId> {
    let mut keep = HashSet::new();
    for sel_str in KEEP_SELECTORS {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in fragment.select(&sel) {
                keep.insert(el.id());
            }
        }
    }
    keep
}

/// Conditional cleaning tags per Go CleanTags
fn mark_clean_conditionally(
    fragment: &Html,
    keep_ids: &HashSet<ego_tree::NodeId>,
    skip: &mut HashSet<ego_tree::NodeId>,
) {
    let selector = Selector::parse(CLEAN_CONDITIONALLY_TAGS_LIST).unwrap();
    for el in fragment.select(&selector) {
        if should_keep(&el, keep_ids) {
            continue;
        }
        let mut weight = get_score_attr(&el);
        if weight == 0 {
            weight = get_weight(&el);
        }
        if weight < 0 {
            skip.insert(el.id());
        } else if remove_unless_content(&el, weight) {
            skip.insert(el.id());
        }
    }
}

fn get_score_attr(el: &ElementRef) -> i32 {
    el.value()
        .attr("data-content-score")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Clean headers (h2..h6) using Go rules
fn mark_headers(fragment: &Html, title: &str, skip: &mut HashSet<ego_tree::NodeId>) {
    let selector = Selector::parse(HEADER_TAG_LIST).unwrap();
    let mut seen_p = false;
    // Walk in document order
    for node in fragment.tree.root().descendants() {
        if let Some(el) = ElementRef::wrap(node) {
            let name = el.value().name();
            if name.eq_ignore_ascii_case("p") {
                seen_p = true;
            }
            if selector.matches(&el) {
                let remove = should_remove_header(&el, title, seen_p);
                if remove {
                    skip.insert(el.id());
                }
            }
        }
    }
}

/// Clean images (spacers / too small)
fn mark_images(fragment: &Html, skip: &mut HashSet<ego_tree::NodeId>) {
    let selector = Selector::parse("img").unwrap();
    for el in fragment.select(&selector) {
        if should_remove_image(&el) {
            skip.insert(el.id());
        }
    }
}

/// Remove empty paragraphs
fn mark_empty(fragment: &Html, skip: &mut HashSet<ego_tree::NodeId>) {
    let selector = Selector::parse("p").unwrap();
    for el in fragment.select(&selector) {
        if is_empty_paragraph(&el) {
            skip.insert(el.id());
        }
    }
}

/// Serialize fragment skipping nodes and filtering attributes
fn serialize_cleaned(fragment: &Html, skip: &HashSet<ego_tree::NodeId>) -> String {
    let mut out = String::new();
    for child in fragment.root_element().children() {
        serialize_node_clean(child, skip, &mut out);
    }
    out
}

fn serialize_node_clean(
    node: ego_tree::NodeRef<scraper::Node>,
    skip: &HashSet<ego_tree::NodeId>,
    out: &mut String,
) {
    if skip.contains(&node.id()) {
        return;
    }
    match node.value() {
        scraper::Node::Text(t) => out.push_str(&**t),
        scraper::Node::Element(el) => {
            let name = el.name();
            out.push('<');
            out.push_str(name);

            for (k, v) in el.attrs() {
                if !WHITELIST_ATTRS_RE.is_match(k) {
                    continue;
                }
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(&escape_attr(v));
                out.push('"');
            }

            if is_void_element(name) {
                out.push_str(" />");
                return;
            }

            out.push('>');
            for child in node.children() {
                serialize_node_clean(child, skip, out);
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        scraper::Node::Comment(c) => {
            out.push_str("<!--");
            out.push_str(&**c);
            out.push_str("-->");
        }
        _ => {}
    }
}

/// Clean article HTML to mirror Go cleaners
pub fn clean_article(html: &str, title: &str) -> String {
    // Step 1: convert divs to paragraphs where appropriate
    let converted = {
        let frag = Html::parse_fragment(html);
        convert_divs_to_paragraphs(&frag)
    };

    // Step 2: parse fragment for subsequent cleaning
    let fragment = Html::parse_fragment(&converted);
    let keep_ids = build_keep_ids(&fragment);

    // Step 3: build skip set
    let mut skip = strip_unlikely(&fragment, &keep_ids);
    mark_clean_conditionally(&fragment, &keep_ids, &mut skip);
    mark_headers(&fragment, title, &mut skip);
    mark_images(&fragment, &mut skip);
    mark_empty(&fragment, &mut skip);

    // Step 4: serialize with attribute filtering
    let cleaned = serialize_cleaned(&fragment, &skip);

    // Step 5: convert <br><br> to paragraphs and rewrite top-level
    let br_fixed = crate::dom::brs::brs_to_ps(&cleaned);
    crate::dom::brs::rewrite_top_level(&br_fixed)
}

/// Escape attribute value
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Check if tag is void element
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
    fn test_is_unlikely_candidate() {
        let html = r#"<div class="sidebar ad-break">Ads</div>"#;
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("div").unwrap();
        let el = doc.select(&sel).next().unwrap();

        assert!(is_unlikely_candidate(&el));
    }

    #[test]
    fn test_is_unlikely_candidate_whitelist() {
        let html = r#"<div class="article-content sidebar">Content</div>"#;
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("div").unwrap();
        let el = doc.select(&sel).next().unwrap();

        // Should not be unlikely because "article" and "content" are in whitelist
        assert!(!is_unlikely_candidate(&el));
    }

    #[test]
    fn test_process_h1_tags_remove() {
        let html = r#"<h1>Title</h1><p>Content</p><h1>Subtitle</h1>"#;
        let result = process_h1_tags(html);

        // Should remove H1s (count < 3)
        assert!(!result.contains("<h1>"));
    }

    #[test]
    fn test_process_h1_tags_convert() {
        let html = r#"<h1>One</h1><h1>Two</h1><h1>Three</h1>"#;
        let result = process_h1_tags(html);

        // Should convert H1s to H2s (count >= 3)
        assert!(result.contains("<h2>"));
        assert!(!result.contains("<h1>"));
    }

    #[test]
    fn test_should_remove_image_spacer() {
        let html = r#"<img src="transparent.gif" />"#;
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("img").unwrap();
        let el = doc.select(&sel).next().unwrap();

        assert!(should_remove_image(&el));
    }

    #[test]
    fn test_should_remove_image_small() {
        let html = r#"<img src="icon.png" width="5" height="5" />"#;
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("img").unwrap();
        let el = doc.select(&sel).next().unwrap();

        assert!(should_remove_image(&el));
    }

    #[test]
    fn test_is_empty_paragraph() {
        let html = r#"<p>   </p>"#;
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("p").unwrap();
        let el = doc.select(&sel).next().unwrap();

        assert!(is_empty_paragraph(&el));
    }

    #[test]
    fn test_is_empty_paragraph_with_image() {
        let html = r#"<p><img src="photo.jpg" /></p>"#;
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("p").unwrap();
        let el = doc.select(&sel).next().unwrap();

        assert!(!is_empty_paragraph(&el));
    }
}
