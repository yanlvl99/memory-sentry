# Marketplace publishing and support model

Memory Sentry supports Windows 10 and Windows 11 desktop editions only (x64 desktop) and currently requires Spotify Desktop plus Spicetify CLI for the same Windows user. The plugin is a Spicetify Custom App; the Memory Sentry server performs the Windows memory-management work. A standalone no-Spicetify mode is a possible future adaptation, not a feature of this release. The project remains experimental; Marketplace discoverability must not be represented as a stable-product endorsement.

## Installation and distribution model

### Official Windows setup installer

The official per-user setup program and SHA-256 checksums are published on:

https://github.com/yanlvl99/spicetify-memory-sentry/releases

A release contains the compiled Rust companion, the Spicetify Custom App, installer/uninstaller flows, GPL-3.0 license, privacy/security documentation, and checksums. Never label a release stable until clean Windows 10 and Windows 11 desktop tests cover install, upgrade, audio continuity, working-set restoration, and uninstall.

### Marketplace listing (discovery only)

Marketplace is used to discover the project and view its README/preview. It is not a second installation method and does not provide a separate Marketplace-specific build. Users must deliberately download and run the same official Setup EXE from GitHub Releases to install the native Windows companion. This repository does not publish a custom portable ZIP.

The separate Rust process exposes its API only on loopback, uses a random per-install token, validates Spotify's HTTPS Origin and loopback Host header, writes recovery data before changing process limits, excludes audio-service processes, and refuses unsafe removal if recovery cannot be confirmed. Logs and metrics remain on the PC.

## Listing readiness

The official Marketplace publisher guide requires a public repository, the exact repository topic `spicetify-apps`, and a valid root `manifest.json` with a preview image and README. This repository keeps that root metadata separate from `app/manifest.json`, which is the runtime manifest used by Spicetify.

Before the Marketplace index can discover this repository:

1. Add the repository topic `spicetify-apps` in GitHub's repository About/Topics settings.
2. Keep the root `manifest.json` preview set to `assets/marketplace-preview-sharp.svg`. The sharpened SVG previews apply a restrained edge filter to the exact `COM.png` and `SEM.png` captures; the original PNGs remain unchanged. `assets/marketplace-banner.svg` uses the same captures and filter, without adding or inventing metrics.
3. Keep the separate native companion requirement and Windows 10/11 limitation explicit in the listing description.
4. Publish one official Setup EXE and its SHA-256 checksum; do not publish a separate Marketplace build or custom portable installer.
5. Test the missing-helper dashboard, the one supported Setup flow, upgrade, and uninstall on Windows 10 and Windows 11.

Adding the topic makes the repository eligible for Marketplace discovery; it does not mean Marketplace has already indexed or approved the app. The custom app entry does not replace the standalone native companion installer.

Official publishing guide: https://github.com/spicetify/marketplace/wiki/Publishing-to-Marketplace
