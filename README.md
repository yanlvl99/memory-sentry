# Memory Sentry

[![Release](https://img.shields.io/github/v/release/yanlvl99/spicetify-memory-sentry?include_prereleases&sort=semver&label=release)](https://github.com/yanlvl99/spicetify-memory-sentry/releases)
[![Validation](https://github.com/yanlvl99/spicetify-memory-sentry/actions/workflows/validate.yml/badge.svg?branch=main)](https://github.com/yanlvl99/spicetify-memory-sentry/actions/workflows/validate.yml)
[![License](https://img.shields.io/github/license/yanlvl99/spicetify-memory-sentry?label=license)](LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-blue)
![Status](https://img.shields.io/badge/status-experimental-orange)

![Memory Sentry](assets/readme-banner.svg)

Memory Sentry is an open-source utility for managing the resident working set of Spotify Desktop processes while Spotify is running in the background.

It integrates with Spicetify as a Custom App and uses a separate native Rust companion to manage process memory on Windows.

**Status:** Experimental release candidate. Windows 10/11 x64 desktop editions are supported; broader compatibility testing remains in progress.

Memory Sentry is an independent community project, not an official Spotify or Spicetify product.

## Features

- **Automatic policy:** Adjusts working-set ceilings based on process activity and memory usage.
- **Manual controls:** Configure working-set limits for individual processes.
- **Audio-aware policy:** Processes assigned to the Audio role use a fixed ceiling in automatic mode.
- **Process-aware recovery:** Records original working-set limits before applying changes and attempts to restore them when management is disabled or Spotify returns to the foreground.
- **Persistent configuration:** Supports saved limits across Spotify restarts and changing process IDs.
- **Local operation:** The companion API listens on loopback, uses a per-install access token, and does not send telemetry.

## How memory management works

Memory Sentry manages a process's **resident working set**: the amount of its memory currently resident in physical RAM.

It does not decommit memory or directly reduce the process's committed memory. The observed reduction varies with the Spotify workload, Windows memory management, and the system's available memory.

Lower working-set limits can increase page faults and make Spotify slower to respond when brought back into the foreground. Aggressive limits may cause glitches or process failures.

The dashboard provides controls to disable memory management. The companion also includes recovery handling for previously recorded process limits.

## Requirements

- Windows 10 or Windows 11 desktop edition, x64.
- Spotify Desktop.
- Spicetify CLI installed for the same Windows user.

macOS, Linux, Windows ARM64, and Windows Server are not supported by this release.

## Installation

1. Open the [GitHub Releases page](https://github.com/yanlvl99/spicetify-memory-sentry/releases).
2. Download **Memory Sentry Setup** and the SHA-256 checksum published with the same release.
3. Verify the checksum before running the installer.
4. Run Setup as your normal Windows user, without administrator privileges.
5. Once installation completes, open Spotify and select **Memory Sentry** from the sidebar.

Setup installs the companion and registers the Spicetify Custom App. Spicetify may refresh or restart Spotify during installation.

**Marketplace listing:** The Marketplace entry provides project discovery and links to this repository. It does not distribute or install the companion. The official Setup executable from GitHub Releases is the supported installation method.

A checksum verifies file integrity, not publisher identity. The current release is unsigned, so Windows security or reputation warnings may still appear. Do not bypass such warnings blindly.

## Testing and project status

The CI workflow checks package metadata and JavaScript syntax, runs Rust policy unit tests on Linux and Windows, and builds the Windows x64 companion and setup package.

These checks validate specific parts of the project; they do not constitute a complete end-to-end test of Spotify playback, foreground restoration, or compatibility across Windows systems.

The broader desktop test matrix remains incomplete. Results may vary by hardware, Spotify workload, and Windows configuration. The project should therefore be treated as experimental rather than as a stable, universally validated memory-management solution.

## Development

Install Node.js 22+ and a stable Rust toolchain to run the repository's package checks and Rust tests:

    node tests/validate-package.js
    cargo test --manifest-path native/rust/Cargo.toml --locked

For Windows companion builds, installation packaging, and development details, see the [Development Guide](docs/DEVELOPMENT.md).

## Security and privacy

Memory Sentry is designed to operate locally. Its companion API binds to `127.0.0.1` and requires an internal per-install token. Settings, recovery data, and diagnostic logs are stored on the computer.

Upgrade and uninstall procedures include recovery checks. When restoration cannot be confirmed, the installer or uninstaller may stop and preserve recovery data instead of proceeding.

See [SECURITY.md](SECURITY.md) and [PRIVACY.md](PRIVACY.md) for implementation details and limitations.

## Documentation

- [Installation Guide](docs/INSTALLATION.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Benchmarking](docs/BENCHMARKING.md)
- [Development Guide](docs/DEVELOPMENT.md)
- [Security Policy](SECURITY.md)
- [Privacy](PRIVACY.md)
- [Changelog](CHANGELOG.md)

## License

Memory Sentry is licensed under [GPL-3.0](LICENSE).
