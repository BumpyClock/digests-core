# CLI Usage

The `digests-cli` provides a command-line interface for parsing feeds and extracting article content.

## Installation

### Build from source
```bash
cargo build -p digests-cli --release
./target/release/digests-cli --help
```

### Using cargo run (no build required)
```bash
cargo run -p digests-cli -- --help
```

## Command Reference

### Main Commands

#### `parse-feed` - Parse RSS/Atom feeds
```bash
# Parse from URL
digests-cli parse-feed https://example.com/feed.xml

# Parse from file
digests-cli parse-feed feed.xml

# Parse from stdin
cat feed.xml | digests-cli parse-feed -

# Output compact JSON
digests-cli parse-feed --compact https://example.com/feed.xml

# Save to file
digests-cli parse-feed https://example.com/feed.xml > output.json
```

#### `extract` - Extract article content
```bash
# Extract from URL (fetches and extracts)
digests-cli extract https://example.com/article

# Extract from local HTML file
digests-cli extract article.html

# Extract from stdin
cat article.html | digests-cli extract -

# Specify custom user agent
digests-cli extract --user-agent "MyApp/1.0" https://example.com/article

# Set timeout (seconds)
digests-cli extract --timeout 10 https://example.com/article
```

#### `parse` - Parse both feed and extract articles
```bash
# Parse feed and extract all articles
digests-cli parse https://example.com/feed.xml

# Parse single article
digests-cli parse https://example.com/article

# Limit number of articles to process
digests-cli parse --limit 5 https://example.com/feed.xml
```

### Options

#### Common Options
```bash
-h, --help           Print help
-V, --version        Print version
-v, --verbose        Enable verbose output
-q, --quiet          Suppress output
--compact            Output compact JSON (no pretty-printing)
--color <when>       Coloring: auto, always, never [default: auto]
```

#### Parse Options
```bash
--limit <N>          Limit number of items to process [default: 100]
--offset <N>         Start from item N [default: 0]
--sort-by <field>    Sort results by field (date, title, length) [default: none]
--reverse            Sort in reverse order
```

#### Extract Options
```bash
--user-agent <UA>    Custom user agent [default: digests-cli/x.y.z]
--timeout <SECONDS>  Request timeout in seconds [default: 30]
--follow-redirects  Follow HTTP redirects [default: true]
--insecure           Skip SSL certificate verification
--headers <K:V>      Add custom HTTP headers (can be used multiple times)
--cookie <K:V>       Add cookies (can be used multiple times)
```

## Examples

### Basic Feed Parsing
```bash
# Parse a simple RSS feed
digests-cli parse-feed https://news.ycombinator.com/rss

# Parse with compact output
digests-cli parse-feed --compact https://feeds.feedburner.com/oreilly/radar

# Save results to file
digests-cli parse-feed https://example.com/feed.xml > feed_results.json
```

### Article Extraction
```bash
# Extract article content
digests-cli extract https://blog.example.com/my-post

# Extract from local file
digests-cli extract article.html

# Extract with verbose output
digests-cli extract -v https://example.com/article

# Extract with custom timeout
digests-cli extract --timeout 15 https://slow-site.com/article
```

### Combined Feed Processing
```bash
# Parse feed and extract all articles
digests-cli parse https://example.com/feed.xml

# Parse only first 3 articles
digests-cli parse --limit 3 https://example.com/feed.xml

# Sort articles by publish date
digests-cli parse --sort-by date --reverse https://example.com/feed.xml
```

### Advanced Usage
```bash
# Parse with custom headers
digests-cli parse https://example.com/feed.xml \
  --headers "Authorization: Bearer token" \
  --headers "X-Custom: value"

# Parse with cookies
digests-cli parse https://example.com/feed.xml \
  --cookie "session=abc123" \
  --cookie "pref=dark"

# Parse with follow redirects disabled
digests-cli parse --no-follow-redirects https://example.com/feed.xml

# Insecure HTTP for development
digests-cli extract --insecure https://self-signed.example.com/article
```

## Output Format

### Feed Output
```json
{
  "title": "Example Feed",
  "link": "https://example.com",
  "description": "Sample feed description",
  "language": "en-US",
  "items": [
    {
      "title": "Article Title",
      "link": "https://example.com/article1",
      "description": "Article summary",
      "content": "Full article content",
      "author": "Author Name",
      "pub_date": "2024-01-01T00:00:00Z",
      "guid": "article-id-123"
    }
  ]
}
```

