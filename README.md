# Memory Sentry for Spotify Desktop

[![Release](https://img.shields.io/github/v/release/yanlvl99/spicetify-memory-sentry?include_prereleases&sort=semver&label=release)](https://github.com/yanlvl99/spicetify-memory-sentry/releases) [![Version](https://img.shields.io/github/v/tag/yanlvl99/spicetify-memory-sentry?include_prereleases&sort=semver&label=version)](https://github.com/yanlvl99/spicetify-memory-sentry/tags) [![Downloads](https://img.shields.io/github/downloads/yanlvl99/spicetify-memory-sentry/total?label=all%20release%20downloads)](https://github.com/yanlvl99/spicetify-memory-sentry/releases) [![Validation](https://github.com/yanlvl99/spicetify-memory-sentry/actions/workflows/validate.yml/badge.svg?branch=main)](https://github.com/yanlvl99/spicetify-memory-sentry/actions/workflows/validate.yml) [![License](https://img.shields.io/github/license/yanlvl99/spicetify-memory-sentry?label=license)](LICENSE) ![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-blue) ![Status](https://img.shields.io/badge/status-experimental-orange) [![Requires Spicetify](https://img.shields.io/badge/requires-Spicetify-1DB954)](docs/INSTALLATION.md)

**Windows 10 and Windows 11 desktop editions only. Requires Spotify Desktop and Spicetify CLI.** Memory Sentry is an experimental open-source project. Its automatic policy applies process working-set ceilings while Spotify is minimized, hidden, or in the background; Main is capped at 36 MB and Audio processes receive a fixed 36 MB ceiling. Turning off **Automatic policy** selects manual mode, which applies role defaults or saved role-wide limits in the background. A saved role limit follows matching process types when PIDs change between Spotify launches. Original limits remain backed up separately for each process instance. The separate dashboard power switch disables Memory Sentry and restores original Windows limits.

## Spicetify is required by the current release

The supported release is **not a standalone Spotify app**. It requires Spotify Desktop and Spicetify CLI installed for the same Windows user. The plugin runs as a Spicetify Custom App; a separate Memory Sentry server performs the Windows process-memory operations. A standalone interface could be added as a future adaptation, but a no-Spicetify installation mode is **not implemented or supported today**. For the supported setup, use the official installer and keep Spicetify installed.

## Preview: Task Manager with and without Memory Sentry

<table>
  <thead>
    <tr>
      <th width="50%">With Memory Sentry</th>
      <th width="50%">Without Memory Sentry</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td align="center"><img src="assets/COM-sharp.svg" alt="Task Manager capture with Memory Sentry enabled" width="100%" /></td>
      <td align="center"><img src="assets/SEM-sharp.svg" alt="Task Manager capture without Memory Sentry" width="100%" /></td>
    </tr>
  </tbody>
</table>

*These previews use the supplied `COM.png` and `SEM.png` captures with restrained SVG sharpening; the original PNG files remain unchanged. This is an illustrative comparison, not a controlled benchmark or a promise of the same RAM reduction on another PC.*

## Observed memory and restore behavior

Memory Sentry is designed to work across supported Windows 10/11 x64 desktop PCs and to **substantially reduce Spotify's resident working set while the window is minimized, hidden, or in the background**. The amount varies by PC, Spotify workload, process activity, and memory conditions; it is not a fixed RAM cap or a guarantee of identical numbers. In one measured setup, the combined Spotify working set fell to about **40 MB during the initial trim** and later settled near **100 MB**. These are observed resident-working-set figures, not total committed memory. Summed working sets can double-count shared pages.

When Spotify is restored, the intended behavior is to return to the previous interface state without deliberately reloading the page, while playback continues. On very low-end PCs, the window can sometimes appear black temporarily while the interface becomes visible again. **A temporary black screen does not by itself prove that the page reloaded**: if the previous page and state return as they were, the UI state was retained. Keyboard or mouse input can also take a few seconds to respond. Persistent black screens, crashes, playback interruption, or state loss should still be reported and tested. The Windows 10/11 compatibility and recovery test matrix remains open.

## What "memory reduction" means here

Memory Sentry changes **how much of each eligible Spotify process stays resident in physical RAM**. It does not free, decommit, or reduce that process's committed memory, so the process's commit charge normally stays the same even when the working-set figure drops.

Lowering the ceiling or trimming the working set removes pages from the process's working set, but Windows keeps them on its transition/standby lists in RAM. Clean pages, such as code, DLLs, and file-backed data, usually come back on demand through a soft page fault without reading from disk. Private pages that were modified can later be written to the pagefile if Windows needs their physical frames for something else. On a PC under real memory pressure, reusing those pages can require a hard page fault and disk I/O, which is why the interface or input may take longer to respond after restore.

This is not a one-shot "RAM optimizer": automatic mode manages Spotify's process tree while the window is minimized, hidden, or behind another window. Under automatic policy, Audio-role processes receive a fixed 36 MB ceiling and are excluded from adaptive changes. Manual mode applies role-specific defaults automatically after the configured background delay; a saved limit is shared by all processes with the same role and survives Spotify restarts and PID changes. The Audio role uses 36 MB by default. Original limits are captured separately for every process instance before its first change and restored when Spotify returns to the foreground. The dashboard power switch is the master on/off control: switching it off stops new limits and restores the original Windows limits. If restoration cannot complete immediately, the server remains off and retries pending restores. Task Manager's **Memory** column and the dashboard show working-set/resident memory, not committed memory; compare the **Commit size** column separately if you need to verify that committed memory was not reduced.

Windows 10 and Windows 11 may also keep some trimmed pages in memory compression, so the exact mix of standby pages, compressed pages, and pagefile-backed pages depends on the system and memory pressure.

> **1.0.0-rc.1 release candidate — still experimental.** On supported Windows 10/11 x64 desktop PCs, optimization is intended to reduce Spotify's resident memory while minimized, hidden, or in the background; audio processes use a fixed 36 MB ceiling. The amount and restore behavior vary by machine. Very low-end PCs may briefly show a black window during restore; this does not necessarily mean the page reloaded if it returns to the previous state. The full Windows 10/11 release test matrix remains open, so this is not a stable-release claim.

## The only supported installation method

1. Open the [official GitHub Releases page](https://github.com/yanlvl99/spicetify-memory-sentry/releases).
2. Download **Memory Sentry Setup** and its `.sha256` checksum from the same release.
3. Verify the checksum before running Setup. A checksum confirms file integrity, not the publisher's identity.
4. Run Setup as your normal Windows user, not as Administrator. It checks Windows and Spicetify before changing installed files; you do not have to close Spotify manually.
5. Setup installs the server, writes local settings, starts and health-checks the local API, then applies the plugin through Spicetify. Spicetify may restart Spotify automatically.
6. When Spotify returns, select Memory Sentry in the sidebar. The server is also registered to start at login for the current Windows user.

The Marketplace entry only **links to this GitHub repository** so people can view the project and its documentation. It cannot install or download Memory Sentry, and neither the plugin nor the server is distributed through Marketplace. The only supported installation method is the official Setup EXE from GitHub Releases. GitHub's automatically generated source archive is for inspecting the source and is not an installation package.

The Inno Setup installer is per-user and does not require a machine-wide MSI installation. The executable is not code-signed, so Windows SmartScreen or antivirus may still show a reputation warning. Do not bypass security warnings blindly.

## What the adaptive policy does

The current policy keeps an individual working-set ceiling for each eligible Spotify process:

- Automatic ceilings: Main is capped at 36 MB and can adapt only below that ceiling; Audio is fixed at 36 MB. GPU/Renderer start at 28 MB; Utility/Helper at 24 MB; Crashpad at 16 MB.
- For roles that can grow, the process must use its full current working-set ceiling while also using at least 10% of one logical processor for six consecutive seconds before the ceiling can increase by 8 MB, subject to its role maximum. High CPU alone never requests more memory.
- If CPU remains at or below 3% and the working set stays at least 6 MB below its ceiling for ten seconds, the ceiling can decrease by 4 MB.
- Changes have an eight-second cooldown. The global adaptive range is 12–64 MB; Main has a 24 MB floor and a 36 MB maximum, while background roles have lower floors.
- Processes holding an active Windows audio session are identified through the Core Audio session API (the main process hosts audio in current Spotify builds). Audio receives the fixed 36 MB ceiling and is excluded from adaptive increases/decreases. Automatic optimization starts after the configured background delay; original Windows limits are restored when Spotify returns to the foreground.
- Turn off **Automatic policy** to enter manual mode. The same configured background delay applies in both modes. While Spotify is minimized, hidden, or in the background, role defaults are applied automatically: Main/Audio 36 MB, GPU/Renderer 28 MB, Utility/Helper 24 MB, Crashpad 16 MB. Set a whole-number override from 8 to 512 MB for an individual process; overrides replace only that process's role default. Original limits are restored when Spotify returns to the foreground.
- The dashboard power switch disables both modes, stops new limits and restores original Windows limits. If a restore cannot complete immediately, the server retries it while remaining disabled.
- Applying a value below that process's default requires a five-second warning countdown and confirmation; values below the default can cause glitches, slowdowns, or process failures. Limits are keyed to both PID and process creation identity, so a recycled PID never inherits an old manual value.

CPU is estimated from process-time deltas and expressed as a percentage of one logical processor, capped at 100% per process. An increase requires both sustained high CPU and the working set to reach the process's current ceiling throughout the same interval; high CPU below that ceiling does not trigger an increase. Working-set MB is resident RAM, not private committed memory, and summed working sets can double-count shared pages.

## Requirements

- Windows 10 or Windows 11 desktop edition, x64.
- Spotify Desktop and Spicetify CLI installed for the same Windows user.
- The official setup program contains the Memory Sentry server. A global Node.js installation is not required for new installations.
- When migrating a legacy Node.js installation with an outstanding recovery file, setup may request Node.js 22+ temporarily to run the old server's restore command. If legacy recovery cannot be verified, setup stops and preserves the backup instead of deleting it.

macOS, Linux, Windows 7/8/8.1, Windows Server and ARM64 Windows are not supported by this release.

## Installed components and safety

- Custom App: %APPDATA%\spicetify\CustomApps\SpicetifyMemorySentry
- Server, settings and recovery state: %LOCALAPPDATA%\SpicetifyMemorySentry
- User startup registration: HKCU\Software\Microsoft\Windows\CurrentVersion\Run
- Logs: %LOCALAPPDATA%\MemorySentry\logs after uninstall

The server runs as a separate per-user process under the current Windows user, exposes its dashboard API only on 127.0.0.1, requires an internal per-install token, validates the Spotify HTTPS origin and loopback Host header, and does not upload telemetry. The installer launches it automatically and verifies the local status API. Early startup failures are recorded in %LOCALAPPDATA%\\SpicetifyMemorySentry\\logs\\startup.log. Dashboard failures now include the underlying local API error, and uninstall attempts safe recovery without requiring Spotify to be closed. It checks the process creation timestamp before changing or restoring limits, persists recovery data before applying a cap, and stops upgrade/uninstall if a limit cannot be safely restored.

## Upgrade and uninstall

Setup and uninstall do not require you to close Spotify manually; Spicetify apply manages its own refresh/restart. Setup attempts to restore the old server's limits before replacing it and preserves recovery data if restoration cannot be confirmed. Windows Settings → Apps → Installed apps → Memory Sentry → Uninstall runs a pre-uninstall recovery check. If recovery fails or recovery records exist while the executable is missing, uninstallation stops and preserves data. Do not manually delete recovery files to bypass an error.

## Build from source

Install the stable Rust toolchain and Inno Setup 6. From the repository root, run cargo test with manifest path native/rust/Cargo.toml and --locked, then cargo build --release for target x86_64-pc-windows-msvc. A complete setup also needs the plugin and docs staged as described in installer/inno/MemorySentry.iss. GitHub Actions builds the single supported Setup EXE and its SHA-256 checksum. GitHub's automatically generated source archive is for source review, not an alternate installer. CI compilation is not the same as an actual Spotify/audio/restore test on a Windows 10/11 desktop.

## Open source, license and antivirus notices

Source and release packaging are public under GPL-3.0. The server emits no remote telemetry and does not read playlists, track names, listening history or Spotify credentials. The installer does not disable Defender, SmartScreen, the firewall or execution-policy controls.

**No unsigned release can guarantee zero SmartScreen or antivirus warnings.** A new Rust executable can still trigger warnings due to publisher/file reputation. Do not bypass security warnings blindly. Verify the download source and SHA-256, inspect the open source, and report suspected false positives with the specific file hash.

See [PRIVACY.md](PRIVACY.md), [SECURITY.md](SECURITY.md), [docs/INSTALLATION.md](docs/INSTALLATION.md), [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) and [docs/RUST-MIGRATION.md](docs/RUST-MIGRATION.md).
