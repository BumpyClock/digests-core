// ABOUTME: Go-compatible readability scoring for content extraction.
// ABOUTME: Ports ScoreContent, FindTopCandidate, MergeSiblings from Go hermes.

use ego_tree::NodeId;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
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

/// Helper: get a score from the map for a node
fn get_score_for(node_id: NodeId, scores: &NodeScores) -> i32 {
    *scores.get(&node_id).unwrap_or(&0)
}

/// Helper: set a score in the map
fn set_score_for(node_id: NodeId, value: i32, scores: &mut NodeScores) {
    scores.insert(node_id, value);
}

/// Helper: parse an attribute based score if present
fn score_from_attrs(element: &ElementRef) -> Option<i32> {
    if let Some(val) = element.value().attr("data-content-score") {
        if let Ok(score) = val.parse::<i32>() {
            return Some(score);
        }
    }
    if let Some(val) = element.value().attr("score") {
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
fn score_node(element: &ElementRef) -> i32 {
    let tag_name = element.value().name().to_lowercase();

    if PARAGRAPH_SCORE_TAGS.is_match(&tag_name) {
        let text = element.text().collect::<String>();
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
pub fn get_weight(element: &ElementRef) -> i32 {
    let class = element.value().attr("class").unwrap_or("");
    let id = element.value().attr("id").unwrap_or("");
    let mut score = 0i32;

    if !id.is_empty() {
        if POSITIVE_SCORE_RE.is_match(id) {
            score += 25;
        }
        if NEGATIVE_SCORE_RE.is_match(id) {
            score -= 25;
        }
    }

    if !class.is_empty() {
        if score == 0 {
            if POSITIVE_SCORE_RE.is_match(class) {
                score += 25;
            }
            if NEGATIVE_SCORE_RE.is_match(class) {
                score -= 25;
            }
        }

        if PHOTO_HINTS_RE.is_match(class) {
            score += 10;
        }

        if READABILITY_ASSET.is_match(class) {
            score += 25;
        }
    }

    score
}

/// Calculate link density (ratio of link text to total text)
pub fn link_density(element: &ElementRef) -> f64 {
    let total_text = element.text().collect::<String>();
    let total_len = total_text.len();

    if total_len == 0 {
        return 0.0;
    }

    let a_selector = Selector::parse("a").unwrap();
    let link_text_len: usize = element
        .select(&a_selector)
        .map(|a| a.text().collect::<String>().len())
        .sum();

    link_text_len as f64 / total_len as f64
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

/// Score content in a document using Go's algorithm
/// This applies hNews boosting and double-pass paragraph scoring
pub fn score_content(doc: &Html, weight_nodes: bool) -> NodeScores {
    fn add_to_parent(element: &ElementRef, score: i32, scores: &mut NodeScores) {
        if let Some(parent) = element.parent().and_then(ElementRef::wrap) {
            let parent_id = parent.id();
            let parent_score = get_score_for(parent_id, scores);
            let addition = (score as f64 * 0.25) as i32;
            set_score_for(parent_id, parent_score + addition, scores);
        }
    }

    fn get_or_init_score(element: &ElementRef, scores: &mut NodeScores, weight_nodes: bool) -> i32 {
        let node_id = element.id();
        let existing = get_score_for(node_id, scores);
        if existing != 0 {
            return existing;
        }

        let mut score = score_node(element);
        if weight_nodes {
            score += get_weight(element);
        }

        add_to_parent(element, score, scores);
        score
    }

    fn add_score_to(
        element: &ElementRef,
        amount: i32,
        scores: &mut NodeScores,
        weight_nodes: bool,
    ) {
        let node_id = element.id();
        let base = get_or_init_score(element, scores, weight_nodes);
        set_score_for(node_id, base + amount, scores);
    }

    let mut scores: NodeScores = HashMap::new();

    // First, boost hNews selectors
    for (parent_sel, child_sel) in HNEWS_CONTENT_SELECTORS {
        let combined = format!("{} {}", parent_sel, child_sel);
        let selector = match Selector::parse(&combined) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for element in doc.select(&selector) {
            if let Ok(parent_selector) = Selector::parse(parent_sel) {
                // Walk ancestors until matching parent selector
                let mut current = element.parent();
                while let Some(parent_node) = current {
                    if let Some(parent_el) = ElementRef::wrap(parent_node) {
                        if parent_selector.matches(&parent_el) {
                            add_score_to(&parent_el, 80, &mut scores, weight_nodes);
                            break;
                        }
                    }
                    current = parent_node.parent();
                }
            }
        }
    }

    // Double-pass paragraph scoring
    for _ in 0..2 {
        let p_pre_selector = Selector::parse("p, pre").unwrap();
        for element in doc.select(&p_pre_selector) {
            let node_id = element.id();
            if scores.contains_key(&node_id) {
                continue;
            }

            let score = get_or_init_score(&element, &mut scores, weight_nodes);
            set_score_for(node_id, score, &mut scores);

            let raw_score = score_node(&element);

            if let Some(parent) = element.parent().and_then(ElementRef::wrap) {
                add_score_to(&parent, raw_score, &mut scores, weight_nodes);
                if let Some(grandparent) = parent.parent().and_then(ElementRef::wrap) {
                    add_score_to(&grandparent, raw_score / 2, &mut scores, weight_nodes);
                }
            }
        }
    }

    scores
}

/// Find the top scoring candidate element
pub fn find_top_candidate<'a>(doc: &'a Html, scores: &NodeScores) -> Option<ElementRef<'a>> {
    let mut best_candidate: Option<ElementRef<'a>> = None;
    let mut top_score = 0i32;

    // Find all elements with scores
    let all_selector = Selector::parse("*").unwrap();
    for element in doc.select(&all_selector) {
        let node_id = element.id();

        if let Some(&score) = scores.get(&node_id) {
            // Skip non-top-candidate tags
            let tag_name = element.value().name().to_lowercase();
            if NON_TOP_CANDIDATE_TAGS_RE.is_match(&tag_name) {
                continue;
            }
            // Avoid selecting <body> as primary candidate when other options exist.
            // We'll fall back to body later if no suitable candidate remains.
            if tag_name == "body" {
                continue;
            }

            // Penalize very link-heavy candidates (aligns with digests-api heuristic)
            let density = link_density(&element);
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
        if let Ok(body_sel) = Selector::parse("body") {
            if let Some(body) = doc.select(&body_sel).next() {
                return Some(body);
            }
        }
    }

    best_candidate
}

/// Merge siblings that may be part of the main content
/// Returns the HTML of the merged content (wrapping div when siblings qualify)
pub fn merge_siblings(candidate: ElementRef, top_score: i32, scores: &NodeScores) -> String {
    // If no parent, return candidate's HTML
    let parent_node = match candidate.parent() {
        Some(p) => p,
        None => return candidate.html(),
    };

    // Calculate sibling score threshold: max(10, topScore * 0.25)
    let sibling_threshold = 10i32.max((top_score as f64 * 0.25) as i32);

    let candidate_class = candidate.value().attr("class").unwrap_or("");

    // Collect elements to include
    let mut included: Vec<ElementRef> = Vec::new();

    for child in parent_node.children() {
        if let Some(sibling) = ElementRef::wrap(child) {
            let tag_name = sibling.value().name().to_lowercase();

            // Skip non-top-candidate tags
            if NON_TOP_CANDIDATE_TAGS_RE.is_match(&tag_name) {
                continue;
            }

            let sibling_id = sibling.id();
            let sibling_score = scores.get(&sibling_id).copied().unwrap_or(0);

            // Always include the candidate itself
            if sibling.id() == candidate.id() {
                included.push(sibling);
                continue;
            }

            if sibling_score > 0 {
                // Calculate content bonus
                let mut content_bonus = 0i32;
                let density = link_density(&sibling);

                if density < 0.05 {
                    content_bonus += 20;
                }
                if density >= 0.5 {
                    content_bonus -= 20;
                    // If it's very link-heavy, skip adding as sibling
                    continue;
                }

                // Class match bonus
                let sibling_class = sibling.value().attr("class").unwrap_or("");
                if !sibling_class.is_empty() && sibling_class == candidate_class {
                    content_bonus += (top_score as f64 * 0.2) as i32;
                }

                let new_score = sibling_score + content_bonus;

                if new_score >= sibling_threshold {
                    included.push(sibling);
                    continue;
                }

                // Special handling for paragraphs
                if tag_name == "p" {
                    let sibling_text = sibling.text().collect::<String>();
                    let text_len = normalize_spaces(&sibling_text).len();

                    if text_len > 80 && density < 0.25 {
                        included.push(sibling);
                        continue;
                    }

                    if text_len <= 80 && density == 0.0 && has_sentence_end(&sibling_text) {
                        included.push(sibling);
                        continue;
                    }
                }
            }
        }
    }

    // If only candidate was included, return its outer HTML with score
    if included.len() <= 1 {
        return candidate.html();
    }

    // Wrap multiple merged elements in a div preserving order
    let mut output = String::new();
    output.push_str("<div>");
    for node in included {
        output.push_str(&node.html());
    }
    output.push_str("</div>");
    output
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

fn serialize_element(element: &ElementRef, scores: &NodeScores) -> String {
    let mut out = String::new();
    serialize_node_with_scores(element.id(), *element, scores, &mut out);
    out
}

fn serialize_node_with_scores(
    node_id: NodeId,
    node_ref: ego_tree::NodeRef<scraper::Node>,
    scores: &NodeScores,
    output: &mut String,
) {
    match node_ref.value() {
        scraper::Node::Text(text) => output.push_str(&**text),
        scraper::Node::Comment(comment) => {
            output.push_str("<!--");
            output.push_str(&**comment);
            output.push_str("-->");
        }
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

            if let Some(score) = scores.get(&node_id) {
                output.push_str(" data-content-score=\"");
                output.push_str(&score.to_string());
                output.push('"');
            }

            if is_void_element(tag_name) {
                output.push_str(" />");
                return;
            }

            output.push('>');
            for child in node_ref.children() {
                serialize_node_with_scores(child.id(), child, scores, output);
            }
            output.push_str("</");
            output.push_str(tag_name);
            output.push('>');
        }
        _ => {}
    }
}

/// Full content extraction pipeline using Go's scoring algorithm
pub fn extract_best_content(doc: &Html) -> Option<String> {
    // Score all content
    let scores = score_content(doc, true);

    // Find top candidate
    let candidate = find_top_candidate(doc, &scores)?;

    // Get top score for merge threshold calculation
    let candidate_id = candidate.id();
    let top_score = scores.get(&candidate_id).copied().unwrap_or(0);

    // Merge siblings and return content
    Some(merge_siblings(candidate, top_score, &scores))
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
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("div").unwrap();
        let el = doc.select(&sel).next().unwrap();

        let weight = get_weight(&el);
        // Should be positive due to "article" and "content" in class, "main" in id
        assert!(weight > 0);
    }

    #[test]
    fn test_link_density() {
        let html = r##"<div>Some text <a href="#">link</a> more text</div>"##;
        let doc = Html::parse_fragment(html);
        let sel = Selector::parse("div").unwrap();
        let el = doc.select(&sel).next().unwrap();

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
        let doc = Html::parse_document(html);
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
        let doc = Html::parse_document(html);
        let content = extract_best_content(&doc);

        assert!(content.is_some());
        let content = content.unwrap();
        assert!(content.contains("main article content"));
    }
}