### Article Output
```json
{
  "url": "https://example.com/article",
  "title": "Article Title",
  "content": "Cleaned article content...",
  "length": 2456,
  "excerpt": "Brief summary",
  "author": "Author Name",
  "published_date": "2024-01-01T00:00:00Z",
  "language": "en",
  "reading_time": 3,
  "confidence": 0.92,
  "metadata": {
    "title": "Article Title",
    "author": "Author Name",
    "published_date": "2024-01-01T00:00:00Z",
    "excerpt": "Brief summary",
    "keywords": ["tech", "news", "article"],
    "site_name": "Example Site"
  }
}
```

## Error Handling

### Exit Codes
- `0`: Success
- `1`: General error
- `2`: Invalid arguments
- `3`: Network error
- `4`: Parse error
- `5`: Timeout

### Error Examples
```bash
# Invalid URL
$ digests-cli parse-feed not-a-url
Error: Invalid URL: not-a-url

# Network error
$ digests-cli parse-feed https://down-site.com/feed
Error: Network error: Connection timeout

# Parse error
$ digests-cli parse-feed invalid.xml
Error: Parse error: Invalid XML
```

## Configuration

### Environment Variables
```bash
# Custom user agent
export DIGESTS_CLI_USER_AGENT="MyApp/1.0"

# Default timeout
export DIGESTS_CLI_TIMEOUT=60

# Default output format
export DIGESTS_CLI_FORMAT=compact
```

### Config File
Create `~/.config/digests-cli/config.toml`:
```toml
[default]
user_agent = "MyApp/1.0"
timeout = 30
follow_redirects = true
insecure = false

[output]
compact = false
color = "always"

[headers]
Authorization = "Bearer token"
X-Custom = "value"
```

## Integration Examples

### Shell Scripting
```bash
#!/bin/bash

# Process multiple feeds
for feed in "https://site1.com/feed" "https://site2.com/rss"; do
    echo "Processing $feed..."
    digests-cli parse "$feed" > "output_$(basename "$feed").json"
done

# Check extraction confidence
if digests-cli extract "https://example.com/article" | jq '.confidence' | grep -q '0.8'; then
    echo "Good extraction quality"
fi
```

### Pipeline Processing
```bash
# Fetch, parse, and filter articles
curl -s "https://example.com/feed.xml" | \
    digests-cli parse-feed - | \
    jq '.items | map(select(.content | length > 1000))' | \
    digests-cli parse - | \
    jq '.metadata | select(.confidence > 0.9)'
```

### Watchdog for Updates
```bash
#!/bin/bash

url="https://example.com/feed.xml"
last_hash=""

while true; do
    current_hash=$(curl -s "$url" | sha256sum)

    if [ "$current_hash" != "$last_hash" ]; then
        echo "Feed updated!"
        digests-cli parse "$url" | jq '.items[0].title'
        last_hash="$current_hash"
    fi

    sleep 300  # Check every 5 minutes
done
```

## Testing

### Test Feed Parsing
```bash
# Test with sample feeds
digests-cli parse-feed tests/fixtures/sample_rss.xml
digests-cli parse-feed tests/fixtures/sample_atom.xml

# Test with real feeds
digests-cli parse-feed https://blog.rust-lang.org/feed.xml
```

### Test Article Extraction
```bash
# Test with sample articles
mkdir -p test_output
for url in "https://example.com/article1" "https://example.com/article2"; do
    digests-cli extract "$url" > "test_output/$(basename "$url").json"
done
```

## Troubleshooting

### Common Issues

1. **SSL Errors**: Use `--insecure` for development (not recommended for production)
   ```bash
   digests-cli extract --insecure https://self-signed.example.com
   ```

2. **Timeout**: Increase timeout for slow sites
   ```bash
   digests-cli extract --timeout 60 https://slow-site.com
   ```

3. **Large Feeds**: Use `--limit` to process only first N items
   ```bash
   digests-cli parse --limit 10 https://large-feed.com
   ```

4. **Rate Limiting**: Add delays between requests
   ```bash
   for feed in feed1.xml feed2.xml; do
       digests-cli parse "$feed"
       sleep 1
   done
   ```

### Debug Mode
```bash
# Enable verbose output
digests-cli -v parse-feed https://example.com/feed.xml

# Trace HTTP requests
export DIGESTS_CLI_TRACE=1
digests-cli extract https://example.com/article
```

### Performance Tips
1. Use `--compact` for faster output parsing
2. Process multiple feeds in parallel
3. Cache results when processing the same URLs repeatedly
4. Use appropriate timeouts to prevent hanging