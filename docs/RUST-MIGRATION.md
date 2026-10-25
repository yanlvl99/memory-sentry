# Server migration and release gate

## Current preview status

The Rust source now contains an adaptive policy, Win32 process discovery and working-set calls, a loopback-only dashboard API, an internal per-install token, Windows 10/11 validation, legacy recovery hooks, per-user startup, and a recovery-aware uninstall command. The intended behavior across supported Windows 10/11 x64 desktop PCs is substantial reduction of Spotify's resident working set while minimized or hidden. This is a residency change, not a decommit: the process's commit charge normally stays the same, trimmed pages can remain cached in RAM, and dirty private pages may be written to the pagefile later under memory pressure. The exact reduction varies by PC and workload. On very low-end hardware, the window can briefly appear black during restore without that alone proving the page reloaded; test whether the previous UI page and state return, and record any input delay. The full cross-device end-to-end test matrix remains required.

The server is the Memory Sentry backend for the preview installer. It uses no third-party Rust crates in the current core, so there is no global Node.js/Koffi requirement for new installs. Legacy Node recovery is retained to help previous users restore old limits safely.

## Important safety model

1. The dedicated Spotify audio-service process is excluded from working-set changes.
2. Every memory mutation verifies the target process creation timestamp before changing or restoring limits.
3. Original working-set limits are written to recovery storage before an initial cap is applied.
4. Recovery records survive failed restoration and block upgrade/uninstall when the running server or recovery state cannot be verified.
5. The server API binds only to 127.0.0.1, requires an internal token and checks both the Host header and Spotify HTTPS Origin.
6. The plugin itself does not download or execute the server.
7. Setup is per-user and does not request elevation or turn off Windows security tools.
8. Logs stay local and are preserved outside the install directory on uninstall.

## Release milestones

- [x] Port the adaptive decision policy with unit tests.
- [x] Add Win32 working-set, window-state, process-identity and token primitives.
- [x] Add a Rust runtime implementation matching the dashboard API.
- [x] Add setup/uninstall registration and legacy Node recovery workflow.
- [x] Add CI policy tests and a Windows x64 compile/build job.
- [x] Verify Inno Setup compiles the per-user setup executable in CI.
- [x] Run the native companion's safe `--version` and `--self-test` checks on the Windows runner and build the validation Setup EXE.
- [ ] Test on a real Windows 10 desktop with Spotify and Spicetify installed.
- [ ] Test on a real Windows 11 desktop with Spotify and Spicetify installed.
- [ ] Measure audio continuity, RAM reduction, UI restore latency, failed process operations, restart recovery and uninstall.
- [ ] Verify the Marketplace discovery-only/missing-companion screen and the single official Setup EXE path.
- [ ] Add the spicetify-apps topic and publish a release before submitting the listing for public discovery.

## Why the MSI is not the first installer

The current setup uses Inno Setup as a per-user EXE installer. It supports custom pre-install and pre-uninstall recovery checks without requiring Windows Installer custom actions or administrator privileges. An MSI can be considered later, after the per-user registration and rollback flow is validated; making an MSI before that would add packaging complexity without improving the memory-control engine.

## Antivirus and signing

Rust does not automatically prevent SmartScreen or antivirus warnings. The preview is unsigned and can receive reputation-based warnings. Keep source and build workflows public, publish SHA-256 files, never disable antivirus, and investigate an open-source signing program only after checking its current eligibility and trust model.
