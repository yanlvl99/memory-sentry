# Changelog

## Unreleased

- The Audio process is no longer pinned to a fixed 36 MB ceiling: it now adapts between 36 MB and 60 MB like the other roles, raising under sustained load at the ceiling and falling back when quiet, and its state badge reads "Adaptive" once a limit is applied.
- Make the low-limit warning apply only through the "Apply lower limit" button: closing it by clicking outside or pressing Escape now restores the previous value instead of applying, and the warning no longer reappears on its own.
- Add an "I understand this can cause problems; do not show this warning again" checkbox to the low-limit warning that, once accepted, remembers the choice locally and applies below-default limits directly without the modal.
- Show the background-delay selector's one-second option as "1 second" / "1 segundo" instead of the plural "1 seconds" / "1 segundos".
- Show the low-limit warning's countdown as "1 second" / "1 segundo" when one second remains (was the plural "1 seconds" / "1 segundos"), and replace the tail with "You can apply the limit now." / "Você já pode aplicar o limite." once the countdown reaches zero.

## 1.0.0-rc.3 — Settings-panel dashboard, configurable triggers and tray-aware detection

- Rename the `native/rust` source tree to `server/` (Cargo package `memory-sentry-server`) and use "server" as the product term throughout the code, installer and documentation, describing Rust only as the current implementation language.
- Rebuild the dashboard as a settings panel: a three-state memory mode (Automatic, Manual, Disabled) above the process list, per-process working-set bars with memory and ceiling on hover, and diagnostics folded into a single section.
- Let the user choose which window states trigger optimization (window behind another app, minimized, hidden) with independent checkboxes; the server keeps at least one state enabled and falls back to the previous all-background behavior for existing settings files.
- Detect the Spotify window state by enumerating real top-level windows in the server (`EnumWindows`, with `IsWindowVisible`, `IsIconic` and DWM cloaking) instead of the PowerShell `MainWindowHandle` probe, which cannot see a window hidden in the tray. Minimized-to-tray is now reported as `hidden` reliably on every pass, a leftover window of the same process can no longer keep the state stuck on `foreground`, and each transition logs the window counts behind the decision.
- Keep the background wait and any active limits while the window state cannot be read, so a momentary `window-not-found` during a tray hide no longer restarts the configured delay.
- Poll the local status every 1.5 seconds instead of 2.5 so the dashboard reflects applied limits sooner.
- Rename the process state badge from "Free" to "No ceiling" and move all state labels into the translated strings.
- Report each process's original Windows working-set ceiling in the status payload.
- Show the original ceiling in a process's memory tooltip when no Memory Sentry limit is active.

## 1.0.0-rc.2 — Adaptive limits and dashboard controls

- Apply role-specific manual defaults after the configured background delay in minimized, hidden, and background-visible states: Main/Audio 36 MB, GPU/Renderer 28 MB, Utility/Helper 24 MB and Crashpad 16 MB. Saved role-wide overrides survive Spotify restarts and PID changes; original limits remain captured per process instance and restored in the foreground.
- Require a five-second warning countdown and explicit confirmation before applying a manual limit below that process's default; validate manual values on both the dashboard and server.
- Preserve original-limit recovery for manual changes and restore active limits before policy transitions or when Memory Sentry is disabled.
- Keep automatic optimization active when Spotify is in the background; restore original limits only when it returns to the foreground.
- Cap Main at 36 MB automatically and apply a fixed 36 MB ceiling to Audio processes, whether Spotify hosts audio in Main or moves it to a separate process.
- Raise automatic ceilings only when high CPU and full use of the current working-set ceiling persist together for six seconds; high CPU alone no longer requests more memory.
- Switch the dashboard's debug mode control to the Spicetify Toggle component, keeping the previous button as a fallback when the component is unavailable.
- Validate Core Audio session discovery and the dashboard Toggle component in the package checks.
- Add a release mode setting so trimming can release memory only down to the current ceiling (default) or as much unused memory as possible, selectable from the dashboard.
- Add a `--swap <exe>` server command and `scripts/swap-server.ps1` so a new server build replaces the running one while Spotify and the plugin stay open.
- Add a debug mode toggle to the dashboard: the default view shows only the essential cards, processes and settings, while diagnostics (page faults, restore timing, adjustments, activity log and per-process detail) appear only when debug mode is enabled. The choice is remembered locally.
- Show distinct dashboard summary metrics for Spotify working-set memory and process-tree count; remove duplicate memory totals and foreground-only active-limit counts.
- Add a master Memory Sentry switch that stops new limits and retries pending restoration while disabled; use the selected background delay in both automatic and manual modes.
- Report the local service as `Server` instead of `Rust` in the status payload so the dashboard shows `Server 1.0.0-rc.2`.
- Streamline the README around project scope, requirements, policy thresholds, recovery controls, and validation limits; keep detailed workflows in dedicated guides.
- Correct documentation of Audio-role behavior, memory metrics, Marketplace topic status, and native companion command formatting.

