# prefer

[![CI](https://github.com/LimpidTech/prefer.rs/workflows/CI/badge.svg)](https://github.com/LimpidTech/prefer.rs/actions)
[![codecov](https://codecov.io/gh/LimpidTech/prefer.rs/branch/main/graph/badge.svg)](https://codecov.io/gh/LimpidTech/prefer.rs)
[![Crates.io](https://img.shields.io/crates/v/prefer.svg)](https://crates.io/crates/prefer)
[![Documentation](https://docs.rs/prefer/badge.svg)](https://docs.rs/prefer)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A library for managing application configurations with support for multiple file formats.

## Overview

`prefer` helps you manage application configurations while providing users the flexibility of using whatever configuration format fits their needs. It automatically discovers configuration files in standard system locations and supports JSON, JSON5, YAML, TOML, INI, and XML formats.

## Features

- **Format-agnostic**: Supports JSON, JSON5, YAML, TOML, INI, and XML
- **Automatic discovery**: Searches standard system paths for configuration files
- **Async by design**: Non-blocking operations for file I/O
- **File watching**: Monitor configuration files for changes
- **Dot-notation access**: Access nested values with simple key strings
- **Cross-platform**: Works on Linux, macOS, and Windows

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
prefer = "0.1"
```

## Usage

### Basic Example

```rust
use prefer::load;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load("settings").await?;

    let username: String = config.get("auth.username").await?;
    let port: u16 = config.get("server.port").await?;

    println!("Username: {}", username);
    println!("Port: {}", port);

    Ok(())
}
```

### Watching for Changes

```rust
use prefer::watch;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut receiver = watch("settings").await?;

    while let Some(config) = receiver.recv().await {
        let port: u16 = config.get("server.port").await?;
        println!("Configuration updated! Port: {}", port);
    }

    Ok(())
}
```

## Supported Formats

The library automatically detects and parses the following formats:

- JSON (`.json`)
- JSON5 (`.json5`, `.jsonc`) - with comments and trailing commas
- YAML (`.yaml`, `.yml`)
- TOML (`.toml`)
- INI (`.ini`)
- XML (`.xml`)

## Configuration Discovery

`prefer` searches for configuration files in the following locations (in order):

### Unix/Linux/macOS
- Current directory
- `$XDG_CONFIG_HOME` (or `~/.config`)
- `$XDG_CONFIG_DIRS`
- `$HOME`
- `/usr/local/etc`
- `/usr/etc`
- `/etc`

### Windows
- Current directory
- `%USERPROFILE%`
- `%APPDATA%`
- `%ProgramData%`
- `%SystemRoot%`

## Features

By default, all format parsers are enabled. You can disable specific formats:

```toml
[dependencies]
prefer = { version = "0.1", default-features = false, features = ["json5"] }
```

Available features:
- `json5` - JSON5 format support
- `xml` - XML format support
- `ini` - INI format support

Note: JSON, YAML, and TOML are always available.

## Examples

See the [examples](examples/) directory for more usage examples:

- `basic.rs` - Basic configuration loading
- `watch.rs` - File watching for live updates

Run examples with:

```bash
cargo run --example basic
cargo run --example watch
```

## Related Projects

- [prefer.js](https://github.com/LimpidTech/prefer.js) - JavaScript/TypeScript version
- [prefer.py](https://github.com/LimpidTech/prefer.py) - Python version
- [prefer.go](https://github.com/LimpidTech/prefer.go) - Go version

## License

MIT
