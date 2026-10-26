# Troubleshooting

Memory Sentry supports Windows 10 and Windows 11 desktop editions only. These steps apply to the experimental Memory Sentry server.

## Dashboard shows "connection unavailable"

The dashboard shows the detailed connection error. The launcher creates `%LOCALAPPDATA%\SpicetifyMemorySentry\logs\startup.log` before reading settings, so early configuration failures should now be recorded too.

The Marketplace entry only links to the project repository and cannot install Memory Sentry. The single supported installation method is the official Setup EXE at https://github.com/yanlvl99/spicetify-memory-sentry/releases; there is no separate Marketplace build or custom portable installer.

If the log is missing, open PowerShell and run:

```powershell
& "$env:LOCALAPPDATA\SpicetifyMemorySentry\MemorySentry.exe" --launch
Get-Content "$env:LOCALAPPDATA\SpicetifyMemorySentry\logs\startup.log" -Tail 40
```

The first command prints a direct error if the launcher cannot initialize; the second shows local startup diagnostics if the file was created. If the executable itself is missing, repair the installation using the official setup executable. For runtime events after the local API starts, inspect `helper.log` in the same `logs` folder. These logs stay on the PC and are not uploaded. A "port already in use" error means another process is listening on port 38472. An "Invalid local token" error means the plugin and server were configured with different internal tokens; rerun the official installer to configure both together.

## Uninstall while Spotify is open

The uninstaller asks the server to restore the original Windows working-set limits and exit, removes its current-user startup registration, then removes the plugin from Spicetify and runs `spicetify apply`. That lets Spicetify refresh Spotify as part of removing the app; you should not need to close Spotify manually for a normal uninstall. If the server cannot confirm recovery or its process will not exit, uninstall stops and preserves the recovery files instead of deleting them.

## Spotify restart during setup

Setup does not require Spotify to be manually closed. It prepares or shuts down the previous Memory Sentry server, installs the plugin files, starts the local API, and then calls `spicetify apply`; Spicetify may restart Spotify itself.

## Setup says Spicetify was not found

Install Spicetify CLI for the same Windows user account that runs Spotify. Reopen Setup after installation. The installer reports this prerequisite before changing installed files and does not silently download or execute Spicetify on the user's behalf.

## Setup refuses a legacy migration

A previous version may have working-set-backup.json containing original Win32 limits. Setup stops its matching old Node server and tries the old server's restore command before deleting legacy files. If Node.js 22+ is missing, install it temporarily from https://nodejs.org/en/download and rerun setup. If any limit cannot be restored, keep the recovery files and contact support; do not delete the backup or force the upgrade.

## Spotify takes a moment to accept input after being restored

Memory Sentry is designed to substantially reduce Spotify's resident working set while minimized, hidden, or in the background on supported Windows 10/11 x64 desktop PCs. Actual savings vary by PC and workload.

On very low-end PCs, the window can briefly appear black while Spotify's interface becomes visible again. **That alone does not prove a reload or loss of state.** Check whether the previous page and UI state return as they were. Input may also take a few seconds to respond after the window is restored. A persistent black screen, lost state, crash, interrupted playback, or unusably long delay is not something to dismiss as normal.

The dashboard's **Limit restoration duration** measures the server's limit-restoration operation, not the time until Spotify accepts the first input. Page faults may contribute to delay, but the available counters do not identify the cause of a particular event. If the problem persists, turn **Memory Sentry off** with the dashboard power switch to stop both automatic and manual limits, then include local logs plus manually measured restore-to-interaction times in a report. Turning off only **Automatic policy** selects manual mode and does not stop background limits.

## Memory dropped, but commit size or pagefile usage did not

This is expected. Memory Sentry limits the **resident working set**, meaning how much of each Spotify process is currently in physical RAM. It does not free or decommit the process's memory. Task Manager's **Memory** column and the dashboard show working-set memory; the **Commit size** column can stay unchanged.

Trimmed pages usually remain cached in RAM on the standby/transition lists and can come back through a soft page fault without disk I/O. Private pages modified by the process may later be written to the pagefile if Windows needs their physical frames for something else, so the pagefile is not bypassed by a special mechanism and committed memory is not reduced. What distinguishes Memory Sentry from one-shot "RAM optimizer" tools is that it applies adaptive ceilings only to Spotify's process tree, gives audio-session processes a fixed 36 MB automatic ceiling, and restores the original limits; it does not claim to release memory that the process still owns.

If hard page faults, pagefile I/O, or restore latency rise sharply on a low-memory PC, the limits may be too aggressive for that machine. Turn **Memory Sentry off** with the dashboard power switch to stop automatic and manual limits, then include the measurements in a report. Turning off only **Automatic policy** switches to manual limits.

## Uninstall stops because recovery failed

This is a deliberate safety stop. Retry uninstall after reviewing the error. If the server is missing but recovery records remain, reinstall the matching release so it can read the backup and restore the recorded settings. Do not manually delete working-set-backup.tsv, working-set-backup.previous, or legacy working-set-backup.json.

## Windows SmartScreen or antivirus warning

A new unsigned executable can show a warning even when its source is open. Verify the download is from the official repository and compare SHA-256. Never disable Defender/SmartScreen as an installation step. If you think a detection is a false positive, report the file hash and detection name on the project issue tracker.

## Logs

After uninstall, diagnostic logs that existed are copied to:

%LOCALAPPDATA%\\MemorySentry\\logs

The server does not upload logs or metrics. Review the logs for personal file paths before attaching them to a public issue.
