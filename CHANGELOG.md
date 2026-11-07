# Changelog

## Unreleased

- Test coverage: the hand-rolled epoch-to-ISO timestamp helper used for log lines and persistence records is now pinned by known-date tests (epoch zero, a leap day, a 64-bit future date and the 2038 boundary) (PR #102).

- The GitHub issue templates now point to the current data folder (`%LOCALAPPDATA%\MemorySentryForSpicetify\logs` instead of the legacy `SpicetifyMemorySentry` path) and ask for the current mode vocabulary (Custom/Disabled, per-process Automatic/Manual); the static validator expects the same phrasing (PR #101).

- Maintenance: the four "identity:value" settings parsers now share a single `collect_cap_map` helper with their range rules declared inline, replacing four hand-copied bodies that had to be edited together (PR #100).

- Maintenance: the ten `Settings { ... }` literals (test fixtures across Runtime.rs plus the fresh-install factory path in Store.rs) now derive from one shared `default_settings()` helper instead of ten hand-written copies that had drifted in indentation and sugar (PR #99).

- Maintenance: the dashboard stylesheet dropped two dead selectors left over from the debug redesign (`.mss-diag-details` and its child); nothing rendered them (PR #98).

- Maintenance: the dashboard's event-message translation layer dropped 22 dead regex mappings carried over from the previous server implementation. None of them can match a message the current server emits; the only live case kept was the `Settings updated: ...` prefix rewrite. No behavior change (PR #97).

- Maintenance: the dashboard now has a single config normalizer (`MSSNormalizeConfig`) shared by the initial state, the "server config" load path and the "save applied" response path, replacing three inline copies that had already drifted in defaults handling (PR #96).

- Maintenance: the Custom App dropped dead code - the unused auto and manual icon glyphs and their unused `MSS_ICON_PATHS` registry (nothing rendered them since the mode picker became Custom + Disabled), and `window.MSS` no longer carries a hardcoded `version: '1.0.0'` and `adapters` that nothing reads anywhere (PR #95).

- The installer's access-token substitution now rewrites only the token declaration in `app/index.js` instead of every occurrence of the placeholder in the file: the declaration gets the per-install token, the comment line stays neutral, and a stray literal outside the declaration fails closed instead of silently mangling the app (which is what blanked the dashboard in an earlier release) (PR #94).

- Maintenance: the Win32 process enumeration loop shared by discovery and the forced-stop path now lives in one place with byte-identical behaviour (no user-visible change) (PR #93).

- Test coverage: the hand-rolled base64 decoder used by the PowerShell discovery fallback is now pinned by known-vector tests (no behavior change) (PR #92).

- The server version string now derives from `server/Cargo.toml` at compile time, so a release bump only touches one place (PR #91).

- The `swap --upgrade` failure message now starts a new line for each unrestored process instead of printing a literal `\n` escape sequence (PR #90).

- The dashboard's memory mode picker now offers exactly two states: Custom and Disabled. The separate Automatic tile is gone; a dashboard previously set to Automatic reads as Custom from now on, since both states manage Spotify the same adaptive way the server already applies (PR #89).

- The release workflow's "Set release body" step now declares `shell: bash`, matching the bash conditional it already used; a tag push no longer fails at that step with a PowerShell parse error before the release is published (PR #86).

- Fixed the Memory Sentry dashboard inside Spotify rendering as an entirely blank panel after installing a release whose installer substitutes the per-install access token: the installer's token replace rewrote the new blank-screen guard condition too, turning it into an always-true comparison that made the app bail out before rendering anything. The guard now reads the token prefix instead of the placeholder literal, so the installer's replace cannot rewrite it; the unchanged placeholder check stays on the constant declaration where it belongs (PR #87).

## 1.0.0-rc.9 — Release version bump

- Contributor documentation now states the testing strategy as five layers, each proving only its own thing: Core policy tests, Runtime helper tests, static package validation, CI across x64/ARM64, and real-machine validation that stays a manual release gate (in docs/DEVELOPMENT.md, no new tooling) (PR #60).

- Opening a pull request from a same-repo branch no longer runs the whole validation suite twice: the `push` trigger now fires only for `develop` and `main`, so every PR gets exactly one CI run (PR #61).

- The installer now attempts to remove the inert leftover "Memory Sentry For Spicetify 1.0.0.0" row from Programs and Features — an empty registry entry left by pre-MSI builds whose uninstall string points at a product no longer registered on the machine. The row lives in the machine-wide registry, which a per-user install cannot write without elevation, so the attempt is best-effort and ignored on failure. On an elevated install it deletes the row; otherwise the documented recovery is a one-line elevated PowerShell command in docs/TROUBLESHOOTING.md, which also explains how to tell the real entry from the dead one (PRs #62, #63, #64).

- Every log file in the data folder now rotates at 512 KB like the runtime event log already did: the startup and setup logs can no longer grow without bound from daily launches, and the newest shifted log stays available beside the current file (PR #54).

- A hand-edited `settings.json` value that is not a whole boolean literal (such as `truex` or a truncated `true`) no longer parses as an accidental state: the parser rejects it and the documented default takes over (PR #55).

- The MSI package version now follows the version in `server/Cargo.toml`: the packaging script passes its numeric prefix (prerelease suffixes are dropped, because MSI versions are numeric only) to WiX, so Programs and Features reports the installed release instead of a fixed 1.0.0.0 (PR #50).

- Fixed Spotify occasionally keeping a reduced memory ceiling while you were actively using it: the dashboard now tells the local server the instant its window is focused and on screen, and a fresh report releases every limit even when the server's own window enumeration briefly fails to see the Spotify window (a tray hide/show or a focus hand-off to a modal). This release path only ever restores limits, so it can never cap a foreground Spotify (PR #49).
- A saved manual limit no longer distorts a Spotify you are actively using: the fixed ceiling takes effect only once the window is backgrounded again, matching the adaptive engine's background-only model (PR #48).
- The Rust test jobs now run the whole Cargo workspace, so the Core policy, Spotify and Windows crate suites are exercised as part of every CI check (PR #51).
- The local API layer has its first unit tests, covering the JSON escaping that guards server-built strings, the origin allowlist behind the loopback CORS gate, bearer-token parsing, the constant-time token comparison and the non-finite number guards (PR #53).
- Internal cleanup with no behavior change: the unused Adapters layer was deleted from the dashboard, the shared resident-memory/ceiling/identity/server-label/tag formulas collapsed into one definition each, and the manual-cap release path does its record lookup once (PRs #56, #57, #58).

- The Win32 command-line discovery now splits PEB offsets by target architecture: x64 keeps the verified values, ARM64 uses the documented Windows ARM64 layout (marked as needing verification on real ARM64 hardware) so native process discovery no longer silently degrades to the PowerShell fallback (PR #68).

- The server no longer leaves a process capped with no durable recovery record: if the backup persist fails after the working-set cap was already applied, the cap is reverted before reporting the failure (PR #69).

- The server no longer leaves `active_caps` inconsistent with on-disk settings after a failed config change: the in-memory caps are snapshotted before `restore_all` mutates them and rolled back if the settings write never happens (PR #70).

- The `MemoryPressure` enum variant order is now identical in the Core and Windows crates, so the explicit mapping in the monitor can never silently mis-map pressure levels (PR #71).

- The status API `pressureLevel` now reports the same memory pressure the adaptive engine acts on (read once per pass from Windows memory resource notifications), instead of computing a separate `GlobalMemoryStatusEx` ratio that could disagree (PR #72).

- The `--self-test` command now runs real adaptive policy checks (DemandHistogram percentiles, `faults_since` with 32-bit wraparound, `decide_adjustment` accept/reject, `memory_tier` classification) and exits non-zero on any failure, instead of printing a message without testing anything (PR #73).

- The MSI `ServerExe` component now uses a stable GUID instead of `Guid="*"`, so the component identity is stable across rebuilds and MajorUpgrade/servicing behaves predictably (PR #74).

- The release workflow body is now conditional: "Release candidate" text for prerelease versions, "Release" text for stable versions, instead of always saying "Release candidate" (PR #75).

- The release workflow no longer has a `paths` filter on its `push` trigger, so a tag `v*` always publishes regardless of which files the commit touched (PR #76).

- The MSI build script now wraps each architecture in try/catch, so x64 is still published even if arm64 compilation or WiX packaging fails (PR #77).

- The MSI now ships BENCHMARKING.md, MARKETPLACE.md, SERVER-MIGRATION.md, RELEASE_CHECKLIST.md and DEVELOPMENT.md under `docs\`, so all contributor docs are available after installation (PR #78).

- The dashboard now fails closed with a clear error if the installer did not substitute the access token (`__MSS_TOKEN__` placeholder), instead of silently returning 403 on every API call (PR #79).

- The dashboard `MSSRequest` now wraps `fetch` in try/catch and throws a clear `Network error contacting local server` message when the local server is unreachable, instead of propagating a raw fetch failure (PR #80).

- The dashboard `Core.js` now guards the `Spicetify` global with a `typeof` check, so the bundle loads outside the Spicetify host with a clear "App bundle is incomplete" message instead of an uncaught `ReferenceError` (PR #81).

- The automatic mode (`enabled=true, customMode=false`) now has a segment in the mode picker: `segAuto` appears between Auto and Custom, driven by `isAuto = config.enabled && !config.customMode`, and `setMode('auto')` configures `enabled=true, autoOptimize=true, customMode=false` (PR #83).

## 1.0.0-rc.7 — Demand-based adaptive policy

- The debug console now reports each process's page-fault rate next to its cumulative delta, so the exact thrash signal the adaptive engine acts on can be inspected. This is the value a real-machine calibration of the thrash threshold has to read.
- The confirmation messages for a fixed limit now name the process they act on instead of calling it a shared role limit, and the leftover strings for the removed foreground capping were dropped.
- The adaptive engine now starts every Spotify process at the same 38 MB ceiling and can descend to a 4 MB absolute floor instead of the previous per-role floors of 12-36 MB. Main no longer grows above 38 MB, while Audio keeps adapting upward to 60 MB. A process that thrashes at its ceiling receives an 8 MB relief increase and that ceiling becomes the identity's learned safe floor, so a later quiet window never descends back into it.
- The engine also watches CPU as a starvation signal: a process that sustains high CPU while holding almost no resident memory gets the same 8 MB relief increase, even if its page-fault rate stays under the thrash threshold. Audio keeps a 25 MB floor so playback does not glitch, and a renderer holds 38 MB while the window is visible in the background, descending toward the floor only once minimized or hidden.
- The adaptive policy now sizes each process from a decaying histogram of its recent working-set demand instead of fixed timers: the target is a high percentile plus a margin that widens when Windows reports plenty of free memory and disappears under low-memory pressure, changes need to pass a dead band, and the thrash guard compares faults per resident megabyte. The absolute step sizes remain only as a rate limit.
- The debug process table now shows each process's demand percentile and effective floor next to its ceiling, so the target the engine is aiming at can be read directly. The table was reorganized into a master-detail layout: the compact row keeps the glanceable columns and the wide forensic fields (identity, creation, pin, learned profile, faults, error) open in an expandable detail row, so long values no longer push the table outside the panel.
- The safety summary and the last-action notice now state the correct 38 MB starting ceiling that every role shares, instead of the old 36 MB figure, matching the adaptive engine and the documentation.
- The Marketplace cover is replaced with the new square artwork, and the unused vector preview and banner assets were removed; the README banner and the Marketplace cover are now the only shipped artwork.

## 1.0.0-rc.6 — Custom-only redesign and native process discovery

- Redesigned the dashboard around a single memory-composition bar with an expandable role panel: the mode model collapsed to Custom and Disabled, per-process limits are addressed and persisted by PID-free process identity (a pin survives restarts and PID changes), a tabbed debug console shows each identity's learned local usage profile, and a dedicated offline screen distinguishes a server that never started from a dropped connection. (PR #33)
- The adaptive engine is now background-only, more aggressive and safety-aware: a page-fault thrash guard blocks reclaim while a process is thrashing and grants relief when it is pinned at its cap, ceilings scale with installed RAM (machines above 16 GB get extra headroom), deeply idle processes step down toward their floor proactively, floors dropped as low as 12 MB, and each process identity's busy usage is learned on-device across sessions to personalize floors and margins. Nothing is uploaded.
- Process discovery no longer spawns a PowerShell and WMI query on every poll: the server enumerates Spotify processes with native Win32 snapshots and reads each command line from the process environment block, keeping PowerShell only as a fallback. Idle CPU use and transient memory pressure dropped substantially.
- Server and dashboard hot paths were optimized: the status payload is assembled into reused buffers, the local API accepts connections without a 20 ms poll loop, the dashboard skips re-rendering when polled data is unchanged, and the release build uses fat LTO with stripped symbols for a smaller server image.
- Product documentation now describes the Custom-only model with the real decrease step (6 MB) and the new engine behaviors, and dead translation strings were removed.
- Per-process CPU percentages are now computed against a shared timestamped baseline stamped at the instant the counters are captured, instead of each sampler's nominal window: previously the fast memory refresh and the slower process snapshot each advanced the other's CPU-time basis, diluting readings and biasing the adaptive engine toward treating busy processes as idle.
- Fixed the manual-limit textbox so typed values stay on screen: it is a controlled field, and the change handler only debounced the server write without storing the draft, so React snapped every edit back and existing pins could not be re-typed.
- Page faults per second are now computed against the exact age of their timestamped counter baseline too, so the fast memory refresh and the slower process snapshot no longer dilute each other's fault window. The thrash guard previously under-read fault pressure and could keep reclaiming memory from a process that was already faulting. (PR #35)

- Differentiate process rows by PID and show the memory value on the line: the role-name chip is replaced by the process PID so identical rows are distinguishable, and the memory MB value is now visible next to the role name instead of only in the tooltip. Small font sizes (10/11.5px) were bumped to 11/12px for readability. (PR #29, d62d5f6)
- The process memory tooltip now clamps to the viewport instead of the bar cell, so it no longer covers the column header or a neighbouring row, flips below the bar when there is no room above, and keeps its arrow pointing at the hovered position. (PR #32)
- The process memory bar is now easier to read and GPU-optimised: capped processes glide to their usage with a smooth transform-based width tween (no layout thrash) and shift colour as they approach the ceiling (blue to green to amber to red, softly pulsing when over), while an uncapped process fills proportionally to its size relative to the largest process (with a flowing shimmer) and is labelled "Limitless" ("Sem limite" in Portuguese); every effect is disabled when the user prefers reduced motion. The hover is now genuinely useful: it reports the percent of the limit and how many MB are left (or the share of the largest process when there is no limit), and in manual/custom mode it also invites you to click the bar to focus that process's limit field.
- Documented the repository's branching and release model: `main` is the protected stable branch holding only released code and `v*` tags, while `develop` is the protected integration branch that every feature, fix and documentation pull request targets, with CI (JavaScript validation, Rust policy tests and the Windows x64/ARM64 setup build) running before merge. A release is cut by promoting `develop` into `main` through a dedicated release pull request and tagging the resulting merge commit; direct merges into `main` are disallowed (PR #24, #30).

## 1.0.0-rc.5 — Automatic caps survive PID changes

- Automatic memory limits now survive Spotify process ID changes: the adaptive ceiling reached for each role is persisted by role (not by PID) and reapplied to new processes of the same role, so a PID rotation no longer drops the learned limit. Per-process backups are still kept for restore. (PR #26, a84b522, 1b08369)

## 1.0.0-rc.4 — WiX v5 MSI installer, Windows ARM64, split data folder, adaptive Audio ceiling

- The installer is now a single per-user WiX v5 MSI (`MemorySentryForSpicetify-<version>-<arch>.msi`); the Inno Setup build, its CI jobs and its release artifacts were removed, so the MSI is the only supported install, upgrade and uninstall path.
- Native Windows ARM64 is now supported: the packaging build emits both `MemorySentryForSpicetify-<version>-x64.msi` and `-arm64.msi`, and CI cross-compiles the `aarch64-pc-windows-msvc` target and publishes both installers.
- Setup now rejects a mismatched-architecture installer: it compares the server binary's built CPU architecture with the machine's native architecture and stops with a clear message if they differ, so the x64 and ARM64 packages cannot be applied to the wrong PC.
- User data now lives in its own folder (`%LOCALAPPDATA%\MemorySentryForSpicetify`) while the program installs under `%LOCALAPPDATA%\Programs\MemorySentryForSpicetify`, so an upgrade never leaves an orphaned second copy. An existing single-folder install migrates automatically and `settings.json` is stamped with a schema version; a corrupt or missing settings file no longer blocks startup (the bad file is kept as `settings.json.bad` and fresh defaults are written); an upgrade force-stops a hung server before replacing it instead of failing; and the Spicetify Custom App is renamed to `MemorySentryForSpicetify`, with the old one unregistered on upgrade so no duplicate sidebar icon remains.
- The Audio process is no longer pinned to a fixed 36 MB ceiling: it now adapts between 36 MB and 60 MB like the other roles, raising under sustained load at the ceiling and falling back when quiet; its state badge reads "Adaptive" once a limit is applied and reflects the ceiling currently in effect.
- The per-process memory bar now scales to each process's applied working-set ceiling, so it reflects the limit in effect instead of a fixed range.
- Process role badges are now localized for every role (Main, Audio, GPU, Renderer, Utility, Helper and Crashpad), not just a subset.
- The low-limit warning now applies only through the "Apply lower limit" button: dismissing it by clicking outside or pressing Escape restores the previous value, and it no longer reappears on its own. An "I understand this can cause problems; do not show this warning again" checkbox remembers the choice locally and applies below-default limits directly without the modal.
- Corrected singular wording: the one-second background-delay option and the low-limit warning's final countdown second now read "1 second" / "1 segundo", and the countdown tail shows "You can apply the limit now." / "Você já pode aplicar o limite." at zero.
- Documentation now matches the shipped product: the Audio ceiling is described as adaptive (36–60 MB), the uninstall and log wording reflects that logs stay in the data folder (which is left in place on uninstall), and the installer is presented as a per-user MSI for x64 and ARM64.
- Removed dead code flagged by the compiler and audits: unused Rust functions, dead CSS classes, unused dashboard translation strings, and unused command-line verbs in the server binary.

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
