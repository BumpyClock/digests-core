# Building digests-core

## Prerequisites

### Rust
- Rust 1.70+ (stable toolchain recommended)
- `cargo` and `rustc` in your PATH

### Platform-specific tools

#### Linux (Ubuntu/Debian)
```bash
sudo apt-get install build-essential
```

#### macOS
```bash
xcode-select --install
```

#### Windows
- Visual Studio 2019/2022 with "C++ build tools"
- Or [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

## Building the Project

### 1. Clone and setup
```bash
git clone https://github.com/BumpyClock/digests-core.git
cd digests-core
cargo build
```

### 2. Run tests
```bash
cargo test    # all tests
cargo test -q # quiet output
```

### 3. Build FFI library
```bash
cargo build -p digests-ffi --release
```

This produces:
- Linux: `target/release/libdigests_ffi.so`
- macOS: `target/release/libdigests_ffi.dylib`
- Windows: `target/release/digests_ffi.dll`

### 4. Build CLI tool
```bash
cargo build -p digests-cli --release
```

Produces: `target/release/digests-cli` (Unix) or `target/release/digests-cli.exe` (Windows)

## Development Setup

### Install rustup (if needed)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Update Rust
```bash
rustup update
```

### Use nightly for development (optional)
```bash
rustup toolchain install nightly
rustup override set nightly
```

## IDE Support

### VS Code
Install these extensions:
- Rust Analyzer (rust-lang.rust-analyzer)
- Code Runner (formulahendry.code-runner)

### IntelliJ IDEA
- Install the Rust plugin
- Configure cargo as the external builder

### Vim/Neovim
- Install [rust.vim](https://github.com/rust-lang/rust.vim)
- Or use [coc.nvim](https://github.com/neoclide/coc.nvim) with coc-rust-analyzer

## Configuration

### Environment Variables
- `RUST_BACKTRACE=1` - Enable detailed backtraces on panic
- `CARGO_TERM_COLOR=always` - Always colored output
- `CARGO_INCREMENTAL=1` - Enable incremental compilation (faster rebuilds)

### Cargo Configuration
Create `.cargo/config.toml` for project-specific settings:
```toml
[build]
rustflags = ["-C", "target-cpu=native"]

[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=native"]

[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=native"]
```

## Troubleshooting Build Issues

### "no such file or directory" on Windows
Ensure you have Visual Studio build tools installed:
```bash
rustup component add rust-src
rustup target add x86_64-pc-windows-msvc
```

### Linker errors on Linux
Install development packages:
```bash
sudo apt-get install libssl-dev
```

### macOS linker errors
Install OpenSSL:
```bash
brew install openssl
```

Then set environment variables:
```bash
export OPENSSL_DIR=$(brew --prefix openssl)
export RUSTFLAGS="-L native=$OPENSSL_DIR/lib"
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig"
```

### Cross-compilation
For other targets:
```bash
# Install target
rustup target add aarch64-unknown-linux-gnu

# Build for target
cargo build --target aarch64-unknown-linux-gnu --release
```

## Performance Tips

1. Use `--release` for production builds
2. Enable LTO (Link Time Optimization) in `.cargo/config.toml`:
   ```toml
   [build]
   lto = true
   ```
3. Use parallel jobs with `cargo build -j 4`
4. Enable incremental compilation for development

## Release Process

1. Update version in `crates/*/Cargo.toml`
2. Run full test suite
3. Build all artifacts:
   ```bash
   cargo build --release -p digests-ffi -p digests-cli
   ```
4. Test FFI library with example consumers
5. Commit and create release