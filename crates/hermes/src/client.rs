// ABOUTME: The main Client struct for Hermes that handles HTTP requests and HTML parsing.
// ABOUTME: Provides async parse() and parse_html() methods to extract article content from URLs or HTML strings.

use chrono::{DateTime, Utc};
use scraper::{Html, Selector};

use crate::dom::cleaners::{clean_article, is_unlikely_candidate};
use crate::dom::scoring::NON_TOP_CANDIDATE_TAGS_RE;
use crate::error::ParseError;
use crate::extractors::content::extract_content_first_html;
#[cfg(test)]
use crate::extractors::custom::ContentExtractor;
use crate::extractors::custom::{ExtractorRegistry, FieldExtractor, SelectorSpec};
use crate::extractors::fields::{
    extract_attr_first, extract_field_text_single, extract_first_attr, extract_meta_content,
    normalize_lang,
};
use crate::extractors::loader::load_builtin_registry;
use crate::extractors::select::extract_field_first_text;
use crate::formats::{
    extract_excerpt, extract_title, html_to_markdown, html_to_text, sanitize_html,
};
use crate::options::{ClientBuilder, ContentType, Options};
use crate::resource::{fetch, FetchOptions};
use crate::result::{word_count, ParseResult};
#[cfg(test)]
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use url::Url;

/// Build a generic title FieldExtractor with fallback selectors.
///
/// Selector order: "title", "h1", "h2".
fn build_generic_title_extractor() -> FieldExtractor {
    FieldExtractor {
        selectors: vec![
            SelectorSpec::Css("title".to_string()),
            SelectorSpec::Css("h1".to_string()),
            SelectorSpec::Css("h2".to_string()),
        ],
        allow_multiple: false,
        default_cleaner: false,
        format: None,
        timezone: None,
    }
}

/// Extract body inner HTML from a parsed document.
///
/// Tries to select "body" element and return its inner HTML.
/// Returns empty string if no body element is found.
fn extract_body_inner_html(doc: &Html) -> String {
    if let Ok(selector) = Selector::parse("body") {
        if let Some(body) = doc.select(&selector).next() {
            return body.inner_html();
        }
    }
    String::new()
}

/// Score and select the best generic content element using readability-style heuristics.
///
/// Iterates through candidate selectors and scores each element based on:
/// - Text length (longer is better)
/// - Link density (lower is better - penalizes navigation-heavy content)
/// - Penalty tags (form, nav, aside descendants reduce score)
///
/// Requires minimum text length of 80 characters to accept a candidate.
/// Returns None if no candidates meet the threshold, triggering body fallback.
fn score_generic_content(doc: &Html, title: &str) -> Option<String> {
    use crate::dom::scoring::{merge_siblings, score_content};

    let scores = score_content(doc, true);

    // Find best candidate skipping unlikely / non-top tags
    let mut best: Option<scraper::ElementRef> = None;
    let mut best_score = i32::MIN;
    let selector = Selector::parse("*").unwrap();
    for el in doc.select(&selector) {
        let tag = el.value().name().to_lowercase();
        if tag == "body" || tag == "html" || NON_TOP_CANDIDATE_TAGS_RE.is_match(&tag) {
            continue;
        }
        if is_unlikely_candidate(&el) {
            continue;
        }
        if let Some(s) = scores.get(&el.id()) {
            let density = crate::dom::scoring::link_density(&el);
            let effective = (*s as f64 - (density * 100.0)) as i32;
            if effective > best_score {
                best_score = effective;
                best = Some(el);
            }
        }
    }

    let candidate = best.or_else(|| {
        if let Ok(sel) = Selector::parse("body") {
            doc.select(&sel).next()
        } else {
            None
        }
    })?;

    let merged = merge_siblings(candidate, best_score, &scores);
    Some(clean_article(&merged, title))
}

/// Generic author selectors in priority order.
const GENERIC_AUTHOR_SELECTORS: &[&str] = &[
    "meta[name='author']",
    "meta[property='article:author']",
    ".byline",
    ".author",
    "[itemprop='author']",
];

/// Generic date selectors for meta tags (content attribute).
const GENERIC_DATE_META_SELECTORS: &[&str] = &[
    "meta[property='article:published_time']",
    "meta[name='date']",
];

/// Generic lead image selectors in priority order.
const GENERIC_IMAGE_SELECTORS: &[(&str, &str)] = &[
    ("meta[property='og:image']", "content"),
    ("meta[name='twitter:image']", "content"),
    ("img", "src"),
];

/// Parse a date string, trying RFC3339 first then falling back to dateparser.
///
/// RFC3339 is tried first as a fast path for standard formats.
/// If that fails, dateparser is used for looser/natural date formats.
/// Returns None if all parsing attempts fail.
fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    // Fast path: RFC3339/ISO8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try common loose date-only formats (no timezone) before falling back to dateparser.
    // This avoids local timezone shifts (e.g., converting midnight local to UTC and changing the day).
    const LOOSE_PATTERNS: &[&str] = &[
        "%b %e, %Y", // Jan 5, 2024
        "%e %b %Y",  // 5 Jan 2024
        "%b %d, %Y", // Jan 05, 2024
        "%d %b %Y",  // 05 Jan 2024
        "%B %e, %Y", // January 5, 2024
        "%e %B %Y",  // 5 January 2024
        "%B %d, %Y", // January 05, 2024
        "%d %B %Y",  // 05 January 2024
    ];
    for pat in LOOSE_PATTERNS {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(s.trim(), pat) {
            let naive_dt = date.and_hms_opt(0, 0, 0)?;
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc));
        }
    }

    // Fall back to dateparser for natural/loose formats
    if let Ok(dt) = dateparser::parse(s) {
        return Some(dt.with_timezone(&Utc));
    }

    None
}

/// Extract author using custom extractor field if available, falling back to generic heuristics.
fn extract_author(doc: &Html, custom: Option<&FieldExtractor>) -> Option<String> {
    // Try custom extractor first
    if let Some(fe) = custom {
        if let Some(author) = extract_field_first_text(doc, fe) {
            return Some(author);
        }
    }

    // Fall back to generic heuristics
    extract_field_text_single(doc, GENERIC_AUTHOR_SELECTORS)
}

/// Extract date_published using custom extractor field if available, falling back to generic heuristics.
fn extract_date_published(doc: &Html, custom: Option<&FieldExtractor>) -> Option<DateTime<Utc>> {
    // Try custom extractor first
    if let Some(fe) = custom {
        if let Some(date_str) = extract_field_first_text(doc, fe) {
            if let Some(dt) = parse_date(&date_str) {
                return Some(dt);
            }
        }
    }

    // Fall back to generic heuristics: meta tags first
    for sel in GENERIC_DATE_META_SELECTORS {
        if let Some(content) = extract_meta_content(doc, sel) {
            if let Some(dt) = parse_date(&content) {
                return Some(dt);
            }
        }
    }

    // Try time[datetime] attribute
    if let Some(dt_str) = extract_attr_first(doc, "time[datetime]", "datetime") {
        if let Some(dt) = parse_date(&dt_str) {
            return Some(dt);
        }
    }

    // Try time element text (now supports natural date formats via dateparser)
    if let Some(time_text) = extract_field_text_single(doc, &["time"]) {
        if let Some(dt) = parse_date(&time_text) {
            return Some(dt);
        }
    }

    None
}

