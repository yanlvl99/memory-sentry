# Memory Sentry

[![Release](https://img.shields.io/github/v/release/yanlvl99/memory-sentry?include_prereleases&sort=semver&label=release)](https://github.com/yanlvl99/memory-sentry/releases)
[![Validation](https://github.com/yanlvl99/memory-sentry/actions/workflows/validate.yml/badge.svg?branch=main)](https://github.com/yanlvl99/memory-sentry/actions/workflows/validate.yml)
[![License](https://img.shields.io/github/license/yanlvl99/memory-sentry?label=license)](LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64%20%7C%20ARM64-blue)
![Status](https://img.shields.io/badge/status-experimental-orange)

![Memory Sentry](assets/readme-banner.png)

Memory Sentry is an open-source utility for managing the resident working set of Spotify Desktop processes while Spotify runs in the background.

**Current release:** `1.0.0-rc.6` - experimental release candidate, not a stable release.

The current release requires Spotify Desktop and Spicetify CLI for the same Windows user. The Spicetify Custom App communicates with the Memory Sentry server, which runs as a **separate per-user process**, is currently implemented in Rust, and performs the Windows memory operations. Memory Sentry is an independent community project, not an official Spotify or Spicetify product.

## Features

- Adaptive working-set limits with optional per-process fixed ceilings for Spotify processes while Spotify is outside the foreground.
- Choose which window states trigger optimization: window behind another app, minimized, hidden, or any combination of the three.
- Per-identity limits with persistent pins that survive process restarts (identity is role + command line, never the PID).
- A two-state memory mode (Custom, Disabled): Custom runs the adaptive engine with per-process pins available; Disabled stops new limits and requests restoration of recorded Windows limits.
- Local-only dashboard and diagnostics; the server does not upload telemetry.

## Policy and controls

Memory Sentry limits the **resident working set**—the amount of process memory currently resident in physical RAM. It does not decommit memory or directly reduce committed memory, and the observed reduction varies by PC and workload.

In Custom mode the engine sizes each process from its own recent demand: it keeps a decaying histogram of that process's working set and targets a high percentile of it plus a small margin, instead of stepping a fixed number of megabytes on a timer. The memory pressure reported by Windows widens the margin when memory is plentiful and removes it when memory is low, so the same workload is treated more gently on an idle machine than on a short one. A change only happens when the target is meaningfully above or below the current ceiling (a dead band), at most once every eight seconds. A capped process that faults heavily for each resident megabyte is treated as thrashing, and a process that keeps burning CPU while its resident set is a small share of its ceiling is treated as starved; both receive a one-step relief increase. That relief ceiling becomes the identity's learned safe floor, so a later quiet window never returns to it. Every role starts at the same **38 MB** ceiling with a **4 MB** absolute floor; Main does not grow above 38 MB, Audio adapts upward to 60 MB, Audio never descends below 25 MB, and a renderer keeps 38 MB while the window is visible in the background. Machines with more than 16 GB of RAM get extra headroom on those maxima. The engine also learns each process identity's typical busy level on-device, which only widens or tightens the same target; this profile is stored locally and never uploaded.

Pinning a process to Manual holds a fixed ceiling that outranks the adaptive engine while the rest of its role keeps adapting. The starting defaults are Main/Audio 36 MB, GPU/Renderer 28 MB, Utility/Helper 24 MB, and Crashpad 16 MB. A manual limit below the process role's default requires a **five-second warning countdown** and explicit confirmation.

The **same configured delay applies whenever Custom is enabled**, and it only starts once Spotify enters a window state you enabled under **Optimize when**. When Spotify returns to the foreground, or enters a state you did not enable, original limits are restored. The **Disabled** mode is the master on/off control: it stops new limits and requests restoration of original limits. If restoration cannot be confirmed immediately, pending recovery data is retained for retry.

## Requirements

- Windows 10 or Windows 11 desktop edition, x64 or ARM64.
- [Spotify Desktop](https://open.spotify.com/download) and [Spicetify CLI](https://spicetify.app/) installed for the same Windows user.

Windows Server, macOS, and Linux are not supported by this release. Native installers are published for both Windows x64 and ARM64.

## Installation

Download the **Memory Sentry** per-user MSI that matches your Windows CPU (`MemorySentryForSpicetify-<version>-x64.msi` or `-arm64.msi`) and its matching SHA-256 checksum from the [official GitHub Releases page](https://github.com/yanlvl99/memory-sentry/releases), verify the checksum, and follow the [Installation Guide](docs/INSTALLATION.md). The official per-user MSI is the only supported installation method. The Marketplace entry only links to the project repository; it does not install or distribute the server.

No unsigned release can guarantee zero SmartScreen warnings. This release is unsigned, and a checksum verifies file integrity—not publisher identity. Do not disable security tools or blindly bypass warnings.

## Testing and project status

CI validates package metadata and JavaScript syntax, runs platform-independent Rust policy tests on Linux and Windows, and builds the Windows x64 and ARM64 servers and the per-user MSI installers. These checks do not constitute an end-to-end test of Spotify playback, foreground restoration, or compatibility across desktop systems.

The broader Windows 10/11 desktop test matrix remains incomplete. Results may vary by hardware, Spotify workload, and Windows configuration, so the project remains experimental.

## Development

Install Node.js 22+ and a stable Rust toolchain to run package checks and policy tests:

    node tests/validate-package.js
    cargo test --manifest-path server/Cargo.toml --locked

For Windows server builds and packaging, see the [Development Guide](docs/DEVELOPMENT.md). For repeatable performance measurements, see [Benchmarking](docs/BENCHMARKING.md).

## Security and privacy

The server API binds to `127.0.0.1` and requires an internal per-install token. Settings, recovery data, and diagnostic logs are stored locally. Upgrade and uninstall procedures include recovery checks and may stop rather than remove files when restoration cannot be confirmed.

See [SECURITY.md](SECURITY.md) and [PRIVACY.md](PRIVACY.md) for details and limitations.

## Documentation

- [Installation Guide](docs/INSTALLATION.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Development Guide](docs/DEVELOPMENT.md)
- [Release checklist](docs/RELEASE_CHECKLIST.md)
- [Server migration and release gate](docs/SERVER-MIGRATION.md)
- [Changelog](CHANGELOG.md)

## License

Memory Sentry is licensed under [GPL-3.0](LICENSE).
