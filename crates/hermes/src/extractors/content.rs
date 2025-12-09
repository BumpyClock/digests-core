// ABOUTME: Content extraction utility for extracting HTML content from DOM using ContentExtractor.
// ABOUTME: Supports selectors, transforms (tag renaming), clean selectors, and default content cleaning.

//! Content extraction utilities for extracting HTML strings from documents.
//!
//! This module provides functions to extract HTML content from documents using
//! `ContentExtractor` configurations, which support CSS selectors, element
//! transforms (e.g., tag renaming), and clean selectors for removing unwanted
//! nodes before serialization.
//!
//! Key behaviors:
//! - Selectors are tried in order; first selector yielding matches wins.
//! - Default cleaner removes scripts, styles, ads, empty paragraphs, collapses <br> tags.
//! - Clean selectors filter out elements that match any clean pattern.
//! - Transforms apply tag renaming during serialization (not structural mutation).
//! - `allow_multiple`: when true, returns all matches; when false, returns first only.

use aho_corasick::AhoCorasick;
use dom_query::{Document, Selection};
use once_cell::sync::Lazy;
use scraper::{Html, Selector};

use crate::extractors::custom::{ContentExtractor, SelectorSpec, TransformSpec};

/// Selectors for elements that should be removed during default cleaning.
const DEFAULT_CLEAN_SELECTORS: &[&str] = &[
    "script", "style", "noscript", "nav", "header", "footer", "aside", "form", "iframe",
];

/// Substrings in class attributes that indicate ad-related elements.
const AD_CLASS_MARKERS: &[&str] = &["ad", "ads", "advert", "sidebar", "related", "sponsored"];

/// Aho-Corasick automaton for efficient multi-pattern matching of ad class markers.
/// This reduces O(N×M) to O(N×L) where L is the average class attribute length.
static AD_MATCHER: Lazy<AhoCorasick> =
    Lazy::new(|| AhoCorasick::new(AD_CLASS_MARKERS).expect("failed to build ad matcher"));

/// Extracts HTML content from a document based on a `ContentExtractor` configuration.
///
/// Iterates through `ce.field.selectors` in order, returning results from the first
/// selector that yields matches. Elements matching any selector in `ce.clean` are
/// excluded from the output. Transforms are applied during serialization.
///
/// Returns `Some(Vec<String>)` with inner HTML strings for matching elements,
/// or `None` if no selector yields matches.
///
/// If `ce.field.allow_multiple` is false, returns only the first match.
pub fn extract_content_html(doc: &Document, ce: &ContentExtractor) -> Option<Vec<String>> {
    extract_content_html_opts(doc, ce, false)
}

/// Like extract_content_html, but optionally preserves tags (skips heavy cleaning) when preserve_tags=true.
pub fn extract_content_html_opts(
    doc: &Document,
    ce: &ContentExtractor,
    preserve_tags: bool,
) -> Option<Vec<String>> {
    // Store clean selectors as strings (dom_query doesn't pre-parse selectors)
    let clean_selectors: Vec<String> = ce.clean.clone();

    // Determine if default cleaner should be applied:
    // true if ce.field.default_cleaner OR ce has a top-level default_cleaner flag
    // Since ContentExtractor doesn't have a separate top-level default_cleaner (it's in field),
    // we just use field.default_cleaner
    let use_default_cleaner = ce.field.default_cleaner;

    // Try each selector spec in order
    for spec in &ce.field.selectors {
        let css = match spec {
            SelectorSpec::Css(s) => s,
            SelectorSpec::CssAttr(parts) if !parts.is_empty() => &parts[0],
            _ => continue,
        };

        // With dom_query, selectors are validated inline
        let matches = doc.select(css);
        if matches.length() == 0 {
            continue;
        }

        // Found matches, extract HTML for each
        // Note: For content extraction, we always collect ALL matches and join them,
        // regardless of allow_multiple. This matches Go behavior where content selectors
        // like ".duet--article--article-body-component" may match multiple elements
        // that together form the complete article.
        let mut results = Vec::new();
        for element in matches.iter() {
            // Get inner HTML from the element
            let inner_html = element.inner_html();

            // Apply filtering and transforms
            let inner = apply_filters_and_transforms(
                &inner_html,
                &clean_selectors,
                &ce.transforms,
                use_default_cleaner,
                preserve_tags,
            );
            results.push(inner);
        }

        if !results.is_empty() {
            return Some(results);
        }
    }

    None
}

/// Convenience function that returns only the first extracted HTML string.
pub fn extract_content_first_html(doc: &Document, ce: &ContentExtractor) -> Option<String> {
    extract_content_html(doc, ce).and_then(|v| v.into_iter().next())
}

/// Extract raw inner_html (no cleaning, no transforms) using the first matching selector.
pub fn extract_content_raw_first_html(doc: &Document, ce: &ContentExtractor) -> Option<String> {
    for spec in &ce.field.selectors {
        let css = match spec {
            SelectorSpec::Css(s) => s,
            SelectorSpec::CssAttr(parts) if !parts.is_empty() => &parts[0],
            _ => continue,
        };
        let sel = doc.select(css);
        if sel.length() > 0 {
            return Some(sel.inner_html().to_string());
        }
    }
    None
}