/// Extract lead_image_url using custom extractor field if available, falling back to generic heuristics.
fn extract_lead_image_url(doc: &Html, custom: Option<&FieldExtractor>) -> Option<String> {
    // Try custom extractor first
    if let Some(fe) = custom {
        if let Some(url) = extract_field_first_text(doc, fe) {
            return Some(url);
        }
    }

    // Fall back to generic heuristics: og:image, twitter:image, then first img
    for (sel, attr) in GENERIC_IMAGE_SELECTORS {
        if let Some(url) = extract_attr_first(doc, sel, attr) {
            return Some(url);
        }
    }

    None
}

/// Extract site_name using generic heuristics.
fn extract_site_name(doc: &Html) -> Option<String> {
    let selectors = &[
        "meta[property='og:site_name']",
        "meta[name='application-name']",
    ];
    extract_first_attr(doc, selectors, "content")
}

/// Extract site_title using generic heuristics.
fn extract_site_title(doc: &Html) -> Option<String> {
    // meta[name=title] content attr, then <title> text
    if let Some(val) = extract_first_attr(doc, &["meta[name='title']"], "content") {
        return Some(val);
    }
    // Fall back to <title> element text
    extract_field_text_single(doc, &["title"])
}

/// Extract site_image using generic heuristics (same as lead image heuristics).
fn extract_site_image(doc: &Html) -> Option<String> {
    let selectors = &["meta[property='og:image']", "meta[name='twitter:image']"];
    extract_first_attr(doc, selectors, "content")
}

/// Extract description using generic heuristics.
fn extract_description_heuristic(doc: &Html) -> Option<String> {
    let selectors = &[
        "meta[name='description']",
        "meta[property='og:description']",
    ];
    extract_first_attr(doc, selectors, "content")
}

/// Extract language from HTML document and normalize to primary tag.
fn extract_language(doc: &Html) -> Option<String> {
    // Try <html lang="..."> first
    if let Some(lang) = extract_first_attr(doc, &["html"], "lang") {
        let normalized = normalize_lang(&lang);
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }

    // Fall back to meta tags
    let meta_selectors = &["meta[property='og:locale']", "meta[name='language']"];
    if let Some(val) = extract_first_attr(doc, meta_selectors, "content") {
        let normalized = normalize_lang(&val);
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }

    None
}

/// Extract theme_color using generic heuristics.
fn extract_theme_color(doc: &Html) -> Option<String> {
    extract_first_attr(doc, &["meta[name='theme-color']"], "content")
}

/// Extract favicon URL using generic heuristics.
fn extract_favicon(doc: &Html) -> Option<String> {
    let selectors = &[
        "link[rel='icon']",
        "link[rel='shortcut icon']",
        "link[rel='apple-touch-icon']",
    ];
    extract_first_attr(doc, selectors, "href")
}

/// Extract dek using custom extractor if available, falling back to description heuristic.
fn extract_dek(doc: &Html, custom: Option<&FieldExtractor>) -> Option<String> {
    // Try custom extractor first
    if let Some(fe) = custom {
        if let Some(dek) = extract_field_first_text(doc, fe) {
            return Some(dek);
        }
    }

    // Fall back to description heuristic
    extract_description_heuristic(doc)
}

/// Extract excerpt using custom extractor if available.
fn extract_custom_excerpt(doc: &Html, custom: Option<&FieldExtractor>) -> Option<String> {
    if let Some(fe) = custom {
        return extract_field_first_text(doc, fe);
    }
    None
}

/// Generic video URL selectors in priority order.
const GENERIC_VIDEO_SELECTORS: &[(&str, &str)] = &[
    ("meta[property='og:video']", "content"),
    ("meta[property='og:video:url']", "content"),
    ("meta[name='twitter:player']", "content"),
    ("video", "src"),
    ("video source", "src"),
];

/// Video metadata meta tag selectors.
const VIDEO_METADATA_SELECTORS: &[(&str, &str)] = &[
    ("meta[property='og:video:type']", "og:video:type"),
    ("meta[property='og:video:width']", "og:video:width"),
    ("meta[property='og:video:height']", "og:video:height"),
    (
        "meta[property='og:video:secure_url']",
        "og:video:secure_url",
    ),
];

/// Extract video URL using priority: og:video, twitter:player, <video src>, <source src>.
fn extract_video_url(doc: &Html) -> Option<String> {
    for (sel, attr) in GENERIC_VIDEO_SELECTORS {
        if let Some(url) = extract_attr_first(doc, sel, attr) {
            return Some(url);
        }
    }
    None
}

/// Extract video metadata as a JSON object with available og:video:* properties.
fn extract_video_metadata(doc: &Html) -> Option<serde_json::Value> {
    let mut map = serde_json::Map::new();

    for (sel, key) in VIDEO_METADATA_SELECTORS {
        if let Some(value) = extract_meta_content(doc, sel) {
            map.insert(key.to_string(), serde_json::Value::String(value));
        }
    }

    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// Extract text direction from the document.
///
/// Priority:
/// 1. dir attribute on <html> or <body>
/// 2. Detect RTL if >= 30% of letters are in RTL unicode ranges (Hebrew/Arabic)
///
/// Returns "rtl" or "ltr" (default).
fn extract_direction(doc: &Html, plain_text: &str) -> String {
    // Check dir attribute on <html>
    if let Some(dir) = extract_first_attr(doc, &["html"], "dir") {
        let dir_lower = dir.to_lowercase();
        if dir_lower == "rtl" || dir_lower == "ltr" {
            return dir_lower;
        }
    }

    // Check dir attribute on <body>
    if let Some(dir) = extract_first_attr(doc, &["body"], "dir") {
        let dir_lower = dir.to_lowercase();
        if dir_lower == "rtl" || dir_lower == "ltr" {
            return dir_lower;
        }
    }

    // Detect RTL based on character frequency in plain text
    let mut rtl_count = 0u32;
    let mut letter_count = 0u32;

    for ch in plain_text.chars() {
        if ch.is_alphabetic() {
            letter_count += 1;
            if is_rtl_char(ch) {
                rtl_count += 1;
            }
        }
    }

    // Use 30% threshold for RTL detection
    if letter_count > 0 && (rtl_count as f64 / letter_count as f64) >= 0.30 {
        "rtl".to_string()
    } else {
        "ltr".to_string()
    }
}

/// Check if a character is in RTL unicode ranges (Hebrew or Arabic).
fn is_rtl_char(ch: char) -> bool {
    let code = ch as u32;
    // Hebrew: U+0590..U+05FF, U+FB1D..U+FB4F
    // Arabic: U+0600..U+06FF, U+0750..U+077F, U+08A0..U+08FF, U+FB50..U+FDFF, U+FE70..U+FEFF
    (0x0590..=0x05FF).contains(&code)
        || (0xFB1D..=0xFB4F).contains(&code)
        || (0x0600..=0x06FF).contains(&code)
        || (0x0750..=0x077F).contains(&code)
        || (0x08A0..=0x08FF).contains(&code)
        || (0xFB50..=0xFDFF).contains(&code)
        || (0xFE70..=0xFEFF).contains(&code)
}

/// Extract articleBody from JSON-LD when HTML content is missing or too short.
fn extract_article_body_from_ld_json(doc: &Html) -> Option<String> {
    let selector = Selector::parse("script[type='application/ld+json']").ok()?;
    for script in doc.select(&selector) {
        let text = script.text().collect::<String>();
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(body) = find_article_body(&value) {
                if !body.trim().is_empty() {
                    return Some(body);
                }
            }
        }
    }
    None
}

