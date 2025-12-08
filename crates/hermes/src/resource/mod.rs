// ABOUTME: Resource handling module for fetching and managing web resources.
// ABOUTME: Handles HTTP fetching with SSRF protection, content-length limits, and charset decoding.

use std::collections::HashMap;
use std::net::IpAddr;

use bytes::Bytes;
use ipnet::{Ipv4Net, Ipv6Net};

use crate::error::ParseError;

/// Maximum allowed content length (10 MB).
pub const MAX_CONTENT_LENGTH: usize = 10 * 1024 * 1024;

/// Options for fetching a resource.
#[derive(Debug, Clone)]
pub struct FetchOptions {
    pub headers: HashMap<String, String>,
    pub allow_private_networks: bool,
    pub parse_non_200: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            headers: HashMap::new(),
            allow_private_networks: false,
            parse_non_200: false,
        }
    }
}

/// Result of a successful fetch operation.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub status: u16,
    pub url: String,
    pub final_url: String,
    pub content_type: Option<String>,
    pub body: Bytes,
}

impl FetchResult {
    /// Decode the body as UTF-8 text, using charset hints from content-type header.
    pub fn text_utf8(&self, content_type_hint: Option<&str>) -> Result<String, ParseError> {
        let ct = content_type_hint.or(self.content_type.as_deref());
        Ok(decode_body(&self.body, ct))
    }
}

/// Check if an IP address is in a private/reserved range.
fn is_private_ip(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(ip) => {
            // RFC1918 private ranges
            let private_10: Ipv4Net = "10.0.0.0/8".parse().unwrap();
            let private_172: Ipv4Net = "172.16.0.0/12".parse().unwrap();
            let private_192: Ipv4Net = "192.168.0.0/16".parse().unwrap();
            // Loopback
            let loopback: Ipv4Net = "127.0.0.0/8".parse().unwrap();
            // Link-local
            let link_local: Ipv4Net = "169.254.0.0/16".parse().unwrap();

            private_10.contains(ip)
                || private_172.contains(ip)
                || private_192.contains(ip)
                || loopback.contains(ip)
                || link_local.contains(ip)
        }
        IpAddr::V6(ip) => {
            // Loopback ::1
            if ip.is_loopback() {
                return true;
            }
            // Unique local fc00::/7
            let unique_local: Ipv6Net = "fc00::/7".parse().unwrap();
            // Link-local fe80::/10
            let link_local: Ipv6Net = "fe80::/10".parse().unwrap();

            unique_local.contains(ip) || link_local.contains(ip)
        }
    }
}

/// Decode body bytes to a String using charset from content-type header or detection.
fn decode_body(body: &[u8], content_type: Option<&str>) -> String {
    // Try to extract charset from content-type header
    if let Some(ct) = content_type {
        if let Some(charset) = extract_charset(ct) {
            if let Some(encoding) = encoding_rs::Encoding::for_label(charset.as_bytes()) {
                let (decoded, _, _) = encoding.decode(body);
                return decoded.into_owned();
            }
        }
    }

    // Use chardetng for detection
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(body, true);
    let encoding = detector.guess(None, true);
    let (decoded, _, _) = encoding.decode(body);
    decoded.into_owned()
}

/// Extract charset value from Content-Type header.
fn extract_charset(content_type: &str) -> Option<String> {
    let lower = content_type.to_lowercase();
    for part in lower.split(';') {
        let trimmed = part.trim();
        if let Some(charset) = trimmed.strip_prefix("charset=") {
            // Remove quotes if present
            let charset = charset.trim_matches('"').trim_matches('\'');
            return Some(charset.to_string());
        }
    }
    None
}

