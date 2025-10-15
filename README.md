# oktofetch

[![CI](https://github.com/mhalder/oktofetch/workflows/CI/badge.svg)](https://github.com/mhalder/oktofetch/actions)
[![codecov](https://codecov.io/gh/mhalder/oktofetch/branch/main/graph/badge.svg)](https://codecov.io/gh/mhalder/oktofetch)
[![Crates.io](https://img.shields.io/crates/v/oktofetch.svg)](https://crates.io/crates/oktofetch)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A simple CLI tool to manage GitHub release binaries for Linux x86_64.

## Features

- Download and install binaries from GitHub releases
- Version tracking and updates
- Extract from `.tar.gz`, `.tgz`, and `.zip` archives

## Usage

Add a tool from a GitHub repository:

```bash
oktofetch add derailed/k9s
```

Update a tool to the latest release:

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

List all managed tools:

```bash
oktofetch list
```

Show tool information:

```bash
oktofetch info k9s
```

Remove a tool:

```bash
oktofetch remove k9s
```

## Configuration

Default install directory: `~/.local/bin`

Change install directory:

```bash
oktofetch config set install_dir /custom/path
```

Show current configuration:

```bash
oktofetch config show
```

## Config File

The configuration is stored in a TOML file at:

- Linux: `~/.config/oktofetch/config.toml`

### Structure

```toml
[settings]
install_dir = "/home/user/.local/bin"

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

- `install_dir`: Directory where binaries are installed
  - Supports tilde expansion: `~/bin` → `/home/user/bin`
  - Supports environment variables: `$HOME/.local/bin` or `${HOME}/.local/bin`

### Tool Entries

Each `[[tools]]` entry tracks an installed tool:

- `name`: Tool identifier (required)
- `repo`: GitHub repository in `owner/repo` format (required)
- `version`: Currently installed version tag (optional)
- `binary_name`: Custom binary name if different from release asset (optional)
- `asset_pattern`: Pattern to match release assets (optional)

## License

[MIT](LICENSE)
