// ABOUTME: Go-compatible readability scoring for content extraction.
// ABOUTME: Ports ScoreContent, FindTopCandidate, MergeSiblings from Go hermes.

use dom_query::{Document, NodeId, NodeRef, Selection};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

// Scoring regex patterns matching Go constants.go
static PARAGRAPH_SCORE_TAGS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(p|li|span|pre)$").unwrap());
static CHILD_CONTENT_TAGS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(td|blockquote|ol|ul|dl)$").unwrap());
static BAD_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(address|form)$").unwrap());
pub static NON_TOP_CANDIDATE_TAGS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(br|b|i|label|hr|area|base|basefont|input|img|link|meta)$").unwrap()
});
static POSITIVE_SCORE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)article|articlecontent|instapaper_body|blog|body|content|entry-content-asset|entry|hentry|main|Normal|page|pagination|permalink|post|story|text|[-_]copy|\\Bcopy").unwrap()
});
static NEGATIVE_SCORE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)adbox|advert|author|bio|bookmark|bottom|byline|clear|com-|combx|comment|comment\\B|contact|copy|credit|crumb|date|deck|excerpt|featured|foot|footer|footnote|graf|head|info|infotext|instapaper_ignore|jump|linebreak|link|masthead|media|meta|modal|outbrain|promo|pr_|related|respond|roundcontent|scroll|secondary|share|shopping|shoutbox|side|sidebar|sponsor|stamp|sub|summary|tags|tools|widget").unwrap()
});
static PHOTO_HINTS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)figure|photo|image|caption").unwrap());
static READABILITY_ASSET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)entry-content-asset").unwrap());

// hNews content selectors for boosting known article patterns
const HNEWS_CONTENT_SELECTORS: &[(&str, &str)] = &[
    (".hentry", ".entry-content"),
    ("entry", ".entry-content"),
    (".entry", ".entry_content"),
    (".post", ".postbody"),
    (".post", ".post_body"),
    (".post", ".post-body"),
];

/// Score storage using NodeId as key
pub type NodeScores = HashMap<NodeId, i32>;

/// Pre-computed text metrics for O(1) link_density lookup
#[derive(Debug, Clone, Default)]
pub struct NodeTextMetrics {
    /// Total text length in this node's subtree (includes all descendants)
    pub total_text_len: usize,
    /// Text length inside <a> tags in this node's subtree
    pub link_text_len: usize,
}

impl NodeTextMetrics {
    /// Calculate link density from pre-computed metrics
    #[inline]
    pub fn link_density(&self) -> f64 {
        if self.total_text_len == 0 {
            0.0
        } else {
            self.link_text_len as f64 / self.total_text_len as f64
        }
    }
}

/// Storage for all node text metrics
pub type TextMetricsMap = HashMap<NodeId, NodeTextMetrics>;

/// Helper: get a score from the map for a node
fn get_score_for(node_id: NodeId, scores: &NodeScores) -> i32 {
    *scores.get(&node_id).unwrap_or(&0)
}

/// Helper: set a score in the map
fn set_score_for(node_id: NodeId, value: i32, scores: &mut NodeScores) {
    scores.insert(node_id, value);
}

/// Helper: get tag name from a Selection (assumes single node)
pub fn get_tag_name(selection: &Selection) -> String {
    selection
        .nodes()
        .first()
        .and_then(|node| node.node_name())
        .unwrap_or_default()
        .to_lowercase()
}

/// Helper: parse an attribute based score if present
fn score_from_attrs(selection: &Selection) -> Option<i32> {
    if let Some(val) = selection.attr("data-content-score") {
        if let Ok(score) = val.parse::<i32>() {
            return Some(score);
        }
    }
    if let Some(val) = selection.attr("score") {
        if let Ok(score) = val.parse::<i32>() {
            return Some(score);
        }
    }
    None
}

/// Count commas in text (more commas = better content quality)
fn score_commas(text: &str) -> i32 {
    text.matches(',').count() as i32
}

