// ABOUTME: CLI binary for the Hermes web content parser.
// ABOUTME: Parses URLs or HTML files and outputs extracted content as JSON.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use digests_hermes::{Client, ContentType};

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
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut had_error = false;

    if let Some(html_path) = &args.html {
        // HTML file mode
        let url = args.url.as_ref().unwrap();
        match fs::read_to_string(html_path) {
            Ok(html_content) => match client.parse_html(&html_content, url).await {
                Ok(result) => {
                    results.push(serde_json::to_value(&result).unwrap());
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
                    results.push(serde_json::to_value(&result).unwrap());
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
        let output_str = if let Some(output_path) = &args.output {
            // Write to file
            let json_str = if results.len() == 1 {
                serde_json::to_string_pretty(&results[0]).unwrap()
            } else {
                serde_json::to_string_pretty(&results).unwrap()
            };
            if let Err(e) = fs::write(output_path, &json_str) {
                eprintln!("error writing to {:?}: {}", output_path, e);
                had_error = true;
            }
            None
        } else {
            // Print to stdout
            if results.len() == 1 {
                Some(serde_json::to_string_pretty(&results[0]).unwrap())
            } else {
                // Multiple results: one JSON object per line
                let lines: Vec<String> = results
                    .iter()
                    .map(|r| serde_json::to_string_pretty(r).unwrap())
                    .collect();
                Some(lines.join("\n"))
            }
        };

        if let Some(s) = output_str {
            println!("{}", s);
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