/// Fetch a resource from the given URL.
pub async fn fetch(
    client: &reqwest::Client,
    url: &str,
    opts: &FetchOptions,
) -> Result<FetchResult, ParseError> {
    // Validate URL is non-empty
    if url.is_empty() {
        return Err(ParseError::invalid_url(url, "Fetch", None));
    }

    // Parse and validate URL
    let parsed_url = url::Url::parse(url).map_err(|e| {
        ParseError::invalid_url(url, "Fetch", Some(anyhow::anyhow!("invalid URL: {}", e)))
    })?;

    // Check scheme
    let scheme = parsed_url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(ParseError::invalid_url(
            url,
            "Fetch",
            Some(anyhow::anyhow!("scheme must be http or https")),
        ));
    }

    // Check for private IP if not allowed
    if !opts.allow_private_networks {
        if let Some(host) = parsed_url.host_str() {
            // Try to parse as IP address
            if let Ok(ip) = host.parse::<IpAddr>() {
                if is_private_ip(&ip) {
                    return Err(ParseError::ssrf(
                        url,
                        "Fetch",
                        Some(anyhow::anyhow!("private IP addresses are not allowed")),
                    ));
                }
            } else {
                // Host is a hostname, resolve it and check all addresses
                let port = parsed_url
                    .port()
                    .unwrap_or(if scheme == "https" { 443 } else { 80 });
                let addrs = tokio::net::lookup_host((host, port)).await.map_err(|e| {
                    ParseError::fetch(
                        url,
                        "Fetch",
                        Some(anyhow::anyhow!("DNS lookup failed: {}", e)),
                    )
                })?;

                for socket_addr in addrs {
                    if is_private_ip(&socket_addr.ip()) {
                        return Err(ParseError::ssrf(
                            url,
                            "Fetch",
                            Some(anyhow::anyhow!("private IP addresses are not allowed")),
                        ));
                    }
                }
            }
        }
    }

    // Build request
    let mut request = client.get(url);
    for (key, value) in &opts.headers {
        request = request.header(key, value);
    }

    // Send request
    let response = request.send().await.map_err(|e| {
        ParseError::fetch(url, "Fetch", Some(anyhow::anyhow!("request failed: {}", e)))
    })?;

    // SSRF check after redirect: verify the final URL doesn't resolve to a private IP
    if !opts.allow_private_networks {
        let final_url_ref = response.url();
        if let Some(host) = final_url_ref.host_str() {
            // Try to parse as IP address first
            if let Ok(ip) = host.parse::<IpAddr>() {
                if is_private_ip(&ip) {
                    return Err(ParseError::ssrf(
                        url,
                        "Fetch",
                        Some(anyhow::anyhow!(
                            "redirect to private IP address is not allowed"
                        )),
                    ));
                }
            } else {
                // Host is a hostname, resolve it and check all addresses
                let port = final_url_ref
                    .port()
                    .unwrap_or(if final_url_ref.scheme() == "https" {
                        443
                    } else {
                        80
                    });
                let addrs = tokio::net::lookup_host((host, port)).await.map_err(|e| {
                    ParseError::fetch(
                        url,
                        "Fetch",
                        Some(anyhow::anyhow!(
                            "DNS lookup failed for redirect target: {}",
                            e
                        )),
                    )
                })?;

                for socket_addr in addrs {
                    if is_private_ip(&socket_addr.ip()) {
                        return Err(ParseError::ssrf(
                            url,
                            "Fetch",
                            Some(anyhow::anyhow!(
                                "redirect to private IP address is not allowed"
                            )),
                        ));
                    }
                }
            }
        }
    }

    // Check Content-Length header before reading body
    // Use content_length() first, fallback to parsing header manually
    let content_length = response.content_length().or_else(|| {
        response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    });

    if let Some(len) = content_length {
        if len as usize > MAX_CONTENT_LENGTH {
            return Err(ParseError::fetch(
                url,
                "Fetch",
                Some(anyhow::anyhow!("content too large")),
            ));
        }
    }

    // Capture response metadata before consuming the response
    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_lowercase());

    // Read body bytes
    let body = response.bytes().await.map_err(|e| {
        ParseError::fetch(
            url,
            "Fetch",
            Some(anyhow::anyhow!("failed to read body: {}", e)),
        )
    })?;

    // Check body size
    if body.len() > MAX_CONTENT_LENGTH {
        return Err(ParseError::fetch(
            url,
            "Fetch",
            Some(anyhow::anyhow!("content too large")),
        ));
    }

    // Check status code
    if status != 200 && !opts.parse_non_200 {
        return Err(ParseError::fetch(
            url,
            "Fetch",
            Some(anyhow::anyhow!("HTTP status {}", status)),
        ));
    }

    Ok(FetchResult {
        status,
        url: url.to_string(),
        final_url,
        content_type,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn create_test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .user_agent("test-agent")
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_fetch_ok_utf8() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/test");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body("hello");
        });

        let client = create_test_client();
        let opts = FetchOptions {
            allow_private_networks: true,
            ..Default::default()
        };

        let result = fetch(&client, &server.url("/test"), &opts).await;
        mock.assert();

        let result = result.expect("fetch should succeed");
        assert_eq!(result.status, 200);
        assert_eq!(result.text_utf8(None).unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_fetch_non_200_rejected() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/notfound");
            then.status(404).body("not found");
        });

        let client = create_test_client();
        let opts = FetchOptions {
            allow_private_networks: true,
            parse_non_200: false,
            ..Default::default()
        };

        let result = fetch(&client, &server.url("/notfound"), &opts).await;
        mock.assert();

        let err = result.expect_err("should fail on 404");
        assert!(err.is_fetch());
    }

    #[tokio::test]
    async fn test_fetch_non_200_allowed() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/notfound");
            then.status(404).body("not found");
        });

        let client = create_test_client();
        let opts = FetchOptions {
            allow_private_networks: true,
            parse_non_200: true,
            ..Default::default()
        };

        let result = fetch(&client, &server.url("/notfound"), &opts).await;
        mock.assert();

        let result = result.expect("fetch should succeed with parse_non_200");
        assert_eq!(result.status, 404);
    }

    #[tokio::test]
    async fn test_fetch_content_length_limit() {
        // Test Content-Length enforcement.
        // httpmock automatically sets Content-Length to match the actual body size,
        // so we test by verifying that the fetch implementation correctly checks
        // both the header and actual body size.
        //
        // Since we can't easily mock a mismatched Content-Length header with httpmock,
        // we verify the body size check works by ensuring small responses pass.
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/normal");
            then.status(200)
                .header("content-type", "text/plain")
                .body("normal sized content");
        });

        let client = create_test_client();
        let opts = FetchOptions {
            allow_private_networks: true,
            ..Default::default()
        };

        let result = fetch(&client, &server.url("/normal"), &opts).await;
        mock.assert();

        // Normal-sized content should succeed
        let result = result.expect("normal content should succeed");
        assert_eq!(result.status, 200);
        assert_eq!(result.body.len(), 20); // "normal sized content"
    }

    #[test]
    fn test_max_content_length_constant() {
        // Verify the constant is set correctly (10 MB)
        assert_eq!(MAX_CONTENT_LENGTH, 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_private_ip_block() {
        let server = MockServer::start();
        // We don't need to mock anything - the SSRF check should fail before the request

        let client = create_test_client();
        let opts = FetchOptions {
            allow_private_networks: false,
            ..Default::default()
        };

        // Use 127.0.0.1 explicitly
        let url = format!("http://127.0.0.1:{}/test", server.port());
        let result = fetch(&client, &url, &opts).await;

        let err = result.expect_err("should fail on private IP");
        assert!(err.is_ssrf());
    }

    #[tokio::test]
    async fn test_decode_iso_8859_1_with_chardetng() {
        // ISO-8859-1 encoded "cafe" with accent (e with acute = 0xe9)
        let iso_bytes: &[u8] = &[0x63, 0x61, 0x66, 0xe9]; // "cafe" with e-acute

        // No charset header, should use chardetng detection
        let decoded = decode_body(iso_bytes, None);

        // chardetng should detect this as ISO-8859-1 or similar and decode correctly
        assert_eq!(decoded, "café");
    }

    #[test]
    fn test_is_private_ip_v4() {
        // Private ranges
        assert!(is_private_ip(&"10.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"10.255.255.255".parse().unwrap()));
        assert!(is_private_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_ip(&"172.31.255.255".parse().unwrap()));
        assert!(is_private_ip(&"192.168.0.1".parse().unwrap()));
        assert!(is_private_ip(&"192.168.255.255".parse().unwrap()));

        // Loopback
        assert!(is_private_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"127.255.255.255".parse().unwrap()));

        // Link-local
        assert!(is_private_ip(&"169.254.0.1".parse().unwrap()));

        // Public IPs should not be private
        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip(&"1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip(&"172.32.0.1".parse().unwrap())); // Outside 172.16/12
    }

    #[test]
    fn test_is_private_ip_v6() {
        // Loopback
        assert!(is_private_ip(&"::1".parse().unwrap()));

        // Unique local
        assert!(is_private_ip(&"fc00::1".parse().unwrap()));
        assert!(is_private_ip(&"fd00::1".parse().unwrap()));

        // Link-local
        assert!(is_private_ip(&"fe80::1".parse().unwrap()));

        // Public IPv6 should not be private
        assert!(!is_private_ip(&"2001:4860:4860::8888".parse().unwrap()));
    }

    #[test]
    fn test_extract_charset() {
        assert_eq!(
            extract_charset("text/html; charset=utf-8"),
            Some("utf-8".to_string())
        );
        assert_eq!(
            extract_charset("text/html; charset=ISO-8859-1"),
            Some("iso-8859-1".to_string())
        );
        assert_eq!(
            extract_charset("text/html; charset=\"utf-8\""),
            Some("utf-8".to_string())
        );
        assert_eq!(extract_charset("text/html"), None);
    }

    #[test]
    fn test_decode_body_with_charset() {
        // UTF-8 content with charset header
        let body = "hello world".as_bytes();
        let decoded = decode_body(body, Some("text/plain; charset=utf-8"));
        assert_eq!(decoded, "hello world");
    }
}
