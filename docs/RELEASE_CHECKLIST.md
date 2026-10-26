# Release checklist

Do not call the release stable until all required real-desktop tests are complete.

## Build and package

- [ ] cargo test for native/rust/Cargo.toml passes.
- [ ] Windows x64 build succeeds for x86_64-pc-windows-msvc.
- [ ] Inno Setup compiles the per-user setup executable in CI.
- [ ] Only the official Setup EXE and its SHA-256 checksum are uploaded and independently verified.
- [ ] Setup version, README and changelog agree.
- [ ] Marketplace manifest and preview exist, root README is valid and repository topic spicetify-apps is set.
- [ ] No Node.js requirement remains for new Rust installs. Any remaining Node dependency is clearly documented as legacy recovery only.

## Required Windows 10 and Windows 11 desktop validation

- [ ] Install on a clean user account with Spotify Desktop and Spicetify already installed.
- [ ] Verify setup rejects unsupported OS editions before modifying files.
- [ ] Confirm setup is per-user, does not ask for admin and does not alter antivirus/SmartScreen/firewall settings.
- [ ] Confirm original process limits are durably saved before each initial cap.
- [ ] Confirm audio-role processes receive a fixed 36 MB automatic ceiling and are excluded from adaptive adjustments.
- [ ] Confirm high CPU alone does not raise a ceiling while the working set is below it; sustained high CPU at the full current ceiling does raise it, and idle/headroom lowers it without oscillation.
- [ ] Confirm the selected background delay applies in both automatic and manual modes.
- [ ] Confirm limits are restored on foreground return, master-switch disable, shutdown, restart recovery and uninstall; pending restores remain recorded and are retried.
- [ ] Confirm an unresponsive recovery path leaves backups and blocks file removal.
- [ ] Test legacy Node migration with and without Node.js 22+ and with successful and failed recovery.
- [ ] Run at least five restore-to-interaction samples on representative low-, mid-, and higher-spec supported PCs; compare medians and record playback continuity, working-set reduction while minimized, UI reload/state retention, any temporary black screen on restore, and time until keyboard/mouse input is accepted.
- [ ] Verify the five-second confirmation countdown for manual limits below the role default.
- [ ] Verify English/Portuguese UI language selection.
- [ ] Verify no network telemetry is emitted by the server.
- [ ] Verify uninstall removes only the Memory Sentry plugin and install directory while preserving logs outside the app folder.

## Public release

- [ ] CI passes on the exact release commit.
- [ ] Setup EXE and SHA-256 files are attached to the correct release.
- [ ] Mark v1.0.0-rc.1 as a prerelease; create stable v1.0.0 only after every required Windows 10/11 real-desktop test passes.
- [ ] Never state the release has no antivirus warnings; the current executable is unsigned.
- [ ] Add the exact GitHub repository topic `spicetify-apps` manually, then submit the discovery listing. The listing only links to the repository and cannot install Memory Sentry.