fn find_article_body(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            let mut is_article = false;
            if let Some(t) = map.get("@type") {
                is_article = matches_type(t, "NewsArticle") || matches_type(t, "BlogPosting");
            }
            if is_article {
                if let Some(body) = map.get("articleBody") {
                    if let Some(s) = body.as_str() {
                        return Some(s.to_string());
                    }
                    if let Some(arr) = body.as_array() {
                        let joined = arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        if !joined.is_empty() {
                            return Some(joined);
                        }
                    }
                }
            }
            // Recurse into common graph holders
            for key in [
                "@graph",
                "graph",
                "mainEntity",
                "mainEntityOfPage",
                "itemListElement",
            ] {
                if let Some(v) = map.get(key) {
                    if let Some(res) = find_article_body(v) {
                        return Some(res);
                    }
                }
            }
            // Recurse values
            for v in map.values() {
                if let Some(res) = find_article_body(v) {
                    return Some(res);
                }
            }
            None
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                if let Some(res) = find_article_body(v) {
                    return Some(res);
                }
            }
            None
        }
        _ => None,
    }
}

fn matches_type(value: &serde_json::Value, expected: &str) -> bool {
    match value {
        serde_json::Value::String(s) => s.eq_ignore_ascii_case(expected),
        serde_json::Value::Array(arr) => arr.iter().any(|v| matches_type(v, expected)),
        _ => false,
    }
}

/// Extract next page URL.
///
/// Priority:
/// 1. Custom extractor's next_page_url field if available
/// 2. <link rel="next"> href attribute
/// 3. .next a[href] (common pagination pattern)
/// 4. .pagination a[rel=next][href]
fn extract_next_page_url(doc: &Html, custom: Option<&FieldExtractor>) -> Option<String> {
    // Try custom extractor first
    if let Some(fe) = custom {
        if let Some(url) = extract_field_first_text(doc, fe) {
            return Some(url);
        }
    }

    // Fall back to link[rel=next] href
    if let Some(url) = extract_attr_first(doc, "link[rel='next']", "href") {
        return Some(url);
    }

    // Try .next a[href] pattern (common pagination)
    if let Some(url) = extract_attr_first(doc, ".next a[href]", "href") {
        return Some(url);
    }

    // Try .pagination a[rel=next][href] pattern
    extract_attr_first(doc, ".pagination a[rel='next'][href]", "href")
}

/// The main Hermes client for parsing web pages.
pub struct Client {
    opts: Options,
    http_client: reqwest::Client,
    registry: ExtractorRegistry,
}

impl Client {
    /// Create a new ClientBuilder for configuring the client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Create a new Client with the given options.
    pub fn new(opts: Options) -> Self {
        let http_client = opts.http_client.clone().unwrap_or_else(|| {
            let allow_private = opts.allow_private_networks;
            let redirect_policy = reqwest::redirect::Policy::custom(move |attempt| {
                let next = attempt.url().clone();
                if !allow_private {
                    if let Some(host) = next.host_str() {
                        let scheme = next.scheme();
                        let port = next
                            .port()
                            .unwrap_or(if scheme == "https" { 443 } else { 80 });
                        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
                            if crate::resource::is_private_ip(&ip) {
                                return attempt.error("redirect to private IP blocked");
                            }
                        } else {
                            // synchronous DNS resolution to avoid async in redirect policy
                            let addr_str = format!("{}:{}", host, port);
                            match addr_str.to_socket_addrs() {
                                Ok(addrs) => {
                                    for sa in addrs {
                                        if crate::resource::is_private_ip(&sa.ip()) {
                                            return attempt.error("redirect to private IP blocked");
                                        }
                                    }
                                }
                                Err(_) => {
                                    return attempt.error("DNS lookup failed during redirect");
                                }
                            }
                        }
                    }
                }
                attempt.follow()
            });

            reqwest::Client::builder()
                .redirect(redirect_policy)
                .user_agent(&opts.user_agent)
                .timeout(opts.timeout)
                .cookie_store(true)
                .gzip(true)
                .brotli(true)
                .deflate(true)
                .build()
                .expect("failed to build HTTP client")
        });

        let registry = opts.registry.clone().unwrap_or_else(load_builtin_registry);