/// Bonus for text length in 50-character chunks
fn score_length(text: &str, length_bonus: i32) -> i32 {
    let bonus = if length_bonus == 0 { 1 } else { length_bonus };
    (text.len() / 50) as i32 * bonus
}

/// Multi-factor paragraph scoring
fn score_paragraph(text: &str) -> i32 {
    let text = text.trim();
    if text.is_empty() {
        return 0;
    }

    let mut score = 0i32;

    // Base score from commas
    score += score_commas(text);

    // Length bonus
    score += score_length(text, 1);

    // Penalty for short paragraphs
    if text.len() < 20 {
        score -= 10;
    }

    // Bonus for medium-length content
    if text.len() >= 50 && text.len() <= 200 {
        score += 5;
    }

    score
}

/// Score a node based on tag type
fn score_node(selection: &Selection) -> i32 {
    let tag_name = get_tag_name(selection);

    if PARAGRAPH_SCORE_TAGS.is_match(&tag_name) {
        let text = selection.text();
        return score_paragraph(&text);
    }

    if tag_name == "div" {
        return 5;
    }

    if CHILD_CONTENT_TAGS.is_match(&tag_name) {
        return 3;
    }

    if BAD_TAGS.is_match(&tag_name) {
        return -3;
    }

    if tag_name == "th" {
        return -5;
    }

    0
}

/// Get weight based on className and id patterns
pub fn get_weight(selection: &Selection) -> i32 {
    let class = selection.attr("class").unwrap_or_default();
    let id = selection.attr("id").unwrap_or_default();
    let mut score = 0i32;

    if !id.is_empty() {
        if POSITIVE_SCORE_RE.is_match(&id) {
            score += 25;
        }
        if NEGATIVE_SCORE_RE.is_match(&id) {
            score -= 25;
        }
    }

    if !class.is_empty() {
        if score == 0 {
            if POSITIVE_SCORE_RE.is_match(&class) {
                score += 25;
            }
            if NEGATIVE_SCORE_RE.is_match(&class) {
                score -= 25;
            }
        }

        if PHOTO_HINTS_RE.is_match(&class) {
            score += 10;
        }

        if READABILITY_ASSET.is_match(&class) {
            score += 25;
        }
    }

    score
}

/// Deprecated: Use get_weight directly
#[deprecated(note = "Use get_weight directly instead")]
pub fn get_weight_dq(sel: &Selection) -> i32 {
    get_weight(sel)
}

/// Calculate link density (ratio of link text to total text)
pub fn link_density(selection: &Selection) -> f64 {
    let total_text = selection.text();
    let total_len = total_text.len();

    if total_len == 0 {
        return 0.0;
    }

    let link_text_len: usize = selection
        .select("a")
        .iter()
        .map(|a| a.text().len())
        .sum();

    link_text_len as f64 / total_len as f64
}

/// Deprecated: Use link_density directly
#[deprecated(note = "Use link_density directly instead")]
pub fn link_density_dq(sel: &Selection) -> f64 {
    link_density(sel)
}

/// Compute text metrics for all nodes in a single O(N) post-order traversal.
/// Returns a map from NodeId to NodeTextMetrics for O(1) link_density lookups.
pub fn compute_text_metrics(doc: &Document) -> TextMetricsMap {
    let mut metrics = TextMetricsMap::new();

    // Start from html to cover the entire document tree, fall back to body
    let root = doc
        .select("html")
        .nodes()
        .first()
        .cloned()
        .or_else(|| doc.select("body").nodes().first().cloned());

    if let Some(root_node) = root {
        compute_metrics_recursive(&root_node, &mut metrics, false);
    }

    metrics
}