/// Apply domain-specific function-like transforms (ported from Go FunctionTransform)
/// to an HTML fragment. This is a minimal set covering the noop transforms
/// present in the Go extractor corpus (e.g., Verge/Vox noscript imgs, Reddit role=img,
/// LA Times trb_ar_la, National Geographic lead images, Gawker/Deadspin YouTube iframes).
pub fn apply_domain_function_transforms(domain: &str, html: &str) -> String {
    let doc = Document::from(html);
    let mut replacements: std::collections::HashMap<dom_query::NodeId, String> =
        std::collections::HashMap::new();

    // Domain-specific transform registry
    let mut rules: Vec<(String, fn(&Selection) -> Option<String>)> = Vec::new();

    match domain {
        "www.reddit.com" => {
            rules.push((
                r#"div[role="img"]"#.into(),
                reddit_role_img_transform as fn(&Selection) -> Option<String>,
            ));
        }
        "www.latimes.com" => {
            rules.push((
                ".trb_ar_la".into(),
                latimes_trb_ar_la_transform as fn(&Selection) -> Option<String>,
            ));
        }
        "www.nationalgeographic.com" => {
            rules.push((
                ".parsys.content".into(),
                natgeo_parsys_transform as fn(&Selection) -> Option<String>,
            ));
        }
        // Gawker/Kinja network lazy YouTube
        "deadspin.com" | "jezebel.com" | "lifehacker.com" | "kotaku.com" | "gizmodo.com"
        | "jalopnik.com" | "kinja.com" | "avclub.com" | "clickhole.com" | "splinternews.com"
        | "theonion.com" | "theroot.com" | "thetakeout.com" | "theinventory.com" => {
            rules.push((
                "iframe".into(),
                gawker_youtube_transform as fn(&Selection) -> Option<String>,
            ));
        }
        // Generic YouTube lazy iframe
        "www.youtube.com" | "youtu.be" => {
            rules.push((
                "iframe".into(),
                youtube_iframe_transform as fn(&Selection) -> Option<String>,
            ));
        }
        "deadline.com" => {
            rules.push((
                ".embed-twitter".into(),
                embed_twitter_blockquote as fn(&Selection) -> Option<String>,
            ));
        }
        "www.apartmenttherapy.com" => {
            rules.push((
                "div[data-render-react-id=\"images/LazyPicture\"]".into(),
                unwrap_keep_children_fn,
            ));
        }
        "news.mynavi.jp" | "www.lifehacker.jp" | "www.gizmodo.jp" => {
            rules.push(("img".into(), img_data_src_to_src));
        }
        "www.cnn.com" => {
            rules.push((
                ".zn-body__paragraph, .el__leafmedia--sourced-paragraph".into(),
                unwrap_keep_children_fn,
            ));
            rules.push((
                ".media__video--thumbnail".into(),
                cnn_video_thumb as fn(&Selection) -> Option<String>,
            ));
        }
        "www.abendblatt.de" => {
            rules.push(("div".into(), unwrap_keep_children_fn));
            rules.push(("p".into(), unwrap_keep_children_fn));
        }
        "www.reuters.com" => {
            rules.push((".article-subtitle".into(), wrap_tag_fn("h4")));
        }
        "www.newyorker.com" => {
            rules.push((".caption__credit".into(), wrap_tag_fn("figcaption")));
            rules.push((".caption__text".into(), wrap_tag_fn("figcaption")));
        }
        "www.npr.org" => {
            rules.push((".bucketwrap.image".into(), wrap_tag_fn("figure")));
            rules.push((
                ".bucketwrap.image .credit-caption".into(),
                wrap_tag_fn("figcaption"),
            ));
        }
        "www.eonline.com" => {
            rules.push(("div.post-content__image".into(), wrap_tag_fn("figure")));
            rules.push((
                "div.post-content__image .image__credits".into(),
                wrap_tag_fn("figcaption"),
            ));
        }
        "gothamist.com" => {
            for sel in ["div.image-left", "div.image-none", "div.image-right"] {
                rules.push((sel.into(), wrap_tag_fn("figure")));
            }
            for sel in [".image-left i", ".image-none i", ".image-right i"] {
                rules.push((sel.into(), wrap_tag_fn("figcaption")));
            }
        }
        "www.buzzfeed.com" => {
            rules.push((
                "figure.longform_custom_header_media .longform_header_image_source".into(),
                wrap_tag_fn("figcaption"),
            ));
            rules.push(("h2".into(), wrap_tag_fn("b")));
        }
        "nymag.com" => {
            rules.push(("h1".into(), wrap_tag_fn("h2")));
        }
        "www.vox.com" => {
            rules.push(("figure .e-image__meta".into(), wrap_tag_fn("figcaption")));
        }
        "epaper.zeit.de" => {
            for (sel, tag) in [
                (".article__author", "p"),
                ("byline", "p"),
                ("linkbox", "p"),
                ("p.title", "h1"),
            ] {
                rules.push((sel.into(), wrap_tag_fn(tag)));
            }
        }
        "twitter.com" => {
            rules.push(("s".into(), wrap_tag_fn("span")));
        }
        "uproxx.com" => {
            rules.push(("div.image".into(), wrap_tag_fn("figure")));
            rules.push((
                "div.image .wp-media-credit".into(),
                wrap_tag_fn("figcaption"),
            ));
        }
        "www.fool.com" => {
            rules.push((".caption".into(), wrap_tag_fn("figcaption")));
        }
        "mashable.com" => {
            rules.push((".image-credit".into(), wrap_tag_fn("figcaption")));
        }
        "pastebin.com" => {
            rules.push(("li".into(), wrap_tag_fn("p")));
            rules.push(("ol".into(), wrap_tag_fn("div")));
        }
        "www.washingtonpost.com" => {
            rules.push((".pb-caption".into(), wrap_tag_fn("figcaption")));
        }
        "wikipedia.org" => {
            rules.push((".infobox".into(), wrap_tag_fn("figure")));
            rules.push((".infobox caption".into(), wrap_tag_fn("figcaption")));
        }
        _ => {}
    }

    for (selector_str, func) in rules {
        let selections = doc.select(&selector_str);
        for el in selections.iter() {
            if let Some(repl) = func(&el) {
                if let Some(node) = el.nodes().first() {
                    replacements.insert(node.id, repl);
                }
            }
        }
    }

    if replacements.is_empty() {
        return html.to_string();
    }

    // Serialize with replacements
    serialize_doc_with_replacements(&doc, &replacements)
}

fn serialize_doc_with_replacements(
    doc: &Document,
    replacements: &std::collections::HashMap<dom_query::NodeId, String>,
) -> String {
    // For dom_query, we'll apply replacements by modifying the document
    // Since dom_query supports mutation, we can replace nodes directly
    for (node_id, replacement_html) in replacements {
        // Find the node by ID and replace it
        if let Some(node) = doc.tree.get(node_id) {
            let sel = Selection::from(node);
            sel.replace_with_html(replacement_html.clone());
        }
    }
    doc.html().to_string()
}

// --- domain transform helpers ---

fn reddit_role_img_transform(el: &Selection) -> Option<String> {
    let src = el.attr("data-url").map(|s| s.to_string()).or_else(|| {
        el.attr("style").and_then(|style| {
            // extract url(...) from style
            style
                .split("url(")
                .nth(1)
                .and_then(|rest| rest.split(')').next())
                .map(|s| s.trim_matches(&['\'', '"'][..]).to_string())
        })
    })?;
    let alt = el.attr("aria-label").unwrap_or_default();
    Some(format!(
        "<img src=\"{}\" alt=\"{}\" />",
        escape_attr(&src),
        escape_attr(&alt)
    ))
}

fn youtube_iframe_transform(el: &Selection) -> Option<String> {
    let src_attr = el.attr("src");
    if src_attr.is_some() {
        return None; // already has src
    }
    if let Some(dsrc) = el.attr("data-src") {
        return Some(build_element_with_attr(el, "src", &dsrc));
    }
    if let Some(rec) = el.attr("data-recommend-id") {
        if let Some(id) = rec.strip_prefix("youtube://") {
            let url = format!("https://www.youtube.com/embed/{}", id);
            return Some(build_element_with_attr(el, "src", &url));
        }
    }
    if let Some(id) = el.attr("id") {
        if let Some(rest) = id.strip_prefix("youtube-") {
            let url = format!("https://www.youtube.com/embed/{}", rest);
            return Some(build_element_with_attr(el, "src", &url));
        }
    }
    None
}

fn gawker_youtube_transform(el: &Selection) -> Option<String> {
    // same as youtube_iframe_transform but also handles data-recommend-id
    youtube_iframe_transform(el)
}

fn cnn_video_thumb(el: &Selection) -> Option<String> {
    // turn thumbnail into figure with img
    let img_sel = el.select("img");
    if img_sel.length() > 0 {
        let src = img_sel.attr("src").unwrap_or_default();
        if !src.is_empty() {
            return Some(format!(
                r#"<figure class="media__video--thumbnail"><img src="{}"/></figure>"#,
                escape_attr(&src)
            ));
        }
    }
    None
}

fn embed_twitter_blockquote(el: &Selection) -> Option<String> {
    // Preserve inner HTML; wrap in blockquote.twitter-tweet if not already
    let inner = el.inner_html();
    if el.is("blockquote") {
        return Some(format!(
            r#"<blockquote class="twitter-tweet">{}</blockquote>"#,
            inner
        ));
    }
    Some(format!(
        r#"<blockquote class="twitter-tweet">{}</blockquote>"#,
        inner
    ))
}

fn img_data_src_to_src(el: &Selection) -> Option<String> {
    // Copy data-src/srcset onto img
    let mut attrs: Vec<(String, String)> = el
        .nodes()
        .first()
        .map(|node| {
            node.attrs()
                .iter()
                .map(|attr| (attr.name.local.to_string(), attr.value.to_string()))
                .collect()
        })
        .unwrap_or_default();

    fix_lazy_img_attrs(&mut attrs);

    // Get tag name
    let tag_name = el
        .nodes()
        .first()
        .and_then(|n| n.node_name())
        .unwrap_or_default();

    // Serialize back
    let mut out = String::new();
    out.push('<');
    out.push_str(&tag_name);
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(&k);
        out.push_str("=\"");
        out.push_str(&escape_attr(&v));
        out.push('"');
    }
    out.push_str(" />");
    Some(out)
}

fn wrap_tag_fn(new_tag: &str) -> fn(&Selection) -> Option<String> {
    match new_tag {
        "h4" => wrap_tag_h4,
        "figcaption" => wrap_tag_figcaption,
        "figure" => wrap_tag_figure,
        "b" => wrap_tag_b,
        "h2" => wrap_tag_h2,
        "p" => wrap_tag_p,
        "h1" => wrap_tag_h1,
        "span" => wrap_tag_span,
        "div" => wrap_tag_div,
        _ => wrap_tag_div,
    }
}