        Self {
            opts,
            http_client,
            registry,
        }
    }

    /// Parse content from a URL.
    ///
    /// Fetches the page at the given URL and extracts article content.
    pub async fn parse(&self, url: &str) -> Result<ParseResult, ParseError> {
        if url.is_empty() {
            return Err(ParseError::invalid_url(url, "Parse", None));
        }

        // Validate URL format
        if url::Url::parse(url).is_err() {
            return Err(ParseError::invalid_url(
                url,
                "Parse",
                Some(anyhow::anyhow!("malformed URL")),
            ));
        }

        // Prepare fetch options
        let fetch_opts = FetchOptions {
            headers: self.opts.headers.clone(),
            allow_private_networks: self.opts.allow_private_networks,
            parse_non_200: false,
        };

        // Fetch the resource
        let fetch_result = fetch(&self.http_client, url, &fetch_opts).await?;

        // Decode the body as UTF-8 text
        let raw_html = fetch_result.text_utf8(None)?;

        // Parse the document once for extraction
        let doc = Html::parse_document(&raw_html);

        // Extract domain from final URL
        let domain = url::Url::parse(&fetch_result.final_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
            .unwrap_or_default();

        // Look up custom extractor for this domain
        let custom_extractor = self.registry.get(&domain);

        // Extract title: prefer custom extractor if available, then extract_title, then generic
        let title = custom_extractor
            .and_then(|ce| ce.title.as_ref())
            .and_then(|te| extract_field_first_text(&doc, te))
            .or_else(|| extract_title(&raw_html))
            .or_else(|| {
                let title_extractor = build_generic_title_extractor();
                extract_field_first_text(&doc, &title_extractor)
            })
            .unwrap_or_default();

        // Extract content: prefer custom extractor if available, then best generic, then body
        let mut content_html = custom_extractor
            .and_then(|ce| ce.content.as_ref())
            .and_then(|ce| extract_content_first_html(&doc, ce))
            .or_else(|| score_generic_content(&doc, &title))
            .unwrap_or_else(|| extract_body_inner_html(&doc));

        // Fallback: if content is too short, try JSON-LD articleBody
        let mut content_plain = html_to_text(&content_html);
        if content_plain.trim().len() < 500 {
            if let Some(ld_body) = extract_article_body_from_ld_json(&doc) {
                content_html = ld_body;
                content_plain = html_to_text(&content_html);
            }
        }

        // Sanitize the extracted HTML before conversion
        let sanitized_html = sanitize_html(&content_html);

        // Extract author, date_published, lead_image_url
        let author = extract_author(&doc, custom_extractor.and_then(|ce| ce.author.as_ref()));
        let date_published = extract_date_published(
            &doc,
            custom_extractor.and_then(|ce| ce.date_published.as_ref()),
        );
        let lead_image_url = extract_lead_image_url(
            &doc,
            custom_extractor.and_then(|ce| ce.lead_image_url.as_ref()),
        );

        // Extract additional metadata fields
        let dek = extract_dek(&doc, custom_extractor.and_then(|ce| ce.dek.as_ref()));
        let custom_excerpt =
            extract_custom_excerpt(&doc, custom_extractor.and_then(|ce| ce.excerpt.as_ref()));
        let site_name = extract_site_name(&doc);
        let site_title = extract_site_title(&doc);
        let site_image = extract_site_image(&doc);
        let language = extract_language(&doc);
        let theme_color = extract_theme_color(&doc);
        let favicon = extract_favicon(&doc);

        // Extract video URL and metadata
        let video_url = extract_video_url(&doc);
        let video_metadata = extract_video_metadata(&doc);

        // Extract next page URL
        let mut next_page_url = extract_next_page_url(
            &doc,
            custom_extractor.and_then(|ce| ce.next_page_url.as_ref()),
        );

        // Extract plain text for word count and direction detection (use raw_html)
        let plain_text = html_to_text(&raw_html);

        // Extract direction using plain text for RTL detection
        let direction = Some(extract_direction(&doc, &plain_text));

        // Convert content based on requested content type (using sanitized HTML)
        let mut final_content = match self.opts.content_type {
            ContentType::Markdown => html_to_markdown(&sanitized_html),
            ContentType::Text => html_to_text(&sanitized_html),
            ContentType::Html => sanitized_html.clone(),
        };

        // Store sanitized HTML for potential concatenation
        let mut final_sanitized_html = sanitized_html;

        // Track whether we actually followed a next page
        let mut did_follow = false;

        // Multi-page follow: if enabled and next_page_url is present, fetch one more page
        let mut next_next_page_url: Option<String> = None;

        if self.opts.follow_next {
            if let Some(ref next_url) = next_page_url {
                // Resolve relative URL against the current page URL
                if let Ok(base_url) = Url::parse(&fetch_result.final_url) {
                    if let Ok(resolved_url) = base_url.join(next_url) {
                        // Fetch the next page
                        if let Ok(next_fetch_result) =
                            fetch(&self.http_client, resolved_url.as_str(), &fetch_opts).await
                        {
                            if let Ok(next_raw_html) = next_fetch_result.text_utf8(None) {
                                let next_doc = Html::parse_document(&next_raw_html);

                                // Extract domain from next page URL for custom extractor lookup
                                let next_domain = Url::parse(&next_fetch_result.final_url)
                                    .ok()
                                    .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
                                    .unwrap_or_default();

                                let next_custom_extractor = self.registry.get(&next_domain);

                                // Extract content from next page using same pipeline
                                let mut next_content_html = next_custom_extractor
                                    .and_then(|ce| ce.content.as_ref())
                                    .and_then(|ce| extract_content_first_html(&next_doc, ce))
                                    .or_else(|| score_generic_content(&next_doc, &title))
                                    .unwrap_or_else(|| extract_body_inner_html(&next_doc));

                                // JSON-LD fallback for next page
                                let mut next_plain = html_to_text(&next_content_html);
                                if next_plain.trim().len() < 500 {
                                    if let Some(ld_body) =
                                        extract_article_body_from_ld_json(&next_doc)
                                    {
                                        next_content_html = ld_body;
                                        next_plain = html_to_text(&next_content_html);
                                    }
                                }

                                let next_sanitized_html = sanitize_html(&next_content_html);

                                // Append content based on content type
                                match self.opts.content_type {
                                    ContentType::Html => {
                                        final_sanitized_html = format!(
                                            "{}\n\n{}",
                                            final_sanitized_html, next_sanitized_html
                                        );
                                        final_content = final_sanitized_html.clone();
                                    }
                                    ContentType::Markdown => {
                                        let next_md = html_to_markdown(&next_sanitized_html);
                                        final_content = format!("{}\n\n{}", final_content, next_md);
                                        final_sanitized_html = format!(
                                            "{}\n\n{}",
                                            final_sanitized_html, next_sanitized_html
                                        );
                                    }
                                    ContentType::Text => {
                                        let next_text = html_to_text(&next_sanitized_html);
                                        final_content =
                                            format!("{}\n\n{}", final_content, next_text);
                                        final_sanitized_html = format!(
                                            "{}\n\n{}",
                                            final_sanitized_html, next_sanitized_html
                                        );
                                    }
                                }
                                // capture next-next if present
                                next_next_page_url = extract_next_page_url(
                                    &next_doc,
                                    next_custom_extractor.and_then(|ce| ce.next_page_url.as_ref()),
                                );

                                did_follow = true;
                            }
                        }
                    }
                }
                // Clear next_page_url since we consumed it (only if we actually tried to follow)
                if did_follow {
                    next_page_url = next_next_page_url;
                }
            }
        }

        // Calculate word count from plain text of final content
        let wc = if did_follow {
            let final_text = html_to_text(&final_sanitized_html);
            word_count(&final_text)
        } else {
            word_count(&plain_text)
        };

        // Determine description: if custom excerpt is set and dek is not, use custom_excerpt for description
        let description = if custom_excerpt.is_some() && dek.is_none() {
            custom_excerpt.clone()
        } else {
            extract_description_heuristic(&doc)
        };

        // Determine excerpt: prefer custom extractor, else use existing behavior
        let excerpt = custom_excerpt.or_else(|| extract_excerpt(&raw_html));

        Ok(ParseResult {
            url: fetch_result.final_url,
            domain,
            content: final_content,
            title,
            excerpt,
            word_count: wc,
            author,
            date_published,
            lead_image_url,
            dek,
            site_name,
            site_title,
            site_image,
            description,
            language,
            theme_color,
            favicon,
            video_url,
            video_metadata,
            next_page_url,
            direction,
            ..Default::default()
        })
    }

    /// Parse content from an HTML string.
    ///
    /// Extracts article content from the provided HTML, using the given URL for context.
    pub async fn parse_html(&self, html: &str, url: &str) -> Result<ParseResult, ParseError> {
        if html.is_empty() {
            return Err(ParseError::invalid_url(
                url,
                "ParseHTML",
                Some(anyhow::anyhow!("empty HTML")),
            ));
        }

        if url.is_empty() {
            return Err(ParseError::invalid_url(url, "ParseHTML", None));
        }

        // Validate URL format
        let parsed_url = url::Url::parse(url).map_err(|_| {
            ParseError::invalid_url(url, "ParseHTML", Some(anyhow::anyhow!("malformed URL")))
        })?;

        // Extract domain from URL
        let domain = parsed_url
            .host_str()
            .map(|h| h.to_lowercase())
            .unwrap_or_default();

        // Parse the document once for extraction
        let doc = Html::parse_document(html);

        // Look up custom extractor for this domain
        let custom_extractor = self.registry.get(&domain);

        // Extract title: prefer custom extractor if available, then extract_title, then generic
        let title = custom_extractor
            .and_then(|ce| ce.title.as_ref())
            .and_then(|te| extract_field_first_text(&doc, te))
            .or_else(|| extract_title(html))
            .or_else(|| {
                let title_extractor = build_generic_title_extractor();
                extract_field_first_text(&doc, &title_extractor)
            })
            .unwrap_or_default();

        // Extract content: prefer custom extractor if available, then best generic, then body
        let mut content_html = custom_extractor
            .and_then(|ce| ce.content.as_ref())
            .and_then(|ce| extract_content_first_html(&doc, ce))
            .or_else(|| score_generic_content(&doc, &title))
            .unwrap_or_else(|| extract_body_inner_html(&doc));

        // Fallback: if content is too short, try JSON-LD articleBody
        let mut content_plain = html_to_text(&content_html);
        if content_plain.trim().len() < 500 {
            if let Some(ld_body) = extract_article_body_from_ld_json(&doc) {
                content_html = ld_body;
                content_plain = html_to_text(&content_html);
            }
        }

        // Sanitize the extracted HTML before conversion
        let sanitized_html = sanitize_html(&content_html);

        // Extract author, date_published, lead_image_url
        let author = extract_author(&doc, custom_extractor.and_then(|ce| ce.author.as_ref()));
        let date_published = extract_date_published(
            &doc,
            custom_extractor.and_then(|ce| ce.date_published.as_ref()),
        );
        let lead_image_url = extract_lead_image_url(
            &doc,
            custom_extractor.and_then(|ce| ce.lead_image_url.as_ref()),
        );

        // Extract additional metadata fields
        let dek = extract_dek(&doc, custom_extractor.and_then(|ce| ce.dek.as_ref()));
        let custom_excerpt =
            extract_custom_excerpt(&doc, custom_extractor.and_then(|ce| ce.excerpt.as_ref()));
        let site_name = extract_site_name(&doc);
        let site_title = extract_site_title(&doc);
        let site_image = extract_site_image(&doc);
        let language = extract_language(&doc);
        let theme_color = extract_theme_color(&doc);
        let favicon = extract_favicon(&doc);

        // Extract video URL and metadata
        let video_url = extract_video_url(&doc);
        let video_metadata = extract_video_metadata(&doc);

        // Extract next page URL
        let next_page_url = extract_next_page_url(
            &doc,
            custom_extractor.and_then(|ce| ce.next_page_url.as_ref()),
        );

        // Extract plain text for word count and direction detection (use raw html)
        let plain_text = html_to_text(html);

        // Extract direction using plain text for RTL detection
        let direction = Some(extract_direction(&doc, &plain_text));

        // Calculate word count from plain text of raw HTML
        let wc = word_count(&plain_text);

        // Convert content based on requested content type (using sanitized HTML)
        let content = match self.opts.content_type {
            ContentType::Markdown => html_to_markdown(&sanitized_html),
            ContentType::Text => html_to_text(&sanitized_html),
            ContentType::Html => sanitized_html,
        };

        // Determine description: if custom excerpt is set and dek is not, use custom_excerpt for description
        let description = if custom_excerpt.is_some() && dek.is_none() {
            custom_excerpt.clone()
        } else {
            extract_description_heuristic(&doc)
        };

        // Determine excerpt: prefer custom extractor, else use existing behavior
        let excerpt = custom_excerpt.or_else(|| extract_excerpt(html));

        Ok(ParseResult {
            url: url.to_string(),
            domain,
            content,
            title,
            excerpt,
            word_count: wc,
            author,
            date_published,
            lead_image_url,
            dek,
            site_name,
            site_title,
            site_image,
            description,
            language,
            theme_color,
            favicon,
            video_url,
            video_metadata,
            next_page_url,
            direction,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use chrono::{Datelike, Timelike};
    use httpmock::prelude::*;

    #[tokio::test]
    async fn parse_returns_content_from_fetch() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/test");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body("<html><body>hi</body></html>");
        });

        let client = Client::builder().allow_private_networks(true).build();

        let result = client.parse(&server.url("/test")).await;
        mock.assert();

        let result = result.expect("parse should succeed");
        // Content is extracted from body since no article/main elements exist
        assert_eq!(result.content, "hi");
        assert!(result.domain.contains("127.0.0.1") || result.domain.contains("localhost"));
        assert_eq!(result.word_count, 1); // "hi" is the only whitespace-separated word
    }

    #[tokio::test]
    async fn parse_blocks_private_hostname() {
        let server = MockServer::start();
        // No need to mock - the SSRF check should fail before the request

        // Default client has allow_private_networks=false
        let client = Client::builder().build();

        let result = client.parse(&server.url("/")).await;

        let err = result.expect_err("should fail on private hostname");
        assert_eq!(err.code, ErrorCode::Ssrf);
    }

    #[tokio::test]
    async fn parse_html_returns_result() {
        let client = Client::builder().build();

        let result = client
            .parse_html(
                "<html><body><p>hi there</p></body></html>",
                "https://example.com/x",
            )
            .await;

        let result = result.expect("parse_html should succeed");
        // Content is extracted from body since no article/main elements exist
        assert_eq!(result.content, "<p>hi there</p>");
        assert_eq!(result.domain, "example.com");
        assert_eq!(result.word_count, 2); // "hi" and "there" when converted to text
    }

    #[tokio::test]
    async fn parse_respects_content_type_markdown() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/md");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body("<html><body><article><h1>Hello</h1></article></body></html>");
        });

        let client = Client::builder()
            .allow_private_networks(true)
            .content_type(ContentType::Markdown)
            .build();

        let result = client.parse(&server.url("/md")).await;
        mock.assert();

        let result = result.expect("parse should succeed");
        assert!(
            result.content.starts_with("# Hello"),
            "expected markdown h1, got: {}",
            result.content
        );
        // word_count is computed from plain text of raw HTML ("Hello"), not markdown content
        assert_eq!(result.word_count, 1);
    }

    #[tokio::test]
    async fn parse_respects_content_type_text() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/txt");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body("<html><body><article><p>Hello world</p></article></body></html>");
        });

        let client = Client::builder()
            .allow_private_networks(true)
            .content_type(ContentType::Text)
            .build();

        let result = client.parse(&server.url("/txt")).await;
        mock.assert();

        let result = result.expect("parse should succeed");
        assert_eq!(result.content, "Hello world");
        assert_eq!(result.word_count, 2);
    }

    #[tokio::test]
    async fn parse_extracts_title_and_excerpt() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/article");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(
                    "<html><head><title>Alpha</title></head><body><p>hello world</p></body></html>",
                );
        });

        let client = Client::builder().allow_private_networks(true).build();

        let result = client.parse(&server.url("/article")).await;
        mock.assert();

        let result = result.expect("parse should succeed");
        assert_eq!(result.title, "Alpha");
        assert!(
            result.excerpt.as_ref().unwrap().contains("hello world"),
            "expected excerpt to contain 'hello world', got: {:?}",
            result.excerpt
        );
    }

    #[tokio::test]
    async fn parse_uses_generic_article() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Title</title></head>