/// Recursive post-order traversal to compute text metrics.
/// The `in_ancestor_anchor` flag indicates if any ancestor is an <a> element.
/// This matches the semantics of the original `link_density` function which looks
/// for <a> elements among descendants, not the element itself.
fn compute_metrics_recursive(
    node: &NodeRef,
    metrics: &mut TextMetricsMap,
    in_ancestor_anchor: bool,
) -> NodeTextMetrics {
    let is_anchor = node
        .node_name()
        .map(|n| n.eq_ignore_ascii_case("a"))
        .unwrap_or(false);

    let mut this_metrics = NodeTextMetrics::default();

    // Process children - they inherit the anchor context
    // If THIS node is an anchor, children are "inside an ancestor anchor"
    let children_in_anchor = in_ancestor_anchor || is_anchor;

    for child in node.children_it(false) {
        if child.is_text() {
            // Text node: measure and attribute
            // Only mark as link text if inside an ANCESTOR anchor (not self)
            let text_len = child.text().len();
            this_metrics.total_text_len += text_len;
            if children_in_anchor {
                // This text is inside an anchor (either this node or an ancestor)
                // For the PARENT's metrics, we need to track link text
                // But for THIS node's metrics, only count if ancestor is anchor
                if in_ancestor_anchor {
                    this_metrics.link_text_len += text_len;
                }
            }
        } else if child.is_element() {
            // Skip script/style - they don't contain visible text
            let name = child.node_name().unwrap_or_default();
            if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style") {
                continue;
            }

            // Recurse with updated anchor context
            let child_metrics = compute_metrics_recursive(&child, metrics, children_in_anchor);
            this_metrics.total_text_len += child_metrics.total_text_len;

            // Child metrics already include link_text from ancestor anchors
            // Add link text based on whether the CHILD is inside an anchor
            if child
                .node_name()
                .map(|n| n.eq_ignore_ascii_case("a"))
                .unwrap_or(false)
            {
                // This child IS an anchor - its total_text is link text for US
                this_metrics.link_text_len += child_metrics.total_text_len;
            } else {
                // Child is not an anchor - use its computed link_text
                this_metrics.link_text_len += child_metrics.link_text_len;
            }
        }
        // Skip comments, doctypes, processing instructions, etc.
    }

    // Store metrics for this node
    metrics.insert(node.id, this_metrics.clone());

    this_metrics
}

/// O(1) link density lookup using pre-computed metrics.
/// Falls back to 0.0 if node not found in metrics map.
pub fn link_density_cached(selection: &Selection, metrics: &TextMetricsMap) -> f64 {
    get_node_id(selection)
        .and_then(|id| metrics.get(&id))
        .map(|m| m.link_density())
        .unwrap_or(0.0)
}

/// Check if text ends with sentence-ending punctuation
pub fn has_sentence_end(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    let last_char = text.chars().last().unwrap();
    matches!(last_char, '.' | '!' | '?' | ':' | ';')
}

/// Normalize whitespace in text
pub fn normalize_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Helper: get NodeId from a Selection
pub fn get_node_id(selection: &Selection) -> Option<NodeId> {
    selection.nodes().first().map(|node| node.id)
}

/// Helper: get parent selection
fn get_parent<'a>(selection: &Selection<'a>) -> Option<Selection<'a>> {
    let node = selection.nodes().first()?;
    let parent = node.parent()?;
    Some(Selection::from(parent))
}

/// Helper: check if element matches selector
fn matches_selector(selection: &Selection, selector: &str) -> bool {
    selection.is(selector)
}

