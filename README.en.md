# tm-watcher

[![GitHub Release](https://img.shields.io/github/v/release/zzerding/tm-exclude-watcher)](https://github.com/zzerding/tm-exclude-watcher/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-Edition%202024-orange?logo=rust)](https://www.rust-lang.org/)
![Release Workflow](https://github.com/zzerding/tm-exclude-watcher/actions/workflows/release.yml/badge.svg)

Tired of your Time Machine backup drive filling up with `node_modules`, `target`, and `build` directories? `tm-watcher` is a lightweight macOS command-line tool that automatically excludes reproducible dependency folders from your backups — and cleans up stale records when those folders are deleted. Run `tm-watcher daemon start` once, and never manage Time Machine exclusion lists by hand again. Faster backups, longer-lived drives, more time for actual coding.

## Features

- 🔍 **Recursive scanning**: scan all matching subdirectories under a given path
- 👀 **Dry-run preview**: use `scan --dry-run` to preview exclusions without changing system state
- 🚫 **Auto exclusion**: invokes `tmutil` to add directories to the Time Machine exclusion list
- 📊 **Record management**: tracks every exclusion in a local database
- 🧹 **Smart cleanup**: detects stale records and syncs them with the Time Machine exclusion list
- 🩺 **Health check**: checks Time Machine, configuration, database, and daemon status

## Requirements

- **Operating system**: macOS 10.13 or later
- **Time Machine**: must be enabled and configured with a backup disk

## Installation

### Homebrew

After the stable release, install via Homebrew:

```bash
brew tap zzerding/tap
brew install tm-watcher
```

Homebrew does not auto-start the daemon. Run `tm-watcher daemon start` to enable background monitoring; check status with `tm-watcher daemon status`; stop with `tm-watcher daemon stop`.

### GitHub Release binary

Download the tarball for your version and architecture from GitHub Releases:

```text
tm-watcher-v<version>-aarch64-apple-darwin.tar.gz
tm-watcher-v<version>-x86_64-apple-darwin.tar.gz
```

Extract and move `tm-watcher` into your `PATH`:

```bash
VERSION=<version>
ARCH=aarch64-apple-darwin
shasum -a 256 -c SHA256SUMS
tar -xzf "tm-watcher-v${VERSION}-${ARCH}.tar.gz"
install -m 0755 tm-watcher*/tm-watcher /usr/local/bin/tm-watcher
```

### Build from source

Requires the Rust toolchain. Install it if needed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Build and install from source:

```bash
git clone https://github.com/zzerding/tm-exclude-watcher.git
cd tm-exclude-watcher
cargo install --path .
```

## Usage

### Scan existing project directories

Recursively scan a path and automatically exclude all dependency directories matching the rules:

```bash
tm-watcher scan ~/Documents/src
```

Example output:

```text
Scanning: /Users/biz/Documents/src

Scan complete:
  Newly excluded: 12 directories
  Skipped: 3 directories (already excluded)
```

Preview directories to be excluded without invoking `tmutil` or writing to the database:

```bash
tm-watcher scan ~/Documents/src --dry-run
```

### List excluded directories

```bash
tm-watcher list
```

Example output:

```text
Exclusion records: 3, known total size 2.4 GB, unknown size 1

#  Size      Rule          Checked at        Path
1  2.3 GB    target        2026-06-11 10:30  ~/Code/project-b/target
2  145.2 MB  node_modules  2026-06-11 10:30  ~/Code/project-a/node_modules
3  Unknown   vendor        Not checked       ~/Code/project-c/vendor
```

### Clean stale records

Check whether recorded directories still exist, remove stale records, and refresh directory sizes:

```bash
tm-watcher clean
```

Example output:

```text
Cleanup complete:
  Removed: 2 records
  Checked: 15 records
  Errors: 0
```

### Daemon mode

Start the daemon to automatically watch configured paths and periodically clean stale records:

```bash
# Start
tm-watcher daemon start

# Check status
tm-watcher daemon status

# Stop
tm-watcher daemon stop
```

**Features:**
- **Auto-start on login**: the daemon starts automatically when the user logs in (macOS LaunchAgent)
- **Crash recovery**: automatically restarts on unexpected exit; does not restart after a normal `stop` command
- **Log path**: `~/.local/share/tm-watcher/daemon.log`
- **Upgrade hint**: `tm-watcher daemon status` checks the daemon state; if the LaunchAgent still points to an old binary, it prompts you to run `tm-watcher daemon stop && tm-watcher daemon start`

**Developer notes:**
- The plist points to the absolute path of `current_exe()`, which is `target/debug/tm-watcher` in development mode
- Re-run `tm-watcher daemon start` after `cargo clean`
- After manually replacing the binary, use `tm-watcher daemon status` to check whether the daemon needs to be restarted

### View daemon logs

```bash
# Show last 50 lines
tm-watcher logs

# Show last 100 lines
tm-watcher logs -n 100

# Follow in real time
tm-watcher logs --follow
```

### Health check

Check Time Machine, configuration, database, daemon status, and LaunchAgent:

```bash
tm-watcher doctor
```

## Configuration

The configuration file is located at `~/.config/tm-watcher/config.toml` and is auto-generated on first run.

Default exclusion rules cover common development dependency directories such as `node_modules`, `target`, `vendor`, `.venv`, `venv`, `__pycache__`, `build`, `dist`, `.next`, `.nuxt`, and `.cache`.

Use `tm-watcher config` to view or update watched paths and exclusion rules:

```bash
# Show current configuration
tm-watcher config show

# Add a watched path
tm-watcher config add-path ~/Projects

# Add an exclusion rule
tm-watcher config add-rule ".pytest_cache"
```

After changing the configuration, run `tm-watcher daemon restart` to apply the changes.

## How it works

1. **Scan**: recursively traverses the specified directories looking for subdirectories matching the rules (e.g. `node_modules`, `target`)
2. **Exclude**: calls `tmutil addexclusion` to add directories to the Time Machine exclusion list
3. **Record**: writes to a local SQLite database at `~/.local/share/tm-watcher/exclusions.db`
4. **Clean**: the `clean` command checks whether recorded directories still exist, removes stale records, and corrects state drift

## Release status

- [x] Manual scan and exclusion (v0.1)
- [x] Stale record cleanup (v0.1)
- [x] Real-time filesystem monitoring (v0.2)
- [x] Background daemon (v0.2)
- [x] Logging and observability (v0.2)
- [x] GitHub Release assets for both macOS architectures (v0.2)
- [x] Homebrew formula generation and tap update workflow (v0.2)
- [x] Log viewing command (v0.3)
- [x] Configuration management commands (v0.3)
- [x] Health check and scan dry-run (v0.3)
- [ ] Apple Silicon real-device E2E and stable release acceptance (v0.3)

Current version: **v0.3.0**

## Contact

Questions, feedback, or just want to say hi? Find me here:

- [linux.do](https://linux.do/u/zzerd/summary)
- [V2EX](https://v2ex.com/member/zzerd)

## Documentation

- [Chinese README](./README.md) — Chinese version of this page
- [docs/CONTEXT.md](./docs/CONTEXT.md) — Domain language, core concepts, and long-term design conventions
- [docs/tm-exclude-watcher-prd.md](./docs/tm-exclude-watcher-prd.md) — Product goals, feature scope, roadmap, and testing strategy
- [skills/stacked-issue-pr-workflow/SKILL.md](./skills/stacked-issue-pr-workflow/SKILL.md) — Collaborative workflow for implementing GitHub issues as stacked pull requests

## License

MIT