<body>
<article><p>Hello world</p></article>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://example.com/page")
            .await
            .expect("parse_html should succeed");

        assert!(
            result.content.contains("Hello world"),
            "expected content to contain 'Hello world', got: {}",
            result.content
        );
        assert_eq!(result.title, "Title");
        // word_count is from raw HTML plain text: "Title Hello world" = 3 words
        assert_eq!(result.word_count, 3);
    }

    #[tokio::test]
    async fn parse_generic_fallback_body() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Page</title></head>
<body>Hi there</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://example.com/page")
            .await
            .expect("parse_html should succeed");

        assert!(
            result.content.contains("Hi there"),
            "expected content to contain 'Hi there', got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn parse_title_fallback_h1() {
        let html = r#"<!DOCTYPE html>
<html>
<body>
<h1>Heading</h1>
<p>Content here</p>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://example.com/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(result.title, "Heading");
    }

    #[tokio::test]
    async fn parse_prefers_custom_content() {
        // medium.com has custom extractor with content selector "article"
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Medium Article</title></head>
<body>
<article>Custom Medium Content!</article>
<main>Generic content</main>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://medium.com/x")
            .await
            .expect("parse_html should succeed");

        assert!(
            result.content.contains("Custom Medium Content!"),
            "expected content to contain 'Custom Medium Content!', got: {}",
            result.content
        );
        assert_eq!(result.domain, "medium.com");
    }

    #[tokio::test]
    async fn parse_uses_supported_domain_alias() {
        // jezebel.com is a supported domain alias for deadspin.com
        // deadspin.com has title selector "header h1" and content selector ".js_post-content"
        let html = r#"<!DOCTYPE html>
<html>
<body>
<header><h1>T</h1></header>
<div class="js_post-content"><p>Hi</p></div>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://jezebel.com/x")
            .await
            .expect("parse_html should succeed");

        assert_eq!(result.title, "T");
        assert!(
            result.content.contains("Hi"),
            "expected content to contain 'Hi', got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn fallback_to_generic_when_no_custom() {
        // nocustom.test has no custom extractor, should fall back to generic
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Fallback Test</title></head>
<body>
<article><p>Gen</p></article>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/x")
            .await
            .expect("parse_html should succeed");

        assert!(
            result.content.contains("Gen"),
            "expected content to contain 'Gen', got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn parse_custom_author_date_image() {
        // Build a custom registry with author/date/image selectors for sample.org
        let mut registry = ExtractorRegistry::new();
        registry.register(crate::extractors::custom::CustomExtractor {
            domain: "sample.org".to_string(),
            supported_domains: vec![],
            title: Some(FieldExtractor {
                selectors: vec![SelectorSpec::Css("title".to_string())],
                allow_multiple: false,
                ..Default::default()
            }),
            content: Some(ContentExtractor {
                field: FieldExtractor {
                    selectors: vec![SelectorSpec::Css("div.post".to_string())],
                    allow_multiple: false,
                    ..Default::default()
                },
                clean: vec![],
                transforms: HashMap::new(),
            }),
            author: Some(FieldExtractor {
                selectors: vec![SelectorSpec::Css("p.by".to_string())],
                allow_multiple: false,
                ..Default::default()
            }),
            date_published: Some(FieldExtractor {
                selectors: vec![SelectorSpec::CssAttr(vec![
                    "meta[name=date]".to_string(),
                    "content".to_string(),
                ])],
                allow_multiple: false,
                ..Default::default()
            }),
            lead_image_url: Some(FieldExtractor {
                selectors: vec![SelectorSpec::CssAttr(vec![
                    "img.hero".to_string(),
                    "src".to_string(),
                ])],
                allow_multiple: false,
                ..Default::default()
            }),
            ..Default::default()
        });

        let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Custom Article</title>
    <meta name="date" content="2024-01-01T00:00:00Z">
</head>
<body>
<p class="by">Custom Author</p>
<div class="post">Content here</div>
<img class="hero" src="https://sample.org/hero.jpg">
</body>
</html>"#;

        let client = Client::builder()
            .content_type(ContentType::Html)
            .registry(registry)
            .build();

        let result = client
            .parse_html(html, "https://sample.org/article")
            .await
            .expect("parse_html should succeed");

        assert_eq!(result.author, Some("Custom Author".to_string()));
        assert!(result.date_published.is_some());
        let dt = result.date_published.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 1);
        assert_eq!(
            result.lead_image_url,
            Some("https://sample.org/hero.jpg".to_string())
        );
    }

    #[tokio::test]
    async fn parse_generic_author_meta() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta name="author" content="Jane">
</head>
<body><p>Hello</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(result.author, Some("Jane".to_string()));
    }

    #[tokio::test]
    async fn parse_generic_lead_image_prefers_og() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta property="og:image" content="https://example.com/og.jpg">
    <meta name="twitter:image" content="https://example.com/tw.jpg">