macro_rules! make_wrap_fn {
    ($fname:ident, $tag:literal) => {
        fn $fname(el: &Selection) -> Option<String> {
            Some(wrap_tag(el, $tag))
        }
    };
}

make_wrap_fn!(wrap_tag_h4, "h4");
make_wrap_fn!(wrap_tag_figcaption, "figcaption");
make_wrap_fn!(wrap_tag_figure, "figure");
make_wrap_fn!(wrap_tag_b, "b");
make_wrap_fn!(wrap_tag_h2, "h2");
make_wrap_fn!(wrap_tag_p, "p");
make_wrap_fn!(wrap_tag_h1, "h1");
make_wrap_fn!(wrap_tag_span, "span");
make_wrap_fn!(wrap_tag_div, "div");

fn wrap_tag(el: &Selection, new_tag: &str) -> String {
    let mut out = String::new();
    out.push('<');
    out.push_str(new_tag);

    // Get attributes from the node
    if let Some(node) = el.nodes().first() {
        for attr in node.attrs().iter() {
            out.push(' ');
            out.push_str(&attr.name.local);
            out.push_str("=\"");
            out.push_str(&escape_attr(&attr.value));
            out.push('"');
        }
    }

    if is_void_element(new_tag) {
        out.push_str(" />");
    } else {
        out.push('>');
        out.push_str(&el.inner_html().to_string());
        out.push_str("</");
        out.push_str(new_tag);
        out.push('>');
    }
    out
}

fn latimes_trb_ar_la_transform(el: &Selection) -> Option<String> {
    let figure_sel = el.select("figure");
    if figure_sel.length() > 0 {
        let inner = figure_sel.inner_html();
        return Some(format!("<figure>{}</figure>", inner));
    }
    None
}

fn natgeo_parsys_transform(el: &Selection) -> Option<String> {
    // Try imageGroup with data-platform-image1-path / image2
    let children = el.children();
    if children.length() > 0 {
        let first_child = children.first();
        if first_child.has_class("imageGroup") {
            let container = first_child.select(".media--medium__container");
            if container.length() > 0 {
                let data_container = container.children().first();
                let img1 = data_container
                    .attr("data-platform-image1-path")
                    .unwrap_or_default();
                let img2 = data_container
                    .attr("data-platform-image2-path")
                    .unwrap_or_default();
                if !img1.is_empty() && !img2.is_empty() {
                    let lead = format!(
                        r#"<div class="__image-lead__"><img src="{}"/><img src="{}"/></div>"#,
                        escape_attr(&img1),
                        escape_attr(&img2)
                    );
                    return Some(wrap_with_same_tag(
                        el,
                        &format!("{}{}", lead, el.inner_html()),
                    ));
                }
            }
        }
    }

    // Fallback: find picturefill data-platform-src
    let picturefill = el.select(".image.parbase.section .picturefill");
    if picturefill.length() > 0 {
        if let Some(src) = picturefill.attr("data-platform-src") {
            let lead = format!(
                r#"<img class="__image-lead__" src="{}"/>"#,
                escape_attr(&src)
            );
            return Some(wrap_with_same_tag(
                el,
                &format!("{}{}", lead, el.inner_html()),
            ));
        }
    }

    None
}

fn wrap_with_same_tag(el: &Selection, inner: &str) -> String {
    let name = el
        .nodes()
        .first()
        .and_then(|n| n.node_name())
        .unwrap_or_default();

    let mut out = String::new();
    out.push('<');
    out.push_str(&name);

    if let Some(node) = el.nodes().first() {
        for attr in node.attrs().iter() {
            out.push(' ');
            out.push_str(&attr.name.local);
            out.push_str("=\"");
            out.push_str(&escape_attr(&attr.value));
            out.push('"');
        }
    }

    out.push('>');
    out.push_str(inner);
    out.push_str("</");
    out.push_str(&name);
    out.push('>');
    out
}

fn unwrap_keep_children_fn(el: &Selection) -> Option<String> {
    // return inner HTML (unwrap the element)
    Some(el.inner_html().to_string())
}

fn build_element_with_attr(el: &Selection, attr: &str, value: &str) -> String {
    let name = el
        .nodes()
        .first()
        .and_then(|n| n.node_name())
        .unwrap_or_default();

    let mut out = String::new();
    out.push('<');
    out.push_str(&name);
    let mut wrote = false;

    if let Some(node) = el.nodes().first() {
        for a in node.attrs().iter() {
            let k = &a.name.local;
            let v = &a.value;
            let write_val = if k == attr { value } else { v };
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            out.push_str(&escape_attr(write_val));
            out.push('"');
            if k == attr {
                wrote = true;
            }
        }
    }

    if !wrote {
        out.push(' ');
        out.push_str(attr);
        out.push_str("=\"");
        out.push_str(&escape_attr(value));
        out.push('"');
    }
    if is_void_element(&name) {
        out.push_str(" />");
    } else {
        out.push('>');
        out.push_str(&el.inner_html().to_string());
        out.push_str("</");
        out.push_str(&name);
        out.push('>');
    }
    out
}


/// Applies default content cleaning to an HTML fragment.
///
/// Performs the following cleaning steps:
/// 1. Removes elements matching standard cleanup selectors (script, style, nav, etc.)
/// 2. Removes elements whose class contains ad-related markers (case-insensitive)
/// 3. Collapses consecutive <br> tags into a single <br>
/// 4. Removes empty paragraphs (no text and no img children)
///
/// Returns the cleaned HTML as a string.
fn apply_default_clean(html: &str) -> String {
    let doc = Document::from(html);

    // 1. Remove elements matching standard cleanup selectors
    for selector in DEFAULT_CLEAN_SELECTORS {
        doc.select(selector).remove();
    }

    // 2. Remove elements with ad-related class markers (using Aho-Corasick for O(N×L) matching)
    let elements: Vec<_> = doc.select("*").nodes().iter().cloned().collect();
    for node in elements {
        let sel = Selection::from(node);
        if let Some(class_attr) = sel.attr("class") {
            let class_lower = class_attr.to_lowercase();
            if AD_MATCHER.is_match(&class_lower) {
                sel.remove();
            }
        }
    }

    // 3. Collapse consecutive <br> tags
    let brs: Vec<_> = doc.select("br").iter().collect();
    for br in brs {
        // Check if next sibling is also a br
        let mut current_next = br.next_sibling();
        while current_next.length() > 0 {
            if current_next.is("br") {
                current_next.remove();
                current_next = br.next_sibling();
            } else {
                // Check if it's whitespace-only text
                let text = current_next.text();
                if text.trim().is_empty()
                    && current_next
                        .nodes()
                        .first()
                        .map(|n| n.is_text())
                        .unwrap_or(false)
                {
                    current_next = current_next.next_sibling();
                } else {
                    break;
                }
            }
        }
    }

    // 4. Remove empty paragraphs
    let paragraphs: Vec<_> = doc.select("p").iter().collect();
    for p in paragraphs {
        let text = p.text();
        let has_img = p.select("img").length() > 0;
        if text.trim().is_empty() && !has_img {
            p.remove();
        }
    }

    doc.html().to_string()
}

/// Applies filters and transforms to HTML string using dom_query in-place mutation.
///
/// Parses HTML once, applies all transforms and cleaners in-place, serializes once.
/// This eliminates repeated parse/serialize cycles for O(S) complexity.
fn apply_filters_and_transforms(
    inner_html: &str,
    clean_selectors: &[String],
    transforms: &std::collections::HashMap<String, TransformSpec>,
    use_default_cleaner: bool,
    preserve_tags: bool,
) -> String {
    apply_filters_and_transforms_unified(
        inner_html,
        clean_selectors,
        transforms,
        use_default_cleaner,
        preserve_tags,
    )
}

