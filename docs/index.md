# digests-core Documentation

Welcome to the digests-core documentation. This shared parsing library provides feed parsing, article extraction, and a C ABI for multi-platform applications.

## Quick Start

- **Building**: [Building](building.md) - Build instructions and requirements
- **Overview**: [Overview](overview.md) - Architecture and components
- **CLI Usage**: [CLI](cli.md) - Command-line interface guide
- **Feed Parsing**: [Feed](feed.md) - RSS/Atom/podcast parsing
- **Article Extraction**: [Hermes](hermes.md) - ReaderView and metadata extraction
- **FFI Interface**: [FFI](ffi.md) - C ABI usage and examples
- **Troubleshooting**: [Troubleshooting](troubleshooting.md) - Common issues and solutions

## Project Structure

```
digests-core/
├── crates/
│   ├── feed/          # Feed parsing (RSS/Atom/podcast)
│   ├── hermes/         # Article extraction and metadata
│   ├── ffi/           # C ABI surface
│   └── cli/           # Developer CLI
├── docs/              # This documentation
└── README.md          # Project overview
```

## Key Features

- **Feed Parsing**: Parse RSS, Atom, and podcast feeds with robust error handling
- **Article Extraction**: Extract clean article content and metadata from HTML
- **C ABI**: Cross-platform interface for embedding in other languages
- **CLI Tool**: Command-line interface for testing and development
- **Comprehensive Testing**: Golden tests ensure consistent output

## Getting Help

If you encounter issues:
1. Check the [Troubleshooting guide](troubleshooting.md)
2. Review the [FFI documentation](ffi.md) for platform-specific notes
3. Open an issue on GitHub with minimal reproduction steps

## Contributing

See the source code comments and existing tests for patterns. All changes should maintain the existing ABI compatibility.