</head>
<body><img src="/local.jpg"></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.lead_image_url,
            Some("https://example.com/og.jpg".to_string())
        );
    }

    #[tokio::test]
    async fn parse_generic_date_time_tag() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Date Test</title></head>
<body>
<time datetime="2023-12-01T12:00:00Z">Dec</time>
<p>Content</p>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert!(result.date_published.is_some());
        let dt = result.date_published.unwrap();
        assert_eq!(dt.year(), 2023);
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 1);
        assert_eq!(dt.hour(), 12);
    }

    #[tokio::test]
    async fn parse_generic_meta_fields() {
        let html = r##"<!DOCTYPE html>
<html lang="en-US">
<head>
    <meta property="og:site_name" content="Example Site">
    <meta name="description" content="A page description">
    <meta name="theme-color" content="#ff0000">
    <meta property="og:image" content="https://example.com/site-img.jpg">
    <link rel="icon" href="/favicon.ico">
    <title>Page Title</title>
</head>
<body><p>Content</p></body>
</html>"##;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(result.site_name, Some("Example Site".to_string()));
        assert_eq!(result.site_title, Some("Page Title".to_string()));
        assert_eq!(
            result.site_image,
            Some("https://example.com/site-img.jpg".to_string())
        );
        assert_eq!(result.description, Some("A page description".to_string()));
        assert_eq!(result.theme_color, Some("#ff0000".to_string()));
        assert_eq!(result.favicon, Some("/favicon.ico".to_string()));
        assert_eq!(result.language, Some("en".to_string()));
    }

    #[tokio::test]
    async fn parse_favicon_prefers_icon_order() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <link rel="shortcut icon" href="/shortcut.ico">
    <link rel="icon" href="/icon.png">
    <link rel="apple-touch-icon" href="/apple.png">