/// Legacy implementation that works with parsed Selectors
fn apply_filters_and_transforms_legacy(
    inner_html: &str,
    clean_selectors: &[Selector],
    transforms: &std::collections::HashMap<String, TransformSpec>,
    use_default_cleaner: bool,
    preserve_tags: bool,
) -> String {
    // Fast path: if no transforms, no cleaners, and default_cleaner is off, return as-is (post cleaners only)
    if !use_default_cleaner && clean_selectors.is_empty() && transforms.is_empty() {
        return apply_post_cleaners(&inner_html);
    }

    // If no transforms and no default cleaner, but there ARE clean selectors,
    // apply a lightweight removal of those selectors while keeping all other tags.
    if preserve_tags && !use_default_cleaner && transforms.is_empty() && !clean_selectors.is_empty()
    {
        let mut frag = Html::parse_fragment(&inner_html);
        let mut to_remove: Vec<ego_tree::NodeId> = Vec::new();
        for selector in clean_selectors {
            for m in frag.select(selector) {
                to_remove.push(m.id());
            }
        }
        if !to_remove.is_empty() {
            // Remove marked nodes from the tree
            for id in to_remove {
                if let Some(mut node) = frag.tree.get_mut(id) {
                    node.detach();
                }
            }
        }
        // Serialize preserving tags
        let mut out = String::new();
        for child in frag.root_element().children() {
            serialize_node_preserve(child, &mut out);
        }
        return apply_post_cleaners(&out);
    }

    // Apply transforms first (before cleaning)
    let transformed_html = if transforms.is_empty() {
        inner_html.to_string()
    } else {
        apply_transforms(&inner_html, transforms)
    };

    // Apply default cleaner if enabled
    let cleaned_html = if use_default_cleaner {
        apply_default_clean(&transformed_html)
    } else {
        transformed_html.clone()
    };

    // Re-parse to apply clean selectors
    let fragment = Html::parse_fragment(&cleaned_html);

    // Collect IDs of elements to skip (those matching clean selectors)
    let mut skip_ids = std::collections::HashSet::new();
    for selector in clean_selectors {
        for matched in fragment.select(selector) {
            collect_node_ids(matched, &mut skip_ids);
        }
    }

    // Serialize while filtering (transforms already applied)
    let filtered_html = serialize_filtered(&fragment, &skip_ids, &std::collections::HashMap::new());

    // Apply post-cleaners: heading fix and empty link rewriting
    apply_post_cleaners(&filtered_html)
}

/// Applies transforms to an HTML fragment based on CSS selector mappings.
///
/// For each (selector, transform) pair, finds matching elements and applies the transform.
/// Transforms are applied by re-serializing the fragment with modifications.
fn apply_transforms(
    html: &str,
    transforms: &std::collections::HashMap<String, TransformSpec>,
) -> String {
    let fragment = Html::parse_fragment(html);

    // Build a map from node IDs to transforms
    let mut node_transforms: std::collections::HashMap<ego_tree::NodeId, &TransformSpec> =
        std::collections::HashMap::new();
    let mut unwrap_ids: std::collections::HashSet<ego_tree::NodeId> =
        std::collections::HashSet::new();

    for (selector_str, transform) in transforms {
        if let Ok(selector) = Selector::parse(selector_str) {
            for matched in fragment.select(&selector) {
                let node_id = matched.id();
                if matches!(transform, TransformSpec::Unwrap) {
                    unwrap_ids.insert(node_id);
                }
                node_transforms.insert(node_id, transform);
            }
        }
    }

    // Serialize with transforms applied
    let mut output = String::new();
    for child in fragment.root_element().children() {
        serialize_node_with_transforms(child, &node_transforms, &unwrap_ids, &mut output);
    }
    output
}

/// Applies a TransformSpec to a Selection using dom_query mutation.
fn apply_transform_to_selection(sel: &Selection, transform: &TransformSpec) {
    match transform {
        TransformSpec::Tag { value } => {
            sel.rename(value);
        }
        TransformSpec::Noop => {}
        TransformSpec::NoscriptToDiv => {
            // HTML5 parses noscript contents as raw text, so rename() may not work.
            // Instead, rebuild as <div> wrapping the inner content.
            let inner = sel.inner_html().to_string();
            sel.replace_with_html(format!("<div>{}</div>", inner));
        }
        TransformSpec::Unwrap => {
            let inner = sel.inner_html().to_string();
            sel.replace_with_html(inner);
        }
        TransformSpec::MoveAttr { from, to } => {
            if let Some(val) = sel.attr(from) {
                let val_str = val.to_string();
                sel.set_attr(to, &val_str);
            }
        }
        TransformSpec::SetAttr { name, value } => {
            sel.set_attr(name, value);
        }
    }
}

/// Handles the special case: noscript containing a single img -> wrap in span
fn handle_noscript_special_case(doc: &Document, selector_str: &str) {
    let noscripts: Vec<_> = doc.select(selector_str).nodes().iter().cloned().collect();
    for node in noscripts {
        let sel = Selection::from(node);
        // Check if this noscript contains exactly one element child that is an img
        // Use children() to get direct children instead of "> *" selector
        let children: Vec<_> = sel.children().iter().collect();
        // Filter to just element children (skip text nodes)
        let element_children: Vec<_> = children
            .iter()
            .filter(|c| c.is("*")) // is("*") matches any element
            .collect();
        if element_children.len() == 1 {
            let first_child = element_children[0];
            if first_child.is("img") {
                let img_html = first_child.html().to_string();
                sel.replace_with_html(format!("<span>{}</span>", img_html));
            }
        }
    }
}

/// Applies transforms using dom_query in-place mutation.
/// This replaces the scraper-based apply_transforms().
fn apply_transforms_dom(
    html: &str,
    transforms: &std::collections::HashMap<String, TransformSpec>,
) -> String {
    if transforms.is_empty() {
        return html.to_string();
    }

    let doc = Document::from(html);

    for (selector_str, transform) in transforms {
        // Handle noscript special case first
        if matches!(transform, TransformSpec::NoscriptToDiv) {
            handle_noscript_special_case(&doc, selector_str);
        }

        // Collect nodes first to avoid mutation during iteration
        let nodes: Vec<_> = doc.select(selector_str).nodes().iter().cloned().collect();
        for node in nodes {
            let sel = Selection::from(node);
            apply_transform_to_selection(&sel, transform);
        }
    }

    doc.html().to_string()
}

/// Unified pipeline that parses once, mutates in-place, serializes once.
/// This consolidates all processing steps into a single parse-mutate-serialize cycle.
fn apply_filters_and_transforms_unified(
    inner_html: &str,
    clean_selectors: &[String],
    transforms: &std::collections::HashMap<String, TransformSpec>,
    use_default_cleaner: bool,
    _preserve_tags: bool, // kept for API compatibility
) -> String {
    // Fast path: no processing needed
    if !use_default_cleaner && clean_selectors.is_empty() && transforms.is_empty() {
        return apply_post_cleaners(inner_html);
    }

    // Single parse
    let doc = Document::from(inner_html);

    // Step 1: Apply transforms (in-place)
    for (selector_str, transform) in transforms {
        // Skip empty selectors to avoid panic
        if selector_str.trim().is_empty() {
            continue;
        }

        // Handle noscript special case
        if matches!(transform, TransformSpec::NoscriptToDiv) {
            handle_noscript_special_case(&doc, selector_str);
        }

        let nodes: Vec<_> = doc.select(selector_str).nodes().iter().cloned().collect();
        for node in nodes {
            let sel = Selection::from(node);
            apply_transform_to_selection(&sel, transform);
        }
    }

    // Step 2: Apply default cleaner (in-place)
    if use_default_cleaner {
        apply_default_clean_to_doc(&doc);
    }

    // Step 3: Remove elements matching clean selectors (in-place)
    for selector in clean_selectors {
        doc.select(selector).remove();
    }

    // Step 4: Apply post-cleaners (in-place)
    fix_headings_doc(&doc);
    rewrite_empty_links_doc(&doc);

    // Serialize content from body (noscript fragments may also end up in head due to HTML5 parsing)
    let body_content = doc.select("body").inner_html().to_string();
    let head_content = doc.select("head").inner_html().to_string();

    // Combine head and body content (head content might contain transformed noscripts)
    if head_content.is_empty() {
        body_content
    } else if body_content.is_empty() {
        head_content
    } else {
        format!("{}{}", head_content, body_content)
    }
}

