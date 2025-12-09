// ABOUTME: CLI binary for the Hermes web content parser.
// ABOUTME: Parses URLs or HTML files and outputs extracted content in various formats.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use digests_hermes::{Client, ContentType, ParseResult};

#[derive(Parser, Debug)]
#[command(name = "hermes")]
#[command(about = "Parse web content and extract article data")]
struct Args {
    /// Output format: html (default), markdown/md, text/txt
    #[arg(short = 'f', long = "format", default_value = "html")]
    format: String,

    /// Output file path (default: stdout)
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Output as JSON instead of raw content (matches Go -f json behavior)
    #[arg(long = "json")]
    json_output: bool,

    /// HTML file to parse (requires --url)
    #[arg(long = "html")]
    html: Option<PathBuf>,

    /// URL context for HTML file parsing (required with --html)
    #[arg(long = "url")]
    url: Option<String>,

    /// Print elapsed time in ms to stderr
    #[arg(long = "timing")]
    timing: bool,

    /// Allow fetching from private/local networks
    #[arg(long = "allow-private-networks")]
    allow_private_networks: bool,

    /// Follow next_page_url to fetch and append content from the next page
    #[arg(long = "follow-next")]
    follow_next: bool,

    /// URLs to parse (fetch mode)
    #[arg()]
    urls: Vec<String>,
}

fn parse_content_type(format: &str) -> ContentType {
    match format.to_lowercase().as_str() {
        "markdown" | "md" => ContentType::Markdown,
        "text" | "txt" => ContentType::Text,
        _ => ContentType::Html,
    }
}

/// Format output based on whether JSON output is requested.
///
/// When json_output is true: outputs full JSON (like Go's -f json)
/// When json_output is false: outputs raw content (like Go's -f html/markdown/text)
fn format_output(results: &[ParseResult], json_output: bool) -> String {
    if json_output {
        // JSON output mode - serialize full result(s)
        if results.len() == 1 {
            serde_json::to_string_pretty(&results[0]).unwrap()
        } else {
            serde_json::to_string_pretty(results).unwrap()
        }
    } else {
        // Raw content mode - output just the content field(s)
        if results.len() == 1 {
            results[0].content.clone()
        } else {
            // Multiple results: separate content with double newlines
            results
                .iter()
                .map(|r| r.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n")
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    // Validate args
    if args.html.is_some() && args.url.is_none() {
        eprintln!("error: --url is required when using --html");
        return ExitCode::from(1);
    }

    if args.html.is_none() && args.urls.is_empty() {
        eprintln!("error: at least one URL is required, or use --html with --url");
        return ExitCode::from(1);
    }

    if args.html.is_some() && !args.urls.is_empty() {
        eprintln!("error: cannot use both --html and positional URLs");
        return ExitCode::from(1);
    }

    let content_type = parse_content_type(&args.format);
    let client = Client::builder()
        .content_type(content_type)
        .allow_private_networks(args.allow_private_networks)
        .follow_next(args.follow_next)
        .build();

    let start = Instant::now();
    let mut results: Vec<ParseResult> = Vec::new();
    let mut had_error = false;

    if let Some(html_path) = &args.html {
        // HTML file mode
        let url = args.url.as_ref().unwrap();
        match fs::read_to_string(html_path) {
            Ok(html_content) => match client.parse_html(&html_content, url).await {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    eprintln!("error parsing HTML: {}", e);
                    had_error = true;
                }
            },
            Err(e) => {
                eprintln!("error reading file {:?}: {}", html_path, e);
                had_error = true;
            }
        }
    } else {
        // URL fetch mode
        for url in &args.urls {
            match client.parse(url).await {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    eprintln!("error parsing {}: {}", url, e);
                    had_error = true;
                }
            }
        }
    }

    let elapsed = start.elapsed();

    // Output results
    if !results.is_empty() {
        let output_str = format_output(&results, args.json_output);

        if let Some(output_path) = &args.output {
            // Write to file
            if let Err(e) = fs::write(output_path, &output_str) {
                eprintln!("error writing to {:?}: {}", output_path, e);
                had_error = true;
            }
        } else {
            // Print to stdout
            println!("{}", output_str);
        }
    }

    // Print timing if requested
    if args.timing {
        let _ = writeln!(io::stderr(), "elapsed: {}ms", elapsed.as_millis());
    }

    if had_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