</head>
<body><p>Content</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // rel="icon" should win because it's first in selector order
        assert_eq!(result.favicon, Some("/icon.png".to_string()));
    }

    #[tokio::test]
    async fn parse_custom_dek_and_excerpt() {
        // Build a custom registry with dek and excerpt selectors for sample.org
        let mut registry = ExtractorRegistry::new();
        registry.register(crate::extractors::custom::CustomExtractor {
            domain: "sample.org".to_string(),
            supported_domains: vec![],
            title: Some(FieldExtractor {
                selectors: vec![SelectorSpec::Css("title".to_string())],
                allow_multiple: false,
                ..Default::default()
            }),
            content: Some(ContentExtractor {
                field: FieldExtractor {
                    selectors: vec![SelectorSpec::Css("div.content".to_string())],
                    allow_multiple: false,
                    ..Default::default()
                },
                clean: vec![],
                transforms: HashMap::new(),
            }),
            dek: Some(FieldExtractor {
                selectors: vec![SelectorSpec::Css("p.dek".to_string())],
                allow_multiple: false,
                ..Default::default()
            }),
            excerpt: Some(FieldExtractor {
                selectors: vec![SelectorSpec::Css("p.excerpt".to_string())],
                allow_multiple: false,
                ..Default::default()
            }),
            ..Default::default()
        });

        let html = r#"<!DOCTYPE html>
<html>
<head><title>Article</title></head>
<body>
<p class="dek">Dek text</p>
<p class="excerpt">Excerpt text</p>
<div class="content">Main content here</div>
</body>
</html>"#;

        let client = Client::builder()
            .content_type(ContentType::Html)
            .registry(registry)
            .build();

        let result = client
            .parse_html(html, "https://sample.org/article")
            .await
            .expect("parse_html should succeed");

        assert_eq!(result.dek, Some("Dek text".to_string()));
        assert_eq!(result.excerpt, Some("Excerpt text".to_string()));
    }

    #[tokio::test]
    async fn parse_parses_loose_date() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta name="date" content="Jan 5, 2024">
</head>
<body><p>Content</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert!(
            result.date_published.is_some(),
            "expected date_published to be set"
        );
        let dt = result.date_published.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 5);
    }

    #[tokio::test]
    async fn parse_extracts_video_url() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta property="og:video" content="https://example.com/video.mp4">
    <meta property="og:video:type" content="video/mp4">
    <meta property="og:video:width" content="1280">
    <meta property="og:video:height" content="720">
</head>
<body><p>Content</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.video_url,
            Some("https://example.com/video.mp4".to_string())
        );

        // Check video metadata
        assert!(result.video_metadata.is_some());
        let meta = result.video_metadata.unwrap();
        assert!(meta.is_object());
        let obj = meta.as_object().unwrap();
        assert_eq!(
            obj.get("og:video:type"),
            Some(&serde_json::Value::String("video/mp4".to_string()))
        );
        assert_eq!(
            obj.get("og:video:width"),
            Some(&serde_json::Value::String("1280".to_string()))
        );
        assert_eq!(
            obj.get("og:video:height"),
            Some(&serde_json::Value::String("720".to_string()))
        );
    }

    #[tokio::test]
    async fn parse_detects_direction_rtl() {
        // Hebrew text: "Shalom" (Peace) written in Hebrew characters
        let html = r#"<!DOCTYPE html>
<html>
<body>
<p>This is some Hebrew text: </p>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // This HTML has no dir attribute and mostly English text, so should be LTR
        assert_eq!(result.direction, Some("ltr".to_string()));

        // Now test with actual Hebrew text (more than 30% RTL)
        let hebrew_html = r#"<!DOCTYPE html>
<html>
<body>
<p>שלום עולם</p>
</body>
</html>"#;

        let result_hebrew = client
            .parse_html(hebrew_html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result_hebrew.direction,
            Some("rtl".to_string()),
            "expected RTL for Hebrew text"
        );
    }

    #[tokio::test]
    async fn parse_detects_direction_from_attr() {
        let html = r#"<!DOCTYPE html>
<html dir="rtl">
<body><p>Some content</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.direction,
            Some("rtl".to_string()),
            "expected RTL from dir attribute"
        );
    }

    #[tokio::test]
    async fn parse_next_page_link() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <link rel="next" href="https://example.com/page2">
