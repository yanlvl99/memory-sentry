# Marketplace publishing and support model

Memory Sentry is an independent community project for Windows 10/11 x64 desktop editions. The current release requires [Spotify Desktop](https://open.spotify.com/download) and [Spicetify CLI](https://spicetify.app/) for the same Windows user. The Spicetify Custom App uses a separate server for Windows memory operations. The project remains experimental; Marketplace discoverability is not an endorsement or stability guarantee.

## Distribution

The official per-user Setup EXE and its SHA-256 checksum are published on the [GitHub Releases page](https://github.com/yanlvl99/memory-sentry/releases). The Setup package contains the server, Custom App, and installation/removal flows.

The Marketplace listing is discovery-only: it links to this repository and does not host or install Memory Sentry. Users must download the official Setup EXE from GitHub Releases; this repository does not publish a separate portable ZIP.

Memory Sentry manages the **resident working set**, not committed memory. It does not free or decommit memory, and observed results vary by workload and system. Avoid describing the result as freed, released, or swapped-out memory.

The server API binds to loopback, requires a per-install token, validates the Spotify HTTPS Origin and loopback Host header, and records original process limits for recovery. Automatic mode gives Audio-role processes a fixed 36 MB ceiling without adaptive adjustments; manual mode uses 36 MB as the default and supports persistent role-wide overrides. Original limits are recorded per process instance. Logs and metrics stay on the PC.

## Listing requirements

The repository is public and currently has the `spicetify-apps` topic. Keep that topic set and maintain a valid root `manifest.json` with a preview image and README. The root manifest supplies Marketplace metadata; `app/manifest.json` is the runtime manifest used by Spicetify.

Before submitting or updating the listing:

1. Verify that the repository topic remains `spicetify-apps`.
2. Keep the root manifest preview set to `assets/marketplace-cover.svg` and the README banner at `assets/readme-banner.svg`.
3. State the Windows 10/11 x64 requirement and separate server requirement in the listing description.
4. Publish only the official Setup EXE and its SHA-256 checksum; do not add a separate Marketplace build or portable installer.
5. Validate installation, missing-server behavior, upgrade, recovery, and uninstall on Windows 10 and Windows 11 desktop systems.

The topic makes the repository eligible for Marketplace discovery; it does not mean the Marketplace has indexed or approved the listing.

Official guide: https://github.com/spicetify/marketplace/wiki/Publishing-to-Marketplace