## 1.0.0-rc.1 — Release candidate and installation consolidation

- Replace the former COM/SEM screenshot artwork with the square Marketplace cover, rectangular README preview, and README banner in `assets/`.
- Consolidate supported distribution to one per-user Setup EXE and its SHA-256 checksum; remove the custom portable installer ZIP and redundant PowerShell/batch installation entry points.
- Make Marketplace discovery-only in the README, dashboard and installation docs; the same official GitHub Setup is the only supported install method.
- Align Cargo, runtime and Setup versions on 1.0.0-rc.1 and mark it as a prerelease pending the real-desktop Windows test matrix.

## 0.3.4 — Installer diagnostics and startup ordering (experimental)

- Remove the incorrect requirement to close Spotify manually; Spicetify apply handles its refresh/restart during install and uninstall.
- Start and health-check the native local API before applying the Custom App, preventing the dashboard's initial render from racing helper startup.
- Give missing Spicetify a dedicated preflight error and show native setup diagnostics in the Inno Setup failure dialog.
- Log configure failures locally to setup.log, guard maintenance PID waits by checking the executable path, and remove stale helper.pid when the API cannot bind.
- Retain recovery-first behavior and the experimental prerelease label.

## 0.3.3 — Early-launch diagnostics (experimental)

- Create `logs/startup.log` before reading settings, so missing or invalid configuration is diagnosed instead of exiting without a log.
- Record launcher invocation, settings-load errors, monitor spawn failures, readiness timeouts, and successful API health checks locally.
- Include the startup-log path when launcher initialization fails; keep the release experimental pending real Windows desktop validation.

## 0.3.2 — Connection diagnostics and non-blocking uninstall (experimental)

- Render the actual dashboard API error instead of only the generic disconnected-state copy.
- Clear the Win32 last-error value before creating the single-instance mutex to avoid false duplicate-instance detection.
- Allow uninstall while Spotify is open by restoring/stopping the companion first, removing startup registration, and then applying the Spicetify removal.
- Wait up to five seconds for a gracefully stopped companion before preserving recovery data and blocking unsafe file removal.

## 0.3.1 — Companion startup diagnostics (experimental)

- Make the installer launch the native companion automatically instead of relying on an optional post-install checkbox.
- Verify the authenticated local status API before the launcher reports success.
- Capture detached monitor startup errors in a local startup log.
- Show specific HTTP/API failure details in the dashboard instead of hiding them behind a generic connection message.
- Keep the release experimental until real Windows 10/11 audio, recovery, and restore-latency tests are complete.

## 0.3.0 — Native Windows companion (experimental)

- Add a native Rust companion and per-user Inno Setup installer for Windows 10/11 x64.
- Add guarded install, upgrade, recovery and uninstall flows that preserve recovery data if limits cannot be restored safely.
- Build a Windows Setup EXE and portable x64 ZIP with SHA-256 checksums in GitHub Actions.
- Keep adaptive per-process working-set policy, audio-service exclusions, loopback-only dashboard API and no-telemetry behavior.
- Align native runtime and Cargo lockfile versions with 0.3.0, and derive validation package versions from Cargo metadata.
- Publish as an experimental prerelease: the complete real-desktop Windows 10/11 behavior and restore-latency test matrix remains outstanding.

## 0.2.0 — Adaptive policy preview

- Replaced the user-selected fixed per-process cap with a role-based adaptive policy.
- Raised a process ceiling by 8 MB after sustained CPU usage at or above 10% of one logical processor for six seconds.
- Lowered a ceiling by 4 MB after sustained CPU usage at or below 3% and at least 6 MB of working-set headroom for ten seconds.
- Added hysteresis, per-role bounds, and an adjustment cooldown to prevent rapid oscillation.
- Added local CPU estimates and per-process adaptive ceilings to the dashboard.
- Added automatic English/Portuguese (Brazil) UI localization based on the system/browser language.
- Added a GitHub Releases action when the Windows companion cannot be reached.
- Switched the project license to GPL-3.0 to require redistributions of covered derivative works to retain the copyleft terms.
- Kept a 24 MB minimum ceiling for the main process while allowing low-activity background renderers to shrink to 12 MB, balancing savings with window restoration.
- Added adaptive policy unit tests and release checksum workflow.
- Prevented new limits from being applied using stale process snapshots after discovery failures, and throttled retries for inaccessible targets.
- Clarified the unsigned-installer/SmartScreen limitations and the reason for the separate native companion.

## 0.1.4

- Reduced window-state polling to 100 ms and moved recurring process discovery off the synchronous focus path.
- Added process-creation identity checks, page-fault metrics, and restore-operation timing.