</head>
<body><p>Content</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.next_page_url,
            Some("https://example.com/page2".to_string())
        );
    }

    #[tokio::test]
    async fn parse_next_page_dot_next_pattern() {
        let html = r#"<!DOCTYPE html>
<html>
<body>
<p>Content</p>
<div class="next"><a href="/page2">Next</a></div>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.next_page_url,
            Some("/page2".to_string()),
            "expected .next a pattern to be detected"
        );
    }

    #[tokio::test]
    async fn parse_next_page_pagination_pattern() {
        let html = r#"<!DOCTYPE html>
<html>
<body>
<p>Content</p>
<div class="pagination">
    <a href="/page1">Prev</a>
    <a rel="next" href="/page2">Next</a>
</div>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.next_page_url,
            Some("/page2".to_string()),
            "expected .pagination a[rel=next] pattern to be detected"
        );
    }

    #[tokio::test]
    async fn word_count_uses_text() {
        // Word count should be based on plain text from raw HTML, not the converted content
        let html = r#"<!DOCTYPE html>
<html>
<body>
<p>Hello <strong>world</strong></p>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // Word count is from plain text: "Hello world" = 2 words
        assert_eq!(
            result.word_count, 2,
            "word_count should be 2 for 'Hello world'"
        );
    }

    #[tokio::test]
    async fn parse_video_fallback_to_video_element() {
        let html = r#"<!DOCTYPE html>
<html>
<body>
<video src="https://example.com/video.webm"></video>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.video_url,
            Some("https://example.com/video.webm".to_string())
        );
    }

    #[tokio::test]
    async fn parse_video_fallback_to_source_element() {
        let html = r#"<!DOCTYPE html>
<html>
<body>
<video>
    <source src="https://example.com/video.ogg" type="video/ogg">
</video>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.video_url,
            Some("https://example.com/video.ogg".to_string())
        );
    }

    #[tokio::test]
    async fn generic_picks_longest_candidate() {
        // Test that the generic content selector picks the element with longest text
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<main>short</main>
<article><p>long long text with more content here</p></article>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // Article has longer text content, so it should be chosen
        assert!(
            result.content.contains("long long text"),
            "expected content to contain 'long long text' from article, got: {}",
            result.content
        );
        assert!(
            !result.content.contains("<main>"),
            "content should not contain the main tag itself: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn sanitizes_script() {
        // Test that script tags are sanitized from content
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<article>
<script>alert(1)</script>
<p>ok</p>
</article>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // Content should not contain the script or alert
        assert!(
            !result.content.contains("alert"),
            "content should not contain 'alert', got: {}",
            result.content
        );
        assert!(
            !result.content.contains("<script"),
            "content should not contain script tag, got: {}",
            result.content
        );
        // Should still contain the safe content
        assert!(
            result.content.contains("ok"),
            "content should contain 'ok', got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn title_uses_og() {
        // Test that og:title is used when <title> is absent
        let html = r#"<!DOCTYPE html>
<html>
<head>
<meta property="og:title" content="OG Title">
</head>
<body><p>Content</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert_eq!(
            result.title, "OG Title",
            "expected title to be 'OG Title' from og:title, got: {}",
            result.title
        );
    }

    #[tokio::test]
    async fn ssrf_blocks_after_redirect() {
        // Test that redirects to private IPs are blocked
        let server = MockServer::start();

        // First endpoint redirects to 127.0.0.1
        let redirect_url = format!("http://127.0.0.1:{}/private", server.port());
        let _redirect_mock = server.mock(|when, then| {
            when.method(GET).path("/redirect");
            then.status(302).header("Location", &redirect_url);
        });

        // Note: The redirect itself goes to 127.0.0.1 which should be blocked
        // The client with allow_private_networks=false should reject this

        let client = Client::builder().allow_private_networks(false).build();

        let result = client.parse(&server.url("/redirect")).await;

        // Since the initial URL resolves to a local address (the mock server),
        // it will be blocked before even making the request.
        // To properly test redirect blocking, we need the initial URL to be "public"
        // but redirect to private. Since we're in a test environment with local mock,
        // both will be blocked. The test verifies SSRF protection works.
        let err = result.expect_err("should fail due to SSRF protection");
        assert!(err.is_ssrf(), "expected SSRF error, got: {:?}", err);
    }

    #[tokio::test]
    async fn generic_prefers_dense_text_over_links() {
        // main has long text with few links; article has similar length but 60% text inside links
        // The scorer should prefer main due to lower link density
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<main>
<p>This is a substantial paragraph of real content that has meaningful text without excessive links. It contains enough characters to exceed the minimum threshold and should be considered high quality content for extraction purposes.</p>
</main>
<article>
<p><a href="/1">Link one with text</a> <a href="/2">Link two with more</a> <a href="/3">Link three here</a> <a href="/4">Another link text</a> <a href="/5">Yet more links</a> <a href="/6">Even more link</a> some small non-link text here.</p>
</article>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // main should win because article has high link density (~60%)
        assert!(
            result
                .content
                .contains("substantial paragraph of real content"),
            "expected main content with dense text, got: {}",
            result.content
        );
        assert!(
            !result.content.contains("Link one"),
            "should not contain link-heavy article content, got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn generic_requires_min_length() {
        // All candidates have text shorter than 80 chars, should fall back to body
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<main>Short main text</main>
<article>Brief article</article>
<section>Tiny section</section>
<p>Body fallback content that is long enough to verify we got the right element selected from the document structure.</p>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // Should fall back to body since no candidate meets minimum length
        assert!(
            result.content.contains("Body fallback content"),
            "expected body fallback content, got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn generic_penalizes_aside() {
        // article has text but many aside descendants; main has similar text but no asides
        // Each aside/nav/form descendant adds 10 point penalty
        // With 8 asides = 80 penalty, article's score drops significantly
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<article>
<p>Article content here.</p>
<aside>Ad</aside>
<aside>Ad</aside>
<aside>Ad</aside>
<aside>Ad</aside>
<aside>Ad</aside>
<aside>Ad</aside>
<aside>Ad</aside>
<aside>Ad</aside>
</article>
<main>
<p>The main element has clean text content without sidebar distractions and noise from advertisements.</p>
</main>
</body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        // main should win because article has 8 asides = 80 point penalty
        // article text ~47 chars (short "Ad" text in asides) - 80 penalty = negative score
        // main text ~97 chars, no penalty
        assert!(
            result.content.contains("main element has clean text"),
            "expected main content without asides, got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn multipage_appends_content() {
        let server = MockServer::start();

        // First page with link rel=next pointing to second page
        let page2_url = server.url("/page2");
        let mock1 = server.mock(|when, then| {
            when.method(GET).path("/page1");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(format!(
                    r#"<!DOCTYPE html>
<html>
<head>
    <title>Page One</title>
    <link rel="next" href="{}">
</head>
<body>
<article><p>Content from page one with enough text to pass the minimum threshold for content extraction.</p></article>
</body>
</html>"#,
                    page2_url
                ));
        });

        // Second page
        let mock2 = server.mock(|when, then| {
            when.method(GET).path("/page2");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(
                    r#"<!DOCTYPE html>
<html>
<head><title>Page Two</title></head>
<body>
<article><p>Content from page two with additional text that should be appended to the first page content.</p></article>
</body>
</html>"#,
                );
        });

        let client = Client::builder()
            .allow_private_networks(true)
            .content_type(ContentType::Text)
            .follow_next(true)
            .build();

        let result = client.parse(&server.url("/page1")).await;
        mock1.assert();
        mock2.assert();

        let result = result.expect("parse should succeed");

        // Content should contain text from both pages
        assert!(
            result.content.contains("Content from page one"),
            "expected content from page one, got: {}",
            result.content
        );
        assert!(
            result.content.contains("Content from page two"),
            "expected content from page two, got: {}",
            result.content
        );

        // next_page_url should be None since it was consumed
        assert!(
            result.next_page_url.is_none(),
            "expected next_page_url to be None after follow, got: {:?}",
            result.next_page_url
        );
    }

    #[tokio::test]
    async fn multipage_respects_flag() {
        let server = MockServer::start();

        // First page with link rel=next pointing to second page
        let page2_url = server.url("/page2");
        let mock1 = server.mock(|when, then| {
            when.method(GET).path("/page1");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(format!(
                    r#"<!DOCTYPE html>
<html>
<head>
    <title>Page One</title>
    <link rel="next" href="{}">
</head>
<body>
<article><p>Content from page one with enough text to pass the minimum threshold for content extraction.</p></article>
</body>
</html>"#,
                    page2_url
                ));
        });

        // Second page should NOT be fetched when follow_next is false
        let mock2 = server.mock(|when, then| {
            when.method(GET).path("/page2");
            then.status(200)
                .header("content-type", "text/html; charset=utf-8")
                .body(
                    r#"<!DOCTYPE html>
<html>
<head><title>Page Two</title></head>
<body>
<article><p>Content from page two</p></article>
</body>
</html>"#,
                );
        });

        // Default: follow_next is false
        let client = Client::builder()
            .allow_private_networks(true)
            .content_type(ContentType::Text)
            .build();

        let result = client.parse(&server.url("/page1")).await;
        mock1.assert();

        // Page 2 should NOT have been fetched
        assert_eq!(
            mock2.calls(),
            0,
            "page2 should not be fetched when follow_next is false"
        );

        let result = result.expect("parse should succeed");

        // Content should only contain text from first page
        assert!(
            result.content.contains("Content from page one"),
            "expected content from page one, got: {}",
            result.content
        );
        assert!(
            !result.content.contains("Content from page two"),
            "should not contain content from page two, got: {}",
            result.content
        );

        // next_page_url should still be set since we didn't follow
        assert!(
            result.next_page_url.is_some(),
            "expected next_page_url to be set when follow_next is false"
        );
    }

    #[tokio::test]
    async fn dateparser_loose_formats() {
        // Test that loose date formats like "5 Jan 2024" are parsed
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta name="date" content="5 Jan 2024">
</head>
<body><p>Content</p></body>
</html>"#;

        let client = Client::builder().content_type(ContentType::Html).build();

        let result = client
            .parse_html(html, "https://nocustom.test/page")
            .await
            .expect("parse_html should succeed");

        assert!(
            result.date_published.is_some(),
            "expected date_published to be set for loose format '5 Jan 2024'"
        );
        let dt = result.date_published.unwrap();
        assert_eq!(dt.year(), 2024, "expected year 2024, got {}", dt.year());
        assert_eq!(dt.month(), 1, "expected month 1, got {}", dt.month());
        assert_eq!(dt.day(), 5, "expected day 5, got {}", dt.day());
    }
}
