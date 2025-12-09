// ABOUTME: Go-compatible DOM cleaners for content extraction.
// ABOUTME: Implements unlikely stripping, conditional cleaning, heading fixes, br->p, and top-level rewrite.

use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;

use super::scoring::{get_weight, link_density, normalize_spaces};

const KEEP_CLASS: &str = "hermes-parser-keep";

const KEEP_SELECTORS: &[&str] = &[
    r#"iframe[src^=\"https://www.youtube.com\"]"#,
    r#"iframe[src^=\"https://www.youtube-nocookie.com\"]"#,
    r#"iframe[src^=\"http://www.youtube.com\"]"#,
    r#"iframe[src^=\"https://player.vimeo\"]"#,
    r#"iframe[src^=\"http://player.vimeo\"]"#,
    r#"iframe[src^=\"https://www.redditmedia.com\"]"#,
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

pub fn clean_tags_filter(element: &ElementRef, keep_ids: &HashSet<ego_tree::NodeId>) -> bool {
    if should_keep(element, keep_ids) {
        return false;
    }
    let weight = get_weight(element);
    if weight < 0 {
        return true;
    }
    remove_unless_content(element, weight)
}

pub fn is_unlikely_candidate(element: &ElementRef) -> bool {
    if element.value().name().eq_ignore_ascii_case("a") {
        return false;
    }
    let class = element.value().attr("class").unwrap_or("");
    let id = element.value().attr("id").unwrap_or("");
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
    element: &ElementRef,
    title: &str,
    has_preceding_paragraph: bool,
) -> bool {
    if !has_preceding_paragraph {
        return true;
    }
    let header_text = normalize_spaces(&element.text().collect::<String>());
    if !title.is_empty() && header_text == normalize_spaces(title) {
        return true;
    }
    if get_weight(element) < 0 {
        return true;
    }
    if header_text.len() < 3 {
        return true;
    }
    false
}

pub fn process_h1_tags(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let h1_selector = Selector::parse("h1").unwrap();
    let h1_count = fragment.select(&h1_selector).count();
    if h1_count == 0 {
        return html.to_string();
    }
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
            scraper::Node::Text(text) => output.push_str(&**text),
            scraper::Node::Element(el) => {
                let is_h1 = h1_ids.contains(&child.id());
                let child_el = ElementRef::wrap(child).unwrap();
                if is_h1 {
                    if h1_count < 3 {
                        continue;
                    } else {
                        output.push_str("<h2");
                        append_sorted_attrs(&mut *output, el.attrs());
                        output.push('>');
                        serialize_h1_processed(child_el, h1_ids, h1_count, output);
                        output.push_str("</h2>");
                    }
                } else {
                    output.push('<');
                    output.push_str(el.name());
                    append_sorted_attrs(&mut *output, el.attrs());
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

pub fn should_remove_image(element: &ElementRef) -> bool {
    if let Some(src) = element.value().attr("src") {
        if SPACER_RE.is_match(src) {
            return true;
        }
    } else {
        return true;
    }
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

pub fn is_empty_paragraph(element: &ElementRef) -> bool {
    let text = element.text().collect::<String>();
    if !text.trim().is_empty() {
        return false;
    }
    let img_selector = Selector::parse("img").unwrap();
    element.select(&img_selector).next().is_none()
}

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

fn remove_unless_content(element: &ElementRef, weight: i32) -> bool {
    if element
        .value()
        .classes()
        .any(|c| c == "entry-content-asset")
    {
        return false;
    }
    let content = normalize_spaces(&element.text().collect::<String>());
    if score_commas(&content) < 10 {
        let p_count = Selector::parse("p")
            .ok()
            .map(|s| element.select(&s).count())
            .unwrap_or(0);
        let input_count = Selector::parse("input, textarea, select, button")
            .ok()
            .map(|s| element.select(&s).count())
            .unwrap_or(0);
        if (input_count as f64) > (p_count as f64 / 3.0) {
            return true;
        }
        let content_length = content.len();
        let img_count = Selector::parse("img")
            .ok()
            .map(|s| element.select(&s).count())
            .unwrap_or(0);
        if content_length < 25 && img_count == 0 {
            return true;
        }
        let density = link_density(element);
        if weight < 25 && density > 0.2 && content_length > 75 {
            return true;
        }
        if weight >= 25 && density > 0.5 {
            let tag_name = element.value().name().to_lowercase();
            let is_list = tag_name == "ol" || tag_name == "ul";
            if is_list {
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
        if script_count > 0 && content_length < 150 {
            return true;
        }
    }
    false
}

fn score_commas(text: &str) -> i32 {
    text.matches(',').count() as i32
}

fn strip_unlikely(fragment: &Html, keep_ids: &HashSet<ego_tree::NodeId>) -> HashSet<ego_tree::NodeId> {
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
    if let Ok(sel) = Selector::parse(&format!(".{}", KEEP_CLASS)) {
        for el in fragment.select(&sel) {
            keep.insert(el.id());
        }
    }
    keep
}

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
        let weight = get_weight(&el);
        if weight < 0 {
            skip.insert(el.id());
        } else if remove_unless_content(&el, weight) {
            skip.insert(el.id());
        }
    }
}

fn mark_headers(fragment: &Html, title: &str, skip: &mut HashSet<ego_tree::NodeId>) {
    let selector = Selector::parse(HEADER_TAG_LIST).unwrap();
    let mut seen_p = false;
    for node in fragment.tree.root().descendants() {
        if let Some(el) = ElementRef::wrap(node) {
            let name = el.value().name();
            if name.eq_ignore_ascii_case("p") {
                seen_p = true;
            }
            if selector.matches(&el) {
                if should_remove_header(&el, title, seen_p) {
                    skip.insert(el.id());
                }
            }
        }
    }
}

fn mark_images(fragment: &Html, skip: &mut HashSet<ego_tree::NodeId>) {
    let selector = Selector::parse("img").unwrap();
    for el in fragment.select(&selector) {
        if should_remove_image(&el) {
            skip.insert(el.id());
        }
    }
}

fn mark_empty(fragment: &Html, skip: &mut HashSet<ego_tree::NodeId>) {
    let selector = Selector::parse("p").unwrap();
    for el in fragment.select(&selector) {
        if is_empty_paragraph(&el) {
            skip.insert(el.id());
        }
    }
}

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

pub fn clean_article(html: &str, title: &str) -> String {
    let converted = {
        let frag = Html::parse_fragment(html);
        convert_divs_to_paragraphs(&frag)
    };
    let converted_h1 = process_h1_tags(&converted);
    let converted = if converted_h1.trim().is_empty() {
        converted
    } else {
        converted_h1
    };
    let fragment = Html::parse_fragment(&converted);
    let keep_ids = build_keep_ids(&fragment);
    let mut skip = strip_unlikely(&fragment, &keep_ids);
    mark_clean_conditionally(&fragment, &keep_ids, &mut skip);
    mark_headers(&fragment, title, &mut skip);
    mark_images(&fragment, &mut skip);
    mark_empty(&fragment, &mut skip);
    let cleaned = serialize_cleaned(&fragment, &skip);
    let br_fixed = crate::dom::brs::brs_to_ps(&cleaned);
    crate::dom::brs::rewrite_top_level(&br_fixed)
}

fn convert_divs_to_paragraphs(fragment: &Html) -> String {
    let mut output = String::new();
    let block_selector = Selector::parse("a,blockquote,dl,div,img,p,pre,table").unwrap();
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
            let is_div = name == "div" || name == "span";
            let use_p = if is_div {
                !node
                    .children()
                    .filter_map(scraper::ElementRef::wrap)
                    .any(|c| block_selector.matches(&c))
            } else {
                false
            };
            let tag = if use_p { "p" } else { el.name() };
            out.push('<');
            out.push_str(tag);
            append_sorted_attrs(out, el.attrs());
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

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn attr_priority(name: &str) -> u8 {
    match name {
        "id" => 0,
        "class" => 1,
        _ => 2,
    }
}

fn append_sorted_attrs<'a, I>(out: &mut String, attrs: I)
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut list: Vec<(String, String)> =
        attrs.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    list.sort_by(|a, b| {
        let wa = attr_priority(&a.0);
        let wb = attr_priority(&b.0);
        wa.cmp(&wb).then_with(|| a.0.cmp(&b.0))
    });
    for (name, value) in list {
        out.push(' ');
        out.push_str(&name);
        out.push_str("=\"");
        out.push_str(&escape_attr(&value));
        out.push('"');
    }
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
        let doc = Html::parse_fragment(&cleaned);
        let h1_sel = Selector::parse("h1").unwrap();
        assert!(doc.select(&h1_sel).next().is_none());

        let h2_sel = Selector::parse("h2").unwrap();
        let h2s: Vec<_> = doc.select(&h2_sel).collect();
        assert_eq!(3, h2s.len());

        let first = &h2s[0];
        assert_eq!(first.value().attr("id"), Some("heading1"));
        assert_eq!(first.value().attr("class"), Some("main-title"));
        assert_eq!(first.value().attr("data-test"), Some("value"));
        assert_eq!(
            normalize_spaces(&first.text().collect::<String>()),
            "First"
        );
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
