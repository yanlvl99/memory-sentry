# Server migration and release gate

This document tracks the move from the legacy Node.js server to the current server, which is implemented in Rust, and the remaining release-validation work. For user-facing installation steps, see [INSTALLATION.md](INSTALLATION.md); for repeatable behavior measurements, see [BENCHMARKING.md](BENCHMARKING.md).

## Current release-candidate status

The server implements process discovery, Windows working-set operations, the loopback dashboard API, per-install authentication, recovery persistence, per-user startup, and guarded upgrade/uninstall commands. The current implementation is written in Rust and uses no third-party crates in its core. New installations do not require Node.js or Koffi; Node.js 22+ may be needed temporarily to recover limits saved by the legacy Node.js version.

Automatic limits apply to Spotify's process tree after the configured background delay. Audio-role processes receive a fixed 36 MB automatic ceiling and are not adaptively adjusted. In manual mode, 36 MB is the Audio default and can be overridden per process role. Role overrides persist across Spotify restarts and PID changes, while original limits are backed up per process instance. The same configured background delay applies in automatic and manual modes.

## Important safety model

1. Each working-set change verifies the target process creation timestamp before modifying or restoring limits.
2. Original working-set limits are written to recovery storage before an initial cap is applied.
3. Failed restoration retains recovery records and can block upgrade or uninstall.
4. The server API binds only to `127.0.0.1`, requires an internal token, and checks the Host header and Spotify HTTPS Origin.
5. The plugin does not download or execute the server.
6. Setup is per-user and does not request elevation or disable Windows security tools.
7. Logs remain local and are preserved outside the application directory on uninstall.

The master dashboard power switch stops new limits and retries pending restoration while disabled. This recovery design reduces the risk of losing recorded limits, but real Windows desktop testing is still required to validate playback continuity, foreground restoration, startup recovery, and uninstall behavior.

## Release milestones

- [x] Port the adaptive decision policy with unit tests.
- [x] Add Win32 working-set, window-state, process-identity, and token primitives.
- [x] Add the server runtime implementation, in Rust, matching the dashboard API.
- [x] Add setup/uninstall registration and legacy Node.js recovery.
- [x] Add CI policy tests and Windows x64 compile/package jobs.
- [x] Run the safe `--version` and `--self-test` commands on the Windows runner and build the validation Setup EXE.
- [ ] Test on a real Windows 10 desktop with Spotify and Spicetify installed.
- [ ] Test on a real Windows 11 desktop with Spotify and Spicetify installed.
- [ ] Measure playback continuity, working-set changes, restore-to-interaction latency, failed process operations, restart recovery, and uninstall.
- [ ] Verify the Marketplace discovery-only/missing-server screen and the official Setup EXE path.
- [ ] Verify the `spicetify-apps` topic remains set and submit the listing for public discovery.

## Installer format

The current installer is a per-user Inno Setup EXE. It supports recovery checks without requiring Windows Installer custom actions or administrator privileges. An MSI can be reconsidered after the registration and rollback flow is fully validated.

## Antivirus and signing

The current release is unsigned and may show SmartScreen or antivirus reputation warnings. Publish SHA-256 checksums, never instruct users to disable security tools, and evaluate code signing only after checking the program's current eligibility and trust model.