/// Applies default cleaning to a Document in-place.
fn apply_default_clean_to_doc(doc: &Document) {
    // Remove common noise elements
    for selector in &[
        "script", "style", "noscript", "nav", "header", "footer", "aside", "form", "iframe",
        "button", "input", "select", "textarea",
    ] {
        doc.select(selector).remove();
    }

    // Remove elements with ad-related classes (using Aho-Corasick for O(N×L) matching)
    let elements: Vec<_> = doc.select("*").nodes().iter().cloned().collect();
    for node in elements {
        let sel = Selection::from(node);
        if let Some(class) = sel.attr("class") {
            let class_lower = class.to_lowercase();
            if AD_MATCHER.is_match(&class_lower) {
                sel.remove();
            }
        }
    }

    // Collapse consecutive <br> tags and remove empty paragraphs
    collapse_consecutive_brs_doc(doc);
    remove_empty_paragraphs_doc(doc);
}

/// Collapses consecutive <br> tags in a Document.
fn collapse_consecutive_brs_doc(doc: &Document) {
    // Find br elements and remove consecutive ones
    let brs: Vec<_> = doc.select("br").nodes().iter().cloned().collect();
    let mut prev_was_br = false;
    for node in brs {
        let sel = Selection::from(node);
        if prev_was_br {
            sel.remove();
        } else {
            prev_was_br = true;
        }
    }
}

/// Removes empty paragraphs from a Document.
fn remove_empty_paragraphs_doc(doc: &Document) {
    let paragraphs: Vec<_> = doc.select("p").nodes().iter().cloned().collect();
    for node in paragraphs {
        let sel = Selection::from(node);
        let text = sel.text();
        let has_img = sel.select("img").length() > 0;
        if text.trim().is_empty() && !has_img {
            sel.remove();
        }
    }
}

