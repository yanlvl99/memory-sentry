# AGENTS.md

Stable project knowledge for anyone (human or agent) working on Memory Sentry. This file records
what does not change often. Do not use it for temporary state: current release-candidate numbers,
open issues, in-progress checklists, or benchmark readings belong elsewhere.

## What Memory Sentry is

An open-source utility that reduces Spotify Desktop's resident RAM while it runs in the background,
by applying adaptive working-set limits to individual Spotify processes. It manages the **resident
working set** only; it does not decommit memory or reduce committed memory. The project is
experimental.

## Architecture

Two parts, never conflated:

- **Spicetify Custom App** (`app/`): the JavaScript dashboard rendered inside Spotify. Modular
  PascalCase components under `app/ui/` (Core, I18n, Api, Adapters, Widgets, TopBar, Hero, RolePanel,
  Settings, Diagnostics, Footer, App). It talks to the server over a loopback HTTP API.
- **Memory Sentry server** (`server/`): a separate per-user Rust process that performs the Windows
  memory operations. It is a Cargo workspace:
  - `crates/Core` (`memory-sentry-core`): platform-independent adaptive policy and unit tests. No
    Win32 side effects; runs on any OS.
  - `crates/Windows` (`memory-sentry-windows`): Win32 FFI (process snapshot, working-set calls,
    window state, memory pressure, audio sessions, arch check, single-instance lock). `#![cfg(windows)]`.
  - `crates/Spotify`: the `AppProfile` implementor holding all Spotify-specific knowledge (executable
    name, Chromium command-line role classification, sub-labels). The generic engine reaches the
    managed app only through the `AppProfile` trait, so supporting another app means adding one crate.
  - `src/`: the `memory-sentry-server` binary. `Runtime.rs` (facade) plus in-crate modules
    `Runtime/{Setup,Store,Api,Monitor}.rs`, and `main.rs` (CLI entry: configure, monitor, launch,
    swap, uninstall).

The installer (`installer/wix/Product.wxs`) is a per-user WiX v5 MSI that ships the server
(`MemorySentry.exe`) and the Custom App. `scripts/build-package.ps1` builds it.

## Supported scope

Spotify Desktop on Windows 10/11 desktop editions, x64 and ARM64, for a single Windows user who also
has Spicetify. Windows Server, Windows 7/8/8.1, macOS and Linux are not supported. Status is
experimental and subject to revalidation.

## Validation commands (verified against the repo)

From the repository root:

```
node tests/validate-package.js
cargo test --manifest-path server/Cargo.toml --locked
cargo build --release --manifest-path server/Cargo.toml --target x86_64-pc-windows-msvc --locked
```

Packaging: `scripts/build-package.ps1`. Safe, non-mutating binary checks: `--version`, `--self-test`.
`tests/validate-package.js` is a static package/source validator; it does not exercise Win32 behavior.

## Documentation responsibilities

Keep each document to its own purpose; do not duplicate the adaptive algorithm across files.

- `README.md` — landing page: what it is, trade-off, requirements, install pointer, doc index.
- `docs/INSTALLATION.md` — install, upgrade, repair, uninstall, data/log retention.
- `docs/TROUBLESHOOTING.md` — symptoms and safe recovery steps.
- `docs/BENCHMARKING.md` — how to measure the memory/latency trade-off.
- `docs/DEVELOPMENT.md` — contributor build/test/packaging and architecture map.
- `docs/MARKETPLACE.md` — Marketplace discovery-only listing vs Custom App vs server vs MSI.
- `docs/RELEASE_CHECKLIST.md` — release gates following the actual pipeline.
- `docs/SERVER-MIGRATION.md` — Node-to-Rust migration history and legacy recovery compatibility.
- `PRIVACY.md` / `SECURITY.md` — local data handling and the real security model.
- `CHANGELOG.md` — user-facing change history (Keep a Changelog; preserve past entries).

## Documentation conventions

- Project docs are written in English. Portuguese exists only as runtime UI localization in
  `app/ui/I18n.js`; do not add translated docs.
- Prefer relative links for in-repo files; verify every referenced file, image and asset exists.
- Write like a developer explaining their own software: direct, accurate, active voice, no marketing
  filler, no decorative emojis or badges, no invented quotes or numbers.
- Keep memory terminology precise: working set, private working set, commit size, page faults and
  pagefile I/O are distinct. Never describe a lower working set as memory freed, deleted or swapped.

## Source of truth and evidence

The current implementation, its tests and its configuration are the source of truth — not older docs,
previous changelog entries, or assumptions. Before stating a technical claim, trace it to the actual
execution path (a function existing does not mean it runs).

Never conflate these categories:

1. Implemented behavior.
2. Behavior actively used at runtime.
3. Behavior covered by automated tests.
4. Behavior verified on real Windows desktop systems.
5. Behavior officially supported for users.

Do not invent features, compatibility, performance results or security guarantees. CI (JavaScript
validation, Linux/Windows Rust policy tests, and Windows x64/ARM64 build + MSI packaging) proves the
package builds and the pure policy is correct; it is not an end-to-end Spotify desktop test. Real
Windows desktop validation of playback, restoration, upgrade and uninstall is still required before
calling a release stable.

## When behavior changes

Whenever user-visible behavior, installation, security or release behavior changes, update the
affected documentation in the same change. Do not blindly rewrite historical changelog entries to
match current behavior.
