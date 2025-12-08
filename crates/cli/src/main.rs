// ABOUTME: CLI for parsing feeds using digests-core feed parser.
// ABOUTME: Fetches a feed from URL or file/stdin and prints JSON for verification.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use clap::Parser;
use digests_feed::parse_feed_bytes;
use serde_json::json;

/// Parse one or more RSS/Atom feeds and output JSON.
#[derive(Parser, Debug)]
#[command(name = "digests-cli")]
#[command(about = "Parse feeds with digests-core and print JSON", long_about = None)]
struct Args {
    /// Feed URL(s) (http/https) or local file paths. Use "-" to read one feed from stdin.
    #[arg(required = true)]
    targets: Vec<String>,

    /// Override feed_url value (only valid when a single target is provided).
    #[arg(long)]
    feed_url: Option<String>,

    /// Output compact JSON instead of pretty.
    #[arg(long, default_value_t = false)]
    compact: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.targets.len() > 1 && args.feed_url.is_some() {
        bail!("--feed-url is only valid when parsing a single target");
    }

    let mut results = Vec::new();

    for target in &args.targets {
        let feed_url = args
            .feed_url
            .clone()
            .unwrap_or_else(|| target.clone());

        match load_bytes(target).and_then(|bytes| parse_feed_bytes(&bytes, &feed_url).map_err(anyhow::Error::new)) {
            Ok(feed) => results.push(json!({
                "feed_url": feed_url,
                "ok": true,
                "feed": feed,
                "error": null
            })),
            Err(err) => results.push(json!({
                "feed_url": feed_url,
                "ok": false,
                "feed": null,
                "error": err.to_string()
            })),
        }
    }

    // Output format:
    // - Single target and ok => emit the feed object (backward compatible)
    // - Otherwise emit an envelope with feeds array and counts
    let output = if args.targets.len() == 1 {
        if let Some(first) = results.first() {
            if first.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                first.get("feed").cloned().unwrap_or_else(|| json!({}))
            } else {
                json!({ "feeds": results, "total_feeds": results.len(), "parsed": 0, "failed": 1 })
            }
        } else {
            json!({})
        }
    } else {
        let parsed = results.iter().filter(|r| r.get("ok").and_then(|v| v.as_bool()) == Some(true)).count();
        let failed = results.len() - parsed;
        json!({
            "feeds": results,
            "total_feeds": results.len(),
            "parsed": parsed,
            "failed": failed
        })
    };

    if args.compact {
        println!("{}", serde_json::to_string(&output)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

fn load_bytes(target: &str) -> Result<Vec<u8>> {
    if target == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        return Ok(buf);
    }

    if target.starts_with("http://") || target.starts_with("https://") {
        let resp = reqwest::blocking::get(target)?.error_for_status()?;
        let bytes = resp.bytes()?;
        return Ok(bytes.to_vec());
    }

    let path = PathBuf::from(target);
    if !path.exists() {
        return Err(anyhow!("file not found: {}", target));
    }
    Ok(fs::read(path)?)
}