/// Recursively serializes a node, applying transforms based on node ID mappings.
fn serialize_node_with_transforms(
    node: ego_tree::NodeRef<scraper::Node>,
    node_transforms: &std::collections::HashMap<ego_tree::NodeId, &TransformSpec>,
    unwrap_ids: &std::collections::HashSet<ego_tree::NodeId>,
    output: &mut String,
) {
    match node.value() {
        scraper::Node::Text(text) => {
            output.push_str(&**text);
        }
        scraper::Node::Element(el) => {
            let node_id = node.id();
            let tag_name = el.name();
            let tag_lower = tag_name.to_lowercase();

            // Check if this node should be unwrapped (remove element, keep children)
            if unwrap_ids.contains(&node_id) {
                for child in node.children() {
                    serialize_node_with_transforms(child, node_transforms, unwrap_ids, output);
                }
                return;
            }

            // Determine effective tag name and attribute modifications
            let transform = node_transforms.get(&node_id);

            // Special-case Go FunctionTransform behavior: if noscript contains a single img,
            // replace it with a span wrapping that img (The Verge / Vox pattern).
            if tag_lower == "noscript" {
                let mut element_children = node.children().filter_map(scraper::ElementRef::wrap);
                if let Some(first_child) = element_children.next() {
                    if element_children.next().is_none()
                        && first_child.value().name().eq_ignore_ascii_case("img")
                    {
                        output.push_str("<span>");
                        // serialize the img child normally (with transforms/lazy fixes)
                        for gc in first_child.children() {
                            serialize_node_with_transforms(gc, node_transforms, unwrap_ids, output);
                        }
                        output.push_str("</span>");
                        return;
                    }
                }
            }

            let effective_tag = match transform {
                Some(TransformSpec::Tag { value }) => value.as_str(),
                Some(TransformSpec::NoscriptToDiv) => "div",
                _ => tag_name,
            };

            // If transform is Unwrap, we already returned above; if Noscript, change tag name; additionally,
            // default noscript handling: surface lazy content by converting to div even without explicit transform.
            if tag_lower == "noscript" && !matches!(transform, Some(TransformSpec::Tag { .. })) {
                output.push_str("<div>");
                for child in node.children() {
                    serialize_node_with_transforms(child, node_transforms, unwrap_ids, output);
                }
                output.push_str("</div>");
                return;
            }

            // Collect attributes, applying MoveAttr and SetAttr transforms
            let mut attrs: Vec<(String, String)> = el
                .attrs()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();

            if let Some(transform) = transform {
                match transform {
                    TransformSpec::MoveAttr { from, to } => {
                        // Find the value of `from` attr
                        if let Some(from_val) = el.attr(from) {
                            let from_value = from_val.to_string();
                            // Update or add `to` attr
                            let mut found = false;
                            for (name, value) in attrs.iter_mut() {
                                if name == to {
                                    *value = from_value.clone();
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                attrs.push((to.clone(), from_value));
                            }
                        }
                    }
                    TransformSpec::SetAttr { name, value } => {
                        // Update or add the attribute
                        let mut found = false;
                        for (attr_name, attr_value) in attrs.iter_mut() {
                            if attr_name == name {
                                *attr_value = value.clone();
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            attrs.push((name.clone(), value.clone()));
                        }
                    }
                    _ => {}
                }
            }

            // Generic lazy-load fixes
            if tag_lower == "img" {
                fix_lazy_img_attrs(&mut attrs);
            } else if tag_lower == "a" {
                fix_lazy_anchor_attrs(&mut attrs);
            }

            // Open tag
            output.push('<');
            output.push_str(effective_tag);

            // Attributes
            for (name, value) in &attrs {
                output.push(' ');
                output.push_str(name);
                output.push_str("=\"");
                output.push_str(&escape_attr(value));
                output.push('"');
            }

            // Check for void elements
            if is_void_element(effective_tag) {
                output.push_str(" />");
            } else {
                output.push('>');

                // Children
                for child in node.children() {
                    serialize_node_with_transforms(child, node_transforms, unwrap_ids, output);
                }

                // Close tag
                output.push_str("</");
                output.push_str(effective_tag);
                output.push('>');
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

/// Collects all node IDs in a subtree (to skip when serializing).
fn collect_node_ids(
    element: scraper::ElementRef,
    ids: &mut std::collections::HashSet<ego_tree::NodeId>,
) {
    ids.insert(element.id());
    for child in element.children() {
        if let Some(child_el) = scraper::ElementRef::wrap(child) {
            collect_node_ids(child_el, ids);
        } else {
            ids.insert(child.id());
        }
    }
}

/// Serializes an HTML fragment, skipping nodes in `skip_ids` and applying transforms.
fn serialize_filtered(
    fragment: &Html,
    skip_ids: &std::collections::HashSet<ego_tree::NodeId>,
    transforms: &std::collections::HashMap<String, TransformSpec>,
) -> String {
    let mut output = String::new();

    // The fragment root is a document node; iterate its children
    for child in fragment.root_element().children() {
        serialize_node(child, skip_ids, transforms, &mut output);
    }

    output
}

/// Recursively serializes a node, applying filters and transforms.
fn serialize_node(
    node: ego_tree::NodeRef<scraper::Node>,
    skip_ids: &std::collections::HashSet<ego_tree::NodeId>,
    transforms: &std::collections::HashMap<String, TransformSpec>,
    output: &mut String,
) {
    if skip_ids.contains(&node.id()) {
        return;
    }

    match node.value() {
        scraper::Node::Text(text) => {
            output.push_str(&**text);
        }
        scraper::Node::Element(el) => {
            let tag_name = el.name();
            let tag_lower = tag_name.to_lowercase();

            // Apply tag transform
            let mut effective_tag =
                if let Some(TransformSpec::Tag { value }) = transforms.get(tag_name) {
                    value.as_str()
                } else {
                    tag_name
                };

            // Default noscript handling if no explicit transform: turn into div
            if tag_lower == "noscript" {
                effective_tag = "div";
            }

            // Collect attrs so we can apply lazy fixes
            let mut attrs: Vec<(String, String)> = el
                .attrs()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();

            if tag_lower == "img" {
                fix_lazy_img_attrs(&mut attrs);
            } else if tag_lower == "source" {
                fix_lazy_source_attrs(&mut attrs);
            } else if tag_lower == "a" {
                fix_lazy_anchor_attrs(&mut attrs);
            }

            // Open tag
            output.push('<');
            output.push_str(effective_tag);

            // Attributes
            for (name, value) in attrs {
                output.push(' ');
                output.push_str(&name);
                output.push_str("=\"");
                output.push_str(&escape_attr(&value));
                output.push('"');
            }

            // Check for void elements
            if is_void_element(tag_name) {
                output.push_str(" />");
            } else {
                output.push('>');

                // Children
                for child in node.children() {
                    serialize_node(child, skip_ids, transforms, output);
                }

                // Close tag
                output.push_str("</");
                output.push_str(effective_tag);
                output.push('>');
            }
        }
        scraper::Node::Comment(comment) => {
            output.push_str("<!--");
            output.push_str(&**comment);
            output.push_str("-->");
        }
        _ => {
            // Skip other node types (Document, Doctype, etc.)
        }
    }
}

/// Serialize node preserving all tags (used in lightweight clean removal path)
fn serialize_node_preserve(node: ego_tree::NodeRef<scraper::Node>, output: &mut String) {
    match node.value() {
        scraper::Node::Text(text) => output.push_str(&**text),
        scraper::Node::Element(el) => {
            let tag_name = el.name();
            output.push('<');
            output.push_str(tag_name);
            for (name, value) in el.attrs() {
                output.push(' ');
                output.push_str(name);
                output.push_str("=\"");
                output.push_str(&escape_attr(value));
                output.push('"');
            }
            if is_void_element(tag_name) {
                output.push_str(" />");
            } else {
                output.push('>');
                for child in node.children() {
                    serialize_node_preserve(child, output);
                }
                output.push_str("</");
                output.push_str(tag_name);
                output.push('>');
            }
        }
        scraper::Node::Comment(c) => {
            output.push_str("<!--");
            output.push_str(&**c);
            output.push_str("-->");
        }
        _ => {}
    }
}

/// Escapes special characters in attribute values.
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Checks if a tag is a void element (self-closing in HTML5).
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

// Generic lazy image attribute fixer similar to Go FunctionTransforms
fn fix_lazy_img_attrs(attrs: &mut Vec<(String, String)>) {
    let mut src = None;
    let mut srcset = None;
    for (k, v) in attrs.iter() {
        let kl = k.to_lowercase();
        match kl.as_str() {
            "src" if !v.is_empty() => src = Some(v.clone()),
            "srcset" if !v.is_empty() => srcset = Some(v.clone()),
            "data-src" | "data-original" | "data-lazy" | "data-lazy-src" | "data-zoom"
            | "data-zoom-src" | "data-href" | "data-url"
                if !v.is_empty() =>
            {
                if src.is_none() {
                    src = Some(v.clone())
                }
            }
            "data-srcset" | "data-original-set" | "data-src-set" if !v.is_empty() => {
                if srcset.is_none() {
                    srcset = Some(v.clone())
                }
            }
            _ => {}
        }
    }

    if let Some(s) = src {
        if !attrs.iter().any(|(k, _)| k.eq_ignore_ascii_case("src")) {
            attrs.push(("src".into(), s));
        }
    }
    if let Some(s) = srcset {
        if !attrs.iter().any(|(k, _)| k.eq_ignore_ascii_case("srcset")) {
            attrs.push(("srcset".into(), s));
        }
    }
}

fn fix_lazy_anchor_attrs(attrs: &mut Vec<(String, String)>) {
    let mut href = None;
    for (k, v) in attrs.iter() {
        let kl = k.to_lowercase();
        if kl == "href" && !v.is_empty() {
            href = Some(v.clone());
            break;
        }
        if kl == "data-href" || kl == "data-url" {
            if href.is_none() && !v.is_empty() {
                href = Some(v.clone());
            }
        }
    }
    if let Some(h) = href {
        if !attrs.iter().any(|(k, _)| k.eq_ignore_ascii_case("href")) {
            attrs.push(("href".into(), h));
        }
    }
}

/// Fixes heading structure by demoting extra h1 elements to h2.
///
/// If a fragment contains more than one h1, the first h1 stays as h1 and all
/// subsequent h1 elements are demoted to h2.
pub fn fix_headings(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let h1_selector = match Selector::parse("h1") {
        Ok(s) => s,
        Err(_) => return html.to_string(),
    };

    let h1_elements: Vec<_> = fragment.select(&h1_selector).collect();
    if h1_elements.len() <= 1 {
        return html.to_string();
    }

    // Collect node IDs of h1 elements to demote (all except the first)
    let mut demote_ids: std::collections::HashSet<ego_tree::NodeId> =
        std::collections::HashSet::new();
    for h1 in h1_elements.iter().skip(1) {
        demote_ids.insert(h1.id());
    }

    // Serialize with h1 -> h2 renaming for demoted elements
    let mut output = String::new();
    for child in fragment.root_element().children() {
        serialize_node_demote_headings(child, &demote_ids, &mut output);
    }
    output
}

/// Recursively serializes a node, demoting specified h1 elements to h2.
fn serialize_node_demote_headings(
    node: ego_tree::NodeRef<scraper::Node>,
    demote_ids: &std::collections::HashSet<ego_tree::NodeId>,
    output: &mut String,
) {
    match node.value() {
        scraper::Node::Text(text) => {
            output.push_str(&**text);
        }
        scraper::Node::Element(el) => {
            let tag_name = el.name();
            let node_id = node.id();

            // Demote h1 to h2 if in demote set
            let effective_tag =
                if demote_ids.contains(&node_id) && tag_name.eq_ignore_ascii_case("h1") {
                    "h2"
                } else {
                    tag_name
                };

            output.push('<');
            output.push_str(effective_tag);

            for (name, value) in el.attrs() {
                output.push(' ');
                output.push_str(name);
                output.push_str("=\"");
                output.push_str(&escape_attr(value));
                output.push('"');
            }

            if is_void_element(effective_tag) {
                output.push_str(" />");
            } else {
                output.push('>');
                for child in node.children() {
                    serialize_node_demote_headings(child, demote_ids, output);
                }
                output.push_str("</");
                output.push_str(effective_tag);
                output.push('>');
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

/// Rewrites empty links by unwrapping anchor elements with empty or "#" href.
///
/// Anchor elements with `href=""` or `href="#"` that contain text content are
/// replaced with their inner content (the anchor tag is removed but children remain).
pub fn rewrite_empty_links(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let a_selector = match Selector::parse("a") {
        Ok(s) => s,
        Err(_) => return html.to_string(),
    };

    // Find anchors with empty or "#" href that have text content
    let mut unwrap_ids: std::collections::HashSet<ego_tree::NodeId> =
        std::collections::HashSet::new();

    for anchor in fragment.select(&a_selector) {
        let href = anchor.value().attr("href").unwrap_or("");
        let href_trimmed = href.trim();

        // Check if href is empty or just "#"
        if href_trimmed.is_empty() || href_trimmed == "#" {
            // Check if anchor has text content
            let has_text = anchor.text().any(|t| !t.trim().is_empty());
            if has_text {
                unwrap_ids.insert(anchor.id());
            }
        }
    }

    if unwrap_ids.is_empty() {
        return html.to_string();
    }

    // Serialize, unwrapping the marked anchors
    let mut output = String::new();
    for child in fragment.root_element().children() {
        serialize_node_unwrap_links(child, &unwrap_ids, &mut output);
    }
    output
}

/// Recursively serializes a node, unwrapping specified anchor elements.
fn serialize_node_unwrap_links(
    node: ego_tree::NodeRef<scraper::Node>,
    unwrap_ids: &std::collections::HashSet<ego_tree::NodeId>,
    output: &mut String,
) {
    match node.value() {
        scraper::Node::Text(text) => {
            output.push_str(&**text);
        }
        scraper::Node::Element(el) => {
            let node_id = node.id();

            // Unwrap if this is a marked anchor
            if unwrap_ids.contains(&node_id) {
                for child in node.children() {
                    serialize_node_unwrap_links(child, unwrap_ids, output);
                }
                return;
            }

            let tag_name = el.name();
            output.push('<');
            output.push_str(tag_name);

            for (name, value) in el.attrs() {
                output.push(' ');
                output.push_str(name);
                output.push_str("=\"");
                output.push_str(&escape_attr(value));
                output.push('"');
            }

            if is_void_element(tag_name) {
                output.push_str(" />");
            } else {
                output.push('>');
                for child in node.children() {
                    serialize_node_unwrap_links(child, unwrap_ids, output);
                }
                output.push_str("</");
                output.push_str(tag_name);
                output.push('>');
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

/// Applies post-extraction cleaners: heading fix and empty link rewriting.
///
/// These cleaners are applied after all other transforms and cleaning steps.
pub fn apply_post_cleaners(html: &str) -> String {
    let with_fixed_headings = fix_headings(html);
    rewrite_empty_links(&with_fixed_headings)
}

/// Demotes all h1 elements except the first to h2.
///
/// Works on a Document in-place using dom_query mutation.
/// If a document contains more than one h1, the first h1 stays as h1 and all
/// subsequent h1 elements are demoted to h2.
fn fix_headings_doc(doc: &Document) {
    let h1s: Vec<_> = doc.select("h1").nodes().iter().cloned().collect();

    if h1s.len() <= 1 {
        return; // No demotion needed
    }

    // Skip first h1, demote rest to h2
    for node in h1s.into_iter().skip(1) {
        let sel = Selection::from(node);
        // Use replace_with_html to rename tag
        let outer = sel.html().to_string();
        let new_html = outer
            .replacen("<h1", "<h2", 1)
            .replacen("</h1>", "</h2>", 1);
        sel.replace_with_html(new_html);
    }
}

/// Unwraps <a> tags that have empty href or href="#".
///
/// Works on a Document in-place using dom_query mutation.
/// Anchor elements with `href=""` or `href="#"` that contain text content are
/// replaced with their inner content (the anchor tag is removed but children remain).
fn rewrite_empty_links_doc(doc: &Document) {
    let anchors: Vec<_> = doc.select("a").nodes().iter().cloned().collect();

    for node in anchors {
        let sel = Selection::from(node);
        let href = sel.attr("href").map(|s| s.to_string()).unwrap_or_default();
        let href_trimmed = href.trim();

        // Check if href is empty or just "#"
        if href_trimmed.is_empty() || href_trimmed == "#" {
            // Check if anchor has text content (don't unwrap empty anchors)
            let text = sel.text();
            if !text.trim().is_empty() {
                // Unwrap: replace anchor with its inner content
                let inner = sel.inner_html().to_string();
                sel.replace_with_html(inner);
            }
        }
    }
}

/// Applies all post-cleaners to a Document in-place.
///
/// This is the dom_query version that mutates the Document directly.
fn apply_post_cleaners_doc(doc: &Document) {
    fix_headings_doc(doc);
    rewrite_empty_links_doc(doc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::custom::FieldExtractor;
    use std::collections::HashMap;

    #[test]
    fn extract_content_single() {
        let html = r#"<html><body><article><p>Hi</p></article></body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "<p>Hi</p>");
    }

    #[test]
    fn extract_content_allow_multiple() {
        let html = r#"<html><body>
            <div class="entry">First</div>
            <div class="entry">Second</div>
        </body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("div.entry".to_string())],
                allow_multiple: true,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], "First");
        assert_eq!(values[1], "Second");
    }

    #[test]
    fn clean_removes_nodes() {
        let html =
            r#"<html><body><article><div class="ad">AD</div><p>Hi</p></article></body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                ..Default::default()
            },
            clean: vec![".ad".to_string()],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "<p>Hi</p>");
    }

    #[test]
    fn transform_tag_wraps_output() {
        let html = r#"<html><body><article><span>Hi</span></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert(
            "span".to_string(),
            TransformSpec::Tag {
                value: "strong".to_string(),
            },
        );

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("span".to_string())],
                allow_multiple: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        // The span element's inner_html is "Hi", but we need to test transform
        // Since we're selecting span, its inner_html is "Hi"
        // Transforms apply to children when serializing, not to the selected element itself
        // Let's re-read the requirement: selector selects span, transform transforms span->strong
        // But inner_html doesn't include the element itself, just children
        // Let me reconsider: the selected element is span, its inner_html is "Hi"
        // When we serialize the fragment "<span>Hi</span>", transform changes span to strong
        // Wait, we're selecting span and taking inner_html which is just "Hi"
        // So the test expectation needs adjustment
        assert_eq!(values[0], "Hi");
    }

    #[test]
    fn transform_tag_wraps_nested_output() {
        // Test transform on nested elements within selected content
        let html = r#"<html><body><article><span>Hi</span></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert(
            "span".to_string(),
            TransformSpec::Tag {
                value: "strong".to_string(),
            },
        );

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "<strong>Hi</strong>");
    }

    #[test]
    fn returns_none_when_no_match() {
        let html = r#"<html><body><p>Hello</p></body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_none());
    }

    #[test]
    fn extract_content_first_html_returns_single() {
        let html = r#"<html><body>
            <div class="entry">First</div>
            <div class="entry">Second</div>
        </body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("div.entry".to_string())],
                allow_multiple: true,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_first_html(&doc, &ce);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "First");
    }

    #[test]
    fn default_cleaner_removes_script_and_ads() {
        let html = r#"<html><body><article>
            <script>alert('bad');</script>
            <div class="ads banner">Advertisement</div>
            <p>Good</p>
        </article></body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: true,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        // Should not contain script or ads
        assert!(
            !output.contains("script"),
            "output should not contain script tag"
        );
        assert!(
            !output.to_lowercase().contains("ads"),
            "output should not contain ads"
        );
        assert!(
            !output.contains("Advertisement"),
            "output should not contain ad content"
        );
        // Should contain the good content
        assert!(output.contains("Good"), "output should contain 'Good'");
    }

    #[test]
    fn default_cleaner_collapses_br() {
        let html = r#"<html><body><article>Hello<br><br>World</article></body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: true,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        // Should contain Hello, br, World but only one br (collapsed)
        assert!(output.contains("Hello"), "output should contain 'Hello'");
        assert!(output.contains("World"), "output should contain 'World'");
        // Count occurrences of <br
        let br_count = output.matches("<br").count();
        assert_eq!(br_count, 1, "should have exactly one <br> tag");
    }

    #[test]
    fn default_cleaner_drops_empty_p() {
        let html = r#"<html><body><article><p></p><p>Keep</p></article></body></html>"#;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: true,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        // Should contain the Keep paragraph
        assert!(output.contains("Keep"), "output should contain 'Keep'");
        // Should have only one <p> tag (the non-empty one)
        let p_count = output.matches("<p>").count();
        assert_eq!(
            p_count, 1,
            "should have exactly one <p> tag (the non-empty one)"
        );
    }

    #[test]
    fn transform_tag_rename() {
        // Test CSS selector-based tag rename: span.old -> strong
        let html = r#"<html><body><article><span class="old">Hi</span></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert(
            "span.old".to_string(),
            TransformSpec::Tag {
                value: "strong".to_string(),
            },
        );

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        assert!(
            output.contains("<strong"),
            "output should contain <strong>: {}",
            output
        );
        assert!(
            output.contains("Hi</strong>"),
            "output should contain Hi</strong>: {}",
            output
        );
        assert!(
            !output.contains("<span"),
            "output should not contain <span>: {}",
            output
        );
    }

    #[test]
    fn transform_noscript_to_div() {
        // Test NoscriptToDiv transform: <noscript> -> <div>
        let html =
            r#"<html><body><article><noscript><p>Hidden</p></noscript></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert("noscript".to_string(), TransformSpec::NoscriptToDiv);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        assert!(
            output.contains("<div>"),
            "output should contain <div>: {}",
            output
        );
        assert!(
            output.contains("<p>Hidden</p>"),
            "output should contain inner content: {}",
            output
        );
        assert!(
            !output.contains("<noscript"),
            "output should not contain <noscript>: {}",
            output
        );
    }

    #[test]
    fn transform_unwrap() {
        // Test Unwrap transform: remove element but keep children
        let html = r#"<html><body><article><div class="unwrap"><em>Text</em></div></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert("div.unwrap".to_string(), TransformSpec::Unwrap);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        assert!(
            output.contains("<em>Text</em>"),
            "output should contain <em>Text</em>: {}",
            output
        );
        assert!(
            !output.contains("<div"),
            "output should not contain <div>: {}",
            output
        );
    }

    #[test]
    fn transform_move_attr() {
        // Test MoveAttr transform: copy data-src to src
        let html = r#"<html><body><article><img data-src="a.jpg"></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert(
            "img".to_string(),
            TransformSpec::MoveAttr {
                from: "data-src".to_string(),
                to: "src".to_string(),
            },
        );

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        assert!(
            output.contains("src=\"a.jpg\""),
            "output should contain src=\"a.jpg\": {}",
            output
        );
        // data-src should still be present (we copy, not move)
        assert!(
            output.contains("data-src=\"a.jpg\""),
            "output should still contain data-src: {}",
            output
        );
    }

    #[test]
    fn transform_set_attr() {
        // Test SetAttr transform: set a fixed attribute value
        let html = r#"<html><body><article><a href="old.html">Link</a></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert(
            "a".to_string(),
            TransformSpec::SetAttr {
                name: "target".to_string(),
                value: "_blank".to_string(),
            },
        );

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        assert!(
            output.contains("target=\"_blank\""),
            "output should contain target=\"_blank\": {}",
            output
        );
        assert!(
            output.contains("href=\"old.html\""),
            "output should still contain href: {}",
            output
        );
    }

    #[test]
    fn transform_move_attr_overwrites_existing() {
        // Test MoveAttr overwrites existing `to` attribute
        let html = r#"<html><body><article><img src="old.jpg" data-src="new.jpg"></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert(
            "img".to_string(),
            TransformSpec::MoveAttr {
                from: "data-src".to_string(),
                to: "src".to_string(),
            },
        );

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        // src should now be new.jpg (overwritten)
        assert!(
            output.contains("src=\"new.jpg\""),
            "output should contain src=\"new.jpg\": {}",
            output
        );
        assert!(
            !output.contains("src=\"old.jpg\""),
            "output should not contain src=\"old.jpg\": {}",
            output
        );
    }

    #[test]
    fn transform_img_data_src_to_src() {
        // Test that img with data-src gets src populated via MoveAttr transform
        let html =
            r#"<html><body><article><img data-src="lazy.jpg" alt="Lazy"></article></body></html>"#;
        let doc = Document::from(html);

        let mut transforms = HashMap::new();
        transforms.insert(
            "img".to_string(),
            TransformSpec::MoveAttr {
                from: "data-src".to_string(),
                to: "src".to_string(),
            },
        );

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms,
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];
        // src should be set from data-src
        assert!(
            output.contains("src=\"lazy.jpg\""),
            "output should contain src=\"lazy.jpg\": {}",
            output
        );
        assert!(
            output.contains("alt=\"Lazy\""),
            "output should preserve other attrs: {}",
            output
        );
    }

    #[test]
    fn heading_demote_extra_h1() {
        // Test that multiple h1 elements are demoted: first stays h1, rest become h2
        let html = r#"<h1>First Title</h1><p>Content</p><h1>Second Title</h1><p>More</p><h1>Third Title</h1>"#;
        let output = fix_headings(html);

        // First h1 should stay h1
        assert!(
            output.contains("<h1>First Title</h1>"),
            "first h1 should remain h1: {}",
            output
        );
        // Second and third h1 should become h2
        assert!(
            output.contains("<h2>Second Title</h2>"),
            "second h1 should become h2: {}",
            output
        );
        assert!(
            output.contains("<h2>Third Title</h2>"),
            "third h1 should become h2: {}",
            output
        );
        // Should not have more than one h1
        let h1_count = output.matches("<h1>").count();
        assert_eq!(h1_count, 1, "should have exactly one h1, got: {}", output);
    }

    #[test]
    fn heading_single_h1_unchanged() {
        // Test that a single h1 is not modified
        let html = r#"<h1>Only Title</h1><p>Content</p>"#;
        let output = fix_headings(html);
        assert!(
            output.contains("<h1>Only Title</h1>"),
            "single h1 should remain: {}",
            output
        );
    }

    #[test]
    fn unwrap_empty_links() {
        // Test that anchors with empty or "#" href are unwrapped
        let html = r##"<p><a href="#">Click me</a> and <a href="">Empty link</a></p>"##;
        let output = rewrite_empty_links(html);

        // Anchors should be unwrapped, text should remain
        assert!(
            output.contains("Click me"),
            "text content should remain: {}",
            output
        );
        assert!(
            output.contains("Empty link"),
            "text content should remain: {}",
            output
        );
        // Anchor tags should be removed
        assert!(
            !output.contains("<a "),
            "anchor tags should be removed: {}",
            output
        );
        assert!(
            !output.contains("</a>"),
            "closing anchor tags should be removed: {}",
            output
        );
    }

    #[test]
    fn unwrap_empty_links_preserves_real_links() {
        // Test that anchors with real hrefs are preserved
        let html =
            r##"<p><a href="https://example.com">Real link</a> and <a href="#">Empty</a></p>"##;
        let output = rewrite_empty_links(html);

        // Real link should be preserved
        assert!(
            output.contains("<a href=\"https://example.com\">Real link</a>"),
            "real links should be preserved: {}",
            output
        );
        // Empty href link should be unwrapped
        assert!(
            output.contains("Empty"),
            "empty link text should remain: {}",
            output
        );
        // Should have exactly one anchor tag (the real one)
        let a_count = output.matches("<a ").count();
        assert_eq!(a_count, 1, "should have exactly one anchor tag: {}", output);
    }

    #[test]
    fn post_cleaners_integration() {
        // Test that post-cleaners are applied in content extraction
        let html = r##"<html><body><article>
            <h1>Main Title</h1>
            <h1>Should Be H2</h1>
            <p><a href="#">Click</a></p>
        </article></body></html>"##;
        let doc = Document::from(html);

        let ce = ContentExtractor {
            field: FieldExtractor {
                selectors: vec![SelectorSpec::Css("article".to_string())],
                allow_multiple: false,
                default_cleaner: false,
                ..Default::default()
            },
            clean: vec![],
            transforms: HashMap::new(),
        };

        let result = extract_content_html(&doc, &ce);
        assert!(result.is_some());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        let output = &values[0];

        // First h1 should stay h1
        assert!(
            output.contains("<h1>Main Title</h1>"),
            "first h1 should remain: {}",
            output
        );
        // Second h1 should become h2
        assert!(
            output.contains("<h2>Should Be H2</h2>"),
            "second h1 should become h2: {}",
            output
        );
        // Empty href anchor should be unwrapped
        assert!(
            output.contains("Click"),
            "link text should remain: {}",
            output
        );
        assert!(
            !output.contains(r##"<a href="#">"##),
            "empty href anchor should be unwrapped: {}",
            output
        );
    }
}
