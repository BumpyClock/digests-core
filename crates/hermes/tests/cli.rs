// ABOUTME: Integration tests for the hermes CLI binary.
// ABOUTME: Tests HTML file parsing and multiple URL handling.

use assert_cmd::assert::OutputAssertExt;
use assert_cmd::cargo::CommandCargoExt;
use httpmock::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn hermes_cmd() -> Command {
    Command::cargo_bin("hermes").unwrap()
}

#[test]
fn parse_html_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let html_path = temp_dir.path().join("test.html");

    let html_content = r#"<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
<article><p>Hi there</p></article>
</body>
</html>"#;

    fs::write(&html_path, html_content).unwrap();

    hermes_cmd()
        .arg("--html")
        .arg(&html_path)
        .arg("--url")
        .arg("https://example.com")
        .arg("--format")
        .arg("text")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hi there"));
}

#[test]
fn multiple_urls_outputs_lines() {
    let server = MockServer::start();

    let mock1 = server.mock(|when, then| {
        when.method(GET).path("/page1");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>Page One</p></body></html>");
    });

    let mock2 = server.mock(|when, then| {
        when.method(GET).path("/page2");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body><p>Page Two</p></body></html>");
    });

    let url1 = server.url("/page1");
    let url2 = server.url("/page2");

    let output = hermes_cmd()
        .arg("--allow-private-networks")
        .arg(&url1)
        .arg(&url2)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    mock1.assert();
    mock2.assert();

    let stdout = String::from_utf8(output).unwrap();

    // Count JSON objects by counting occurrences of "url" field
    // Each result should have a "url" field
    let url_count = stdout.matches("\"url\":").count();
    assert_eq!(
        url_count, 2,
        "expected 2 JSON objects with 'url' field, got {}",
        url_count
    );

    // Verify both pages are represented
    assert!(
        stdout.contains("Page One") || stdout.contains("/page1"),
        "expected output to reference page1"
    );
    assert!(
        stdout.contains("Page Two") || stdout.contains("/page2"),
        "expected output to reference page2"
    );
}

#[test]
fn timing_flag_prints_elapsed() {
    let temp_dir = TempDir::new().unwrap();
    let html_path = temp_dir.path().join("test.html");

    let html_content = "<html><body><p>Test</p></body></html>";
    fs::write(&html_path, html_content).unwrap();

    hermes_cmd()
        .arg("--html")
        .arg(&html_path)
        .arg("--url")
        .arg("https://example.com")
        .arg("--timing")
        .assert()
        .success()
        .stderr(predicate::str::contains("elapsed:"))
        .stderr(predicate::str::contains("ms"));
}

#[test]
fn output_to_file() {
    let temp_dir = TempDir::new().unwrap();
    let html_path = temp_dir.path().join("test.html");
    let output_path = temp_dir.path().join("output.json");

    let html_content = "<html><body><article><p>Content</p></article></body></html>";
    fs::write(&html_path, html_content).unwrap();

    hermes_cmd()
        .arg("--html")
        .arg(&html_path)
        .arg("--url")
        .arg("https://example.com")
        .arg("-o")
        .arg(&output_path)
        .assert()
        .success();

    let output_content = fs::read_to_string(&output_path).unwrap();
    assert!(
        output_content.contains("\"content\":"),
        "output file should contain JSON with content field"
    );
}

#[test]
fn missing_url_with_html_fails() {
    let temp_dir = TempDir::new().unwrap();
    let html_path = temp_dir.path().join("test.html");

    let html_content = "<html><body><p>Test</p></body></html>";
    fs::write(&html_path, html_content).unwrap();

    hermes_cmd()
        .arg("--html")
        .arg(&html_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("--url is required"));
}

#[test]
fn no_args_fails() {
    hermes_cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least one URL is required"));
}