/// Score content in a document using Go's algorithm
/// This applies hNews boosting and double-pass paragraph scoring
pub fn score_content(doc: &Document, weight_nodes: bool) -> NodeScores {
    fn add_to_parent(selection: &Selection, score: i32, scores: &mut NodeScores) {
        if let Some(parent) = get_parent(selection) {
            if let Some(parent_id) = get_node_id(&parent) {
                let parent_score = get_score_for(parent_id, scores);
                let addition = (score as f64 * 0.25) as i32;
                set_score_for(parent_id, parent_score + addition, scores);
            }
        }
    }

    fn get_or_init_score(selection: &Selection, scores: &mut NodeScores, weight_nodes: bool) -> i32 {
        if let Some(node_id) = get_node_id(selection) {
            let existing = get_score_for(node_id, scores);
            if existing != 0 {
                return existing;
            }

            let mut score = score_node(selection);
            if weight_nodes {
                score += get_weight(selection);
            }

            add_to_parent(selection, score, scores);
            score
        } else {
            0
        }
    }

    fn add_score_to(
        selection: &Selection,
        amount: i32,
        scores: &mut NodeScores,
        weight_nodes: bool,
    ) {
        if let Some(node_id) = get_node_id(selection) {
            let base = get_or_init_score(selection, scores, weight_nodes);
            set_score_for(node_id, base + amount, scores);
        }
    }

    let mut scores: NodeScores = HashMap::new();

    // First, boost hNews selectors
    for (parent_sel, child_sel) in HNEWS_CONTENT_SELECTORS {
        let combined = format!("{} {}", parent_sel, child_sel);

        for element in doc.select(&combined).iter() {
            // Walk ancestors until matching parent selector
            let mut current = element.clone();
            loop {
                let parent_opt = get_parent(&current);
                if parent_opt.is_none() {
                    break;
                }
                let parent = parent_opt.unwrap();

                if matches_selector(&parent, parent_sel) {
                    add_score_to(&parent, 80, &mut scores, weight_nodes);
                    break;
                }
                current = parent;
            }
        }
    }

    // Helper to check if element is inside <head>
    fn is_inside_head(selection: &Selection) -> bool {
        let mut current = selection.clone();
        loop {
            let parent_opt = get_parent(&current);
            if parent_opt.is_none() {
                break;
            }
            let parent = parent_opt.unwrap();

            let tag_name = get_tag_name(&parent);
            if tag_name == "head" {
                return true;
            }
            current = parent;
        }
        false
    }

    // Double-pass paragraph scoring
    for _ in 0..2 {
        for element in doc.select("p, pre").iter() {
            // Skip elements inside <head> - they shouldn't be scored
            if is_inside_head(&element) {
                continue;
            }

            if let Some(node_id) = get_node_id(&element) {
                if scores.contains_key(&node_id) {
                    continue;
                }

                let score = get_or_init_score(&element, &mut scores, weight_nodes);
                set_score_for(node_id, score, &mut scores);

                let raw_score = score_node(&element);

                if let Some(parent) = get_parent(&element) {
                    add_score_to(&parent, raw_score, &mut scores, weight_nodes);
                    if let Some(grandparent) = get_parent(&parent) {
                        add_score_to(&grandparent, raw_score / 2, &mut scores, weight_nodes);
                    }
                }
            }
        }
    }

    scores
}

/// Find the top scoring candidate element
pub fn find_top_candidate<'a>(
    doc: &'a Document,
    scores: &NodeScores,
    text_metrics: &TextMetricsMap,
) -> Option<Selection<'a>> {
    let mut best_candidate: Option<Selection<'a>> = None;
    let mut top_score = 0i32;

    // Find all elements with scores (from map or inline attrs)
    for element in doc.select("*").iter() {
        if let Some(node_id) = get_node_id(&element) {
            // Prefer explicit score map, but fall back to data-content-score/score attrs
            let raw_score = scores
                .get(&node_id)
                .copied()
                .or_else(|| score_from_attrs(&element));

            let score = match raw_score {
                Some(v) if v != 0 => v,
                _ => continue,
            };

            // Skip non-top-candidate tags
            let tag_name = get_tag_name(&element);
            if NON_TOP_CANDIDATE_TAGS_RE.is_match(&tag_name) {
                continue;
            }
            // Avoid selecting <body> as primary candidate when other options exist.
            // We'll fall back to body later if no suitable candidate remains.
            if tag_name == "body" {
                continue;
            }

            // Penalize very link-heavy candidates (aligns with digests-api heuristic)
            // Uses O(1) cached lookup instead of O(N) subtree traversal
            let density = link_density_cached(&element, text_metrics);
            let adjusted_score = if density > 0.5 {
                ((score as f64) * (1.0 - density)).round() as i32
            } else {
                score
            };

            if adjusted_score > top_score {
                top_score = adjusted_score;
                best_candidate = Some(element);
            }
        }
    }

    // Fall back to body if no candidate found
    if best_candidate.is_none() {
        let body = doc.select("body").first();
        if body.length() > 0 {
            return Some(body);
        }
    }

    best_candidate
}

