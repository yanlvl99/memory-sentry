# Troubleshooting

Memory Sentry supports Windows 10 and Windows 11 desktop editions only. These steps apply to the experimental Memory Sentry server.

## Dashboard shows "connection unavailable"

The dashboard shows the detailed connection error. The launcher creates `%LOCALAPPDATA%\MemorySentryForSpicetify\logs\startup.log` before reading settings, so early configuration failures should now be recorded too.

The Marketplace entry only links to the project repository and cannot install Memory Sentry. The single supported installation method is the official per-user MSI at https://github.com/yanlvl99/memory-sentry/releases; there is no separate Marketplace build or custom portable installer.

If the log is missing, open PowerShell and run:

```powershell
& "$env:LOCALAPPDATA\Programs\MemorySentryForSpicetify\MemorySentry.exe" --launch
Get-Content "$env:LOCALAPPDATA\MemorySentryForSpicetify\logs\startup.log" -Tail 40
```

The first command prints a direct error if the launcher cannot initialize; the second shows local startup diagnostics if the file was created. If the executable itself is missing, repair the installation using the official MSI installer (run it and choose Repair). For runtime events after the local API starts, inspect `helper.log` in the same `logs` folder. These logs stay on the PC and are not uploaded. A "port already in use" error means another process is listening on port 38472. An "Invalid local token" error means the plugin and server were configured with different internal tokens; rerun the official installer to configure both together.

## Uninstall while Spotify is open

The uninstaller asks the server to restore the original Windows working-set limits and exit, removes its current-user startup registration, then removes the plugin from Spicetify and runs `spicetify apply`. That lets Spicetify refresh Spotify as part of removing the app; you should not need to close Spotify manually for a normal uninstall. If the server cannot confirm recovery or its process will not exit, uninstall stops and preserves the recovery files instead of deleting them.

## Two Memory Sentry entries in Programs and Features

Memory Sentry installs as a single per-user entry named after the current release (for recent builds, a `1.0.0` MSI version; the prerelease letter is visible only in the file name and in the server's `--version` output). If you also see a fixed `1.0.0.0` entry, it is a leftover from a pre-release build installed before the per-user MSI existed, in a machine-wide context the current installer cannot reach: its uninstall string points to a product that is not registered on the machine, so that row is inert. The app itself has no remnants (verify: the only `MemorySentry.exe` is under `%LOCALAPPDATA%\Programs\MemorySentryForSpicetify`). Leaving the dead row is harmless.

To remove the dead row while elevated (not required, and only meaningful on machines that had the old build) run PowerShell as administrator:

```powershell
reg.exe delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\{11504C42-5CF2-4B7F-B357-1562AFBD1561}" /f
```

## Spotify restart during setup

Setup does not require Spotify to be manually closed. It prepares or shuts down the previous Memory Sentry server, installs the plugin files, starts the local API, and then calls `spicetify apply`; Spicetify may restart Spotify itself.

## Setup says Spicetify was not found

Install [Spicetify CLI](https://spicetify.app/) for the same Windows user account that runs Spotify. Reopen Setup after installation. The installer reports this prerequisite before changing installed files and does not silently download or execute Spicetify on the user's behalf.

## Spotify takes a moment to accept input after being restored

Memory Sentry is designed to substantially reduce Spotify's resident working set while minimized, hidden, or in the background on supported Windows 10/11 x64 or ARM64 desktop PCs. Actual savings vary by PC and workload.

On very low-end PCs, the window can briefly appear black while Spotify's interface becomes visible again. **That alone does not prove a reload or loss of state.** Check whether the previous page and UI state return as they were. Input may also take a few seconds to respond after the window is restored. A persistent black screen, lost state, crash, interrupted playback, or unusably long delay is not something to dismiss as normal.

The dashboard's **Limit restoration duration** measures the server's limit-restoration operation, not the time until Spotify accepts the first input. Page faults may contribute to delay, but the available counters do not identify the cause of a particular event. If the problem persists, turn **Memory Sentry off** by selecting **Disabled** in the memory mode to stop all limits, then include local logs plus manually measured restore-to-interaction times in a report. Pinning a process to **Manual** only fixes that process's ceiling and does not stop limits.

## Working set dropped, but commit size or pagefile activity did not

This can be expected. Memory Sentry limits the **resident working set**—the process memory currently resident in physical RAM—and does not decommit memory. Task Manager's **Memory** column and the dashboard show resident working-set memory; the **Commit size** column reports committed virtual memory and can remain unchanged.

Private working set, commit size, and pagefile I/O are different measurements. Pages removed from a working set can remain cached in RAM and return through a soft page fault. Modified private pages may be written to the pagefile later if Windows needs their physical frames, but commit size alone does not show how much data is currently on disk. Use separate system-level counters to measure pagefile I/O.

The server applies role-based ceilings only to Spotify's process tree, never system-wide. The exact starting ceiling, floors and how the target responds to memory pressure are described in the [README](../README.md) and the [Benchmarking Guide](BENCHMARKING.md). Original limits are recorded for recovery; a restore is not guaranteed to complete immediately.

If hard faults, pagefile I/O, or restore latency rise sharply on a low-memory PC, the limits may be too aggressive for that machine. Turn **Memory Sentry off** by selecting **Disabled** in the memory mode to stop all limits, then include the measurements in a report. Custom mode is the only active mode; pinning a process to Manual only fixes that process's ceiling and does not stop limits.

## Uninstall stops because recovery failed

This is a deliberate safety stop. Retry uninstall after reviewing the error. If the server is missing but recovery records remain, reinstall the matching release so it can read the backup and restore the recorded settings. Do not manually delete working-set-backup.tsv, working-set-backup.previous, or legacy working-set-backup.json.

## Windows SmartScreen or antivirus warning

A new unsigned executable can show a warning even when its source is open. Verify the download is from the official repository and compare SHA-256. Never disable Defender/SmartScreen as an installation step. If you think a detection is a false positive, report the file hash and detection name on the project issue tracker.

## Logs

Diagnostic logs are written under the data folder and remain there after uninstall:

%LOCALAPPDATA%\MemorySentryForSpicetify\logs

The server does not upload logs or metrics. Review the logs for personal file paths before attaching them to a public issue.
