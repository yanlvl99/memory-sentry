# Installation guide

**Supported systems: Windows 10 and Windows 11 desktop editions, x64 and ARM64.**

**Spicetify is required by the current release.** [Spotify Desktop](https://open.spotify.com/download) and [Spicetify CLI](https://spicetify.app/) must be installed for the same Windows user. The plugin is a Spicetify Custom App; the separate Memory Sentry server performs Windows process-memory management.

## The only supported installation method

1. Open the official release page: https://github.com/yanlvl99/memory-sentry/releases
2. Download the **Memory Sentry** MSI installer that matches your CPU (`-x64.msi` or `-arm64.msi`) and its `.sha256` checksum attached to that same release.
3. Verify the checksum. It confirms file integrity, not publisher identity.
4. Run the installer as the same Windows user that runs Spotify and Spicetify. Do not run as Administrator.
5. The installer stops any running Memory Sentry server silently, checks the supported Windows edition and detects Spicetify before replacing installed files. You do not have to close Spotify manually.
6. The installer lays down the server and Custom App, migrates an existing install's data, writes its local settings, starts and health-checks the local API, then applies the plugin through `spicetify apply`. Spicetify may restart Spotify automatically.
7. When Spotify returns, select Memory Sentry in the sidebar. The server is also registered to start at login for the current user.

The **Marketplace entry only links to the project repository**; it does not install or distribute Memory Sentry. The only supported installation method is the official per-user MSI from GitHub Releases. GitHub-generated source archives are for code review, not installation.

## Upgrade from the Node.js preview

The official per-user MSI is also the supported upgrade route. Before replacing a previous installation, it asks the old server to restore recorded limits and preserves recovery data if restoration cannot be confirmed. If a previous Node-based version has an outstanding `working-set-backup.json`, the installer stops only the matching legacy server and attempts its restore command. It may require Node.js 22+ temporarily for that migration. Never delete a recovery backup manually to force an upgrade.

## Uninstall

Use Windows Settings → Apps → Installed apps → Memory Sentry → Uninstall. The pre-uninstall command restores recorded working-set limits, removes the current-user startup entry and deregisters the plugin before the files are removed. Spotify does not need to be closed manually; `spicetify apply` may refresh/restart it. If recovery fails, uninstall stops and preserves program/recovery data.

Settings and logs remain at `%LOCALAPPDATA%\MemorySentryForSpicetify` after uninstall, so reinstalling keeps the current configuration.

## Security and unsupported systems

The installer is per-user and should not request administrator privileges. It does not disable antivirus, SmartScreen, the firewall or Windows security controls. Unsigned setup files may still show reputation warnings; verify the download source and SHA-256 instead of blindly bypassing warnings.

Unsupported: Windows Server, Windows 7/8/8.1, macOS, and Linux. ARM64 Windows is supported with the `arm64` installer.