/// Merge siblings that may be part of the main content
/// Returns the HTML of the merged content (wrapping div when siblings qualify)
pub fn merge_siblings(
    candidate: Selection,
    top_score: i32,
    scores: &NodeScores,
    text_metrics: &TextMetricsMap,
) -> String {
    // If no parent, return candidate's HTML
    let parent = match get_parent(&candidate) {
        Some(p) => p,
        None => return candidate.html().to_string(),
    };

    // Calculate sibling score threshold: max(10, topScore * 0.25)
    let sibling_threshold = 10i32.max((top_score as f64 * 0.25) as i32);

    let candidate_class = candidate.attr("class").unwrap_or_default();
    let candidate_id = get_node_id(&candidate);

    // Collect elements to include
    let mut included: Vec<Selection> = Vec::new();

    // Get children of parent
    for child in parent.children().iter() {
        let tag_name = get_tag_name(&child);

        // Skip non-top-candidate tags
        if NON_TOP_CANDIDATE_TAGS_RE.is_match(&tag_name) {
            continue;
        }

        let sibling_id = get_node_id(&child);
        let sibling_score = sibling_id
            .and_then(|id| scores.get(&id).copied())
            .or_else(|| score_from_attrs(&child))
            .unwrap_or(0);

        // Always include the candidate itself
        if sibling_id == candidate_id {
            included.push(child.clone());
            continue;
        }

        if sibling_score > 0 {
            // Calculate content bonus
            // Uses O(1) cached lookup instead of O(N) subtree traversal
            let mut content_bonus = 0i32;
            let density = link_density_cached(&child, text_metrics);

            if density < 0.05 {
                content_bonus += 20;
            }
            if density >= 0.5 {
                // If it's very link-heavy, skip adding as sibling
                continue;
            }

            // Class match bonus
            let sibling_class = child.attr("class").unwrap_or_default();
            if !sibling_class.is_empty() && sibling_class == candidate_class {
                content_bonus += (top_score as f64 * 0.2) as i32;
            }

            let new_score = sibling_score + content_bonus;

            if new_score >= sibling_threshold {
                included.push(child.clone());
                continue;
            }

            // Special handling for paragraphs
            if tag_name == "p" {
                let sibling_text = child.text();
                let text_len = normalize_spaces(&sibling_text).len();

                if text_len > 80 && density < 0.25 {
                    included.push(child.clone());
                    continue;
                }

                if text_len <= 80 && density == 0.0 && has_sentence_end(&sibling_text) {
                    included.push(child.clone());
                    continue;
                }
            }
        }
    }

    // If only candidate was included, return its outer HTML with score
    if included.len() <= 1 {
        return candidate.html().to_string();
    }

    // Wrap multiple merged elements in a div preserving order
    let mut output = String::new();
    output.push_str("<div>");
    for node in included {
        output.push_str(&node.html().to_string());
    }
    output.push_str("</div>");
    output
}

#[allow(dead_code)]
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[allow(dead_code)]
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

