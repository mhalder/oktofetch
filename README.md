# oktofetch

[![CI](https://github.com/mhalder/oktofetch/workflows/CI/badge.svg)](https://github.com/mhalder/oktofetch/actions)
[![codecov](https://codecov.io/gh/mhalder/oktofetch/branch/main/graph/badge.svg)](https://codecov.io/gh/mhalder/oktofetch)
[![Crates.io](https://img.shields.io/crates/v/oktofetch.svg)](https://crates.io/crates/oktofetch)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A CLI tool to manage GitHub release binaries for Linux x86_64. Automatically downloads, extracts, and installs binaries from GitHub releases with version tracking.

## Features

- **GitHub Release Management** - Download and install binaries directly from GitHub releases
- **Version Tracking** - Track installed versions and update to latest releases
- **Smart Asset Selection** - Automatically selects the best asset for your platform (prefers tar.gz > zip > standalone binary)
- **Archive Support** - Extracts `.tar.gz`, `.tgz`, `.tar.bz2`, `.tbz`, and `.zip` archives
- **Standalone Binary Support** - Handles direct ELF binary downloads
- **Platform Detection** - Matches assets for Linux x86_64/amd64/x64
- **GitHub Token Support** - Works with both classic and fine-grained personal access tokens
- **Path Expansion** - Supports `~` and environment variables in configuration

## Installation

### From crates.io

```bash
cargo install oktofetch
```

### From source

```bash
git clone https://github.com/mhalder/oktofetch.git
cd oktofetch
cargo install --path .
```

## Usage

### Add a tool

Add a tool from a GitHub repository:

```bash
# Using owner/repo format
oktofetch add derailed/k9s

# Using full GitHub URL
oktofetch add https://github.com/jesseduffield/lazygit

# With custom name
oktofetch add derailed/k9s --name k9s-dev

# With custom binary name (when archive contains differently named binary)
oktofetch add owner/repo --binary custom-binary-name
```

### Update tools

Update a specific tool:

```bash
oktofetch update k9s
```

Update all managed tools:

```bash
oktofetch update --all
```

Force reinstall (even if version matches):

```bash
oktofetch update k9s --force
```

### List tools

List all managed tools with their versions:

```bash
oktofetch list
```

### Show tool info

Display detailed information about a tool:

```bash
oktofetch info k9s
```

### Remove a tool

Remove a tool from management (binary is not deleted):

```bash
oktofetch remove k9s
```

### Configuration

Show current configuration:

```bash
oktofetch config show
```

Change install directory:

```bash
oktofetch config set install_dir /custom/path
```

### Verbose output

Add `-v` or `--verbose` for detailed output:

```bash
oktofetch update k9s -v
```

## Configuration File

Configuration is stored in TOML format at `~/.config/oktofetch/config.toml`.

### Example

```toml
[settings]
install_dir = "~/.local/bin"

[[tools]]
name = "k9s"
repo = "derailed/k9s"
version = "v0.32.5"
asset_pattern = "Linux_amd64"

[[tools]]
name = "lazygit"
repo = "jesseduffield/lazygit"
binary_name = "lazygit"
version = "v0.44.1"
```

### Settings

| Key           | Description                            | Default        |
| ------------- | -------------------------------------- | -------------- |
| `install_dir` | Directory where binaries are installed | `~/.local/bin` |

The `install_dir` supports:

- Tilde expansion: `~/bin` expands to `/home/user/bin`
- Environment variables: `$HOME/.local/bin` or `${HOME}/.local/bin`

### Tool Entries

Each `[[tools]]` entry tracks an installed tool:

| Field           | Required | Description                                |
| --------------- | -------- | ------------------------------------------ |
| `name`          | Yes      | Tool identifier used in commands           |
| `repo`          | Yes      | GitHub repository in `owner/repo` format   |
| `version`       | No       | Currently installed version tag            |
| `binary_name`   | No       | Custom binary name (defaults to tool name) |
| `asset_pattern` | No       | Pattern to match specific release assets   |

## GitHub Authentication

For private repositories or to avoid rate limits, set a GitHub token:

```bash
export GITHUB_TOKEN=your_token_here
```

Both classic tokens and fine-grained personal access tokens (`github_pat_*`) are supported.

## Platform Support

Currently supports **Linux x86_64** only. Asset matching looks for:

- `linux` AND one of: `x86_64`, `amd64`, or `x64`

## Exit Codes

| Code | Description                      |
| ---- | -------------------------------- |
| 0    | Success                          |
| 1    | Tool not found / General error   |
| 2    | GitHub API error                 |
| 3    | No suitable release for platform |
| 4    | Configuration error              |
| 7    | Download failed                  |
| 8    | Extraction failed                |
| 9    | Binary not found in archive      |
| 10   | IO error                         |
| 11   | HTTP error                       |

## License

[MIT](LICENSE)
