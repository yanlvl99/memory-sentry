# Marketplace publishing and support model

Memory Sentry supports Windows 10 and Windows 11 desktop editions only (x64 desktop) and currently requires Spotify Desktop plus Spicetify CLI for the same Windows user. The plugin is a Spicetify Custom App; the Memory Sentry server performs the Windows memory-management work. A standalone no-Spicetify mode is a possible future adaptation, not a feature of this release. The project remains experimental; Marketplace discoverability must not be represented as a stable-product endorsement.

## Installation and distribution model

### Official Windows setup installer

The official per-user setup program and SHA-256 checksums are published on:

https://github.com/yanlvl99/spicetify-memory-sentry/releases

A release contains the compiled server, the Spicetify plugin, installer/uninstaller flows, GPL-3.0 license, privacy/security documentation, and checksums. Never label a release stable until clean Windows 10 and Windows 11 desktop tests cover install, upgrade, audio continuity, working-set restoration, and uninstall.

### Marketplace listing (links only)

The Marketplace entry only displays the project and links to this GitHub repository. It cannot install or download Memory Sentry, and it does not host the plugin or the server. To install, users must open this repository, go to GitHub Releases, and run the official Setup EXE. This repository does not publish a custom portable ZIP.

The listing should not describe the result as released or swapped-out memory. Memory Sentry limits each eligible process's **resident working set**; it does not free or decommit that memory, and commit charge normally stays the same. Trimmed pages usually remain cached in RAM and can return through a soft page fault, while modified private pages may later be written to the pagefile under memory pressure.

The separate server process exposes its API only on loopback, uses an internal per-install token, validates Spotify's HTTPS Origin and loopback Host header, and writes recovery data before changing process limits. Automatic mode gives Audio-role processes a fixed 36 MB ceiling without adaptive adjustments; manual mode uses 36 MB as the default and supports persistent role-wide overrides that survive Spotify restarts and PID changes. Original limits are backed up for each process instance. The server refuses unsafe removal if recovery cannot be confirmed. Logs and metrics remain on the PC.

## Listing readiness

The official Marketplace publisher guide requires a public repository, the exact repository topic `spicetify-apps`, and a valid root `manifest.json` with a preview image and README. This repository keeps that root metadata separate from `app/manifest.json`, which is the runtime manifest used by Spicetify.

Before the Marketplace index can discover this repository:

1. Add the repository topic `spicetify-apps` in GitHub's repository About/Topics settings.
2. Keep the root `manifest.json` preview set to `assets/marketplace-preview-sharp.svg`. The sharpened SVG previews apply a restrained edge filter to the exact `COM.png` and `SEM.png` captures; the original PNGs remain unchanged. `assets/marketplace-banner.svg` uses the same captures and filter, without adding or inventing metrics.
3. Keep the separate server requirement and Windows 10/11 limitation explicit in the listing description.
4. Publish one official Setup EXE and its SHA-256 checksum; do not publish a separate Marketplace build or custom portable installer.
5. Test the missing-server dashboard, the one supported Setup flow, upgrade, and uninstall on Windows 10 and Windows 11.

Adding the topic makes the repository eligible for Marketplace discovery; it does not mean Marketplace has already indexed or approved the app. The Marketplace entry cannot install Memory Sentry and does not replace the official Setup EXE.

Official publishing guide: https://github.com/spicetify/marketplace/wiki/Publishing-to-Marketplace