/// Full content extraction pipeline using Go's scoring algorithm
pub fn extract_best_content(doc: &Document) -> Option<String> {
    // Score all content
    let scores = score_content(doc, true);

    // Pre-compute text metrics for O(1) link density lookups
    let text_metrics = compute_text_metrics(doc);

    // Find top candidate
    let candidate = find_top_candidate(doc, &scores, &text_metrics)?;

    // Get top score for merge threshold calculation
    let top_score = get_node_id(&candidate)
        .and_then(|id| scores.get(&id).copied())
        .unwrap_or(0);

    // Merge siblings and return content
    Some(merge_siblings(candidate, top_score, &scores, &text_metrics))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_commas() {
        assert_eq!(score_commas("hello, world, test"), 2);
        assert_eq!(score_commas("no commas here"), 0);
    }

    #[test]
    fn test_score_paragraph() {
        // Short paragraph gets penalty
        assert!(score_paragraph("Hi") < 0);

        // Medium paragraph gets bonus
        let medium = "This is a medium length paragraph with some commas, and more text.";
        assert!(score_paragraph(medium) > 0);
    }

    #[test]
    fn test_get_weight() {
        let html = r#"<div class="article-content" id="main">test</div>"#;
        let doc = Document::from(html);
        let el = doc.select("div").first();

        let weight = get_weight(&el);
        // Should be positive due to "article" and "content" in class, "main" in id
        assert!(weight > 0);
    }

    #[test]
    fn test_link_density() {
        let html = r##"<div>Some text <a href="#">link</a> more text</div>"##;
        let doc = Document::from(html);
        let el = doc.select("div").first();

        let density = link_density(&el);
        assert!(density > 0.0 && density < 1.0);
    }

    #[test]
    fn test_has_sentence_end() {
        assert!(has_sentence_end("This is a sentence."));
        assert!(has_sentence_end("Is this a question?"));
        assert!(has_sentence_end("Important!"));
        assert!(!has_sentence_end("No ending here"));
    }

    #[test]
    fn test_score_content() {
        let html = r#"
            <html><body>
                <article class="hentry">
                    <div class="entry-content">
                        <p>This is a paragraph with some content, and commas, to score well.</p>
                        <p>Another paragraph with more text and details about the article.</p>
                    </div>
                </article>
            </body></html>
        "#;
        let doc = Document::from(html);
        let scores = score_content(&doc, true);

        // Should have some scored elements
        assert!(!scores.is_empty());
    }

    #[test]
    fn test_extract_best_content() {
        let html = r#"
            <html><body>
                <nav>Navigation links</nav>
                <article>
                    <p>This is the main article content with multiple paragraphs.</p>
                    <p>The second paragraph has more information, details, and context.</p>
                    <p>A third paragraph rounds out the article nicely.</p>
                </article>
                <aside>Sidebar content</aside>
            </body></html>
        "#;
        let doc = Document::from(html);
        let content = extract_best_content(&doc);

        assert!(content.is_some());
        let content = content.unwrap();
        assert!(content.contains("main article content"));
    }

    #[test]
    fn test_find_top_candidate_respects_score_attrs() {
        let html = r#"
            <html><body>
                <div class="content" score="50">High score pick</div>
                <div score="10">Low score</div>
            </body></html>
        "#;
        let doc = Document::from(html);
        let scores = NodeScores::new(); // force attribute-based scoring
        let text_metrics = compute_text_metrics(&doc);

        let candidate = find_top_candidate(&doc, &scores, &text_metrics).expect("candidate");
        let tag = get_tag_name(&candidate);
        assert_eq!(tag, "div");
        let text = normalize_spaces(&candidate.text());
        assert_eq!(text, "High score pick");
    }

    #[test]
    fn test_find_top_candidate_skips_non_candidate_tags() {
        let html = r#"
            <html><body>
                <br score="100">
                <div score="30">Valid content</div>
            </body></html>
        "#;
        let doc = Document::from(html);
        let scores = NodeScores::new();
        let text_metrics = compute_text_metrics(&doc);

        let candidate = find_top_candidate(&doc, &scores, &text_metrics).expect("candidate");
        assert_eq!(get_tag_name(&candidate), "div");
        assert_eq!(
            normalize_spaces(&candidate.text()),
            "Valid content"
        );
    }

    #[test]
    fn test_find_top_candidate_fallbacks_to_body() {
        let html = "<html><body><div>No scores</div></body></html>";
        let doc = Document::from(html);
        let scores = NodeScores::new();
        let text_metrics = compute_text_metrics(&doc);

        let candidate = find_top_candidate(&doc, &scores, &text_metrics).expect("fallback body");
        assert_eq!(get_tag_name(&candidate), "body");
    }

    #[test]
    fn test_merge_siblings_wraps_and_includes() {
        let html = r#"
            <div class="parent">
                <div class="candidate" data-content-score="50">Main content with text.</div>
                <div class="sibling" data-content-score="20">Sibling paragraph with enough length to be included because it has more than eighty characters and low link density.</div>
            </div>
        "#;
        let doc = Document::from(html);
        let candidate = doc.select(".candidate").first();
        let sibling = doc.select(".sibling").first();
        let text_metrics = compute_text_metrics(&doc);

        let mut scores = NodeScores::new();
        if let Some(cand_id) = get_node_id(&candidate) {
            scores.insert(cand_id, 50);
        }
        if let Some(sib_id) = get_node_id(&sibling) {
            scores.insert(sib_id, 20);
        }

        let merged = merge_siblings(candidate, 50, &scores, &text_metrics);
        assert!(merged.starts_with("<div>"));
        assert!(merged.contains("Main content with text"));
        assert!(merged.contains("Sibling paragraph"));
    }

    #[test]
    fn test_merge_siblings_filters_non_top_candidate_tags() {
        let html = r#"
            <div class="parent">
                <div class="candidate" data-content-score="50">Main content</div>
                <br data-content-score="10">
                <b data-content-score="10">Bold text</b>
                <div class="valid" data-content-score="20">Valid sibling</div>
            </div>
        "#;
        let doc = Document::from(html);
        let cand = doc.select(".candidate").first();
        let valid = doc.select(".valid").first();
        let text_metrics = compute_text_metrics(&doc);

        let mut scores = NodeScores::new();
        if let Some(cand_id) = get_node_id(&cand) {
            scores.insert(cand_id, 50);
        }
        if let Some(valid_id) = get_node_id(&valid) {
            scores.insert(valid_id, 20);
        }

        let merged = merge_siblings(cand, 50, &scores, &text_metrics);
        assert!(merged.contains("Main content"));
        assert!(merged.contains("Valid sibling"));
        assert!(!merged.contains("<br"));
        assert!(!merged.contains("<b"));
    }

    #[test]
    fn test_merge_siblings_paragraph_rules() {
        let html = r##"
            <div class="parent">
                <div class="candidate" data-content-score="50">Main content</div>
                <p class="long" data-content-score="5">This is a long paragraph with more than eighty characters of text content to test the length threshold logic with low link density for inclusion.</p>
                <p class="short-end" data-content-score="5">Short sentence.</p>
                <p class="short-no-end" data-content-score="5">Short no end</p>
                <p class="short-link" data-content-score="5">Short <a href="#">link</a>.</p>
            </div>
        "##;

        let doc = Document::from(html);
        let cand = doc.select(".candidate").first();
        let text_metrics = compute_text_metrics(&doc);
        let mut scores = NodeScores::new();
        if let Some(cand_id) = get_node_id(&cand) {
            scores.insert(cand_id, 50);
        }

        let merged = merge_siblings(cand, 50, &scores, &text_metrics);

        assert!(merged.contains("long paragraph"));
        assert!(merged.contains("Short sentence."));
        assert!(!merged.contains("short-link"));
    }

    #[test]
    fn test_compute_text_metrics_basic() {
        let html = r##"<html><body><div>Hello <a href="#">World</a>!</div></body></html>"##;
        let doc = Document::from(html);
        let metrics = compute_text_metrics(&doc);

        let div = doc.select("div").first();
        let div_id = get_node_id(&div).expect("div should have id");
        let m = metrics.get(&div_id).expect("div should have metrics");

        // "Hello World!" = 12 chars total, "World" = 5 chars link
        assert_eq!(m.total_text_len, 12);
        assert_eq!(m.link_text_len, 5);
        assert!((m.link_density() - 5.0 / 12.0).abs() < 0.001);
    }

    #[test]
    fn test_link_density_cached_matches_link_density() {
        let html = r##"
            <html><body>
                <div class="content">
                    <p>Regular paragraph text without any links.</p>
                    <p>Paragraph with <a href="#">one link</a> inside.</p>
                    <div><a href="#">All links</a></div>
                </div>
            </body></html>
        "##;
        let doc = Document::from(html);
        let metrics = compute_text_metrics(&doc);

        // Compare cached vs direct link_density for all elements
        for element in doc.select("*").iter() {
            let direct = link_density(&element);
            let cached = link_density_cached(&element, &metrics);
            assert!(
                (direct - cached).abs() < 0.001,
                "Mismatch for {:?}: direct={}, cached={}",
                get_tag_name(&element),
                direct,
                cached
            );
        }
    }
}
