# Privacy

Memory Sentry is designed to operate locally.

## Data collected

No information is uploaded by Memory Sentry. There is no telemetry endpoint, analytics SDK, crash-reporting service, or remote logging service in this project.

The server temporarily reads Windows process metadata needed to locate Spotify's process tree, estimate per-process CPU use, measure resident working sets, and verify process identity. Local dashboard metrics include process IDs, roles, working-set estimates, private-memory estimates, CPU estimates, page-fault counters, state transitions, adaptive policy decisions, and error messages.

## Local storage

Settings, an internal per-install access token, a recovery file containing original working-set limits, and diagnostic logs are stored under `%LOCALAPPDATA%\MemorySentryForSpicetify`. Logs are capped/rotated by the server, and the data folder (including its logs) is left in place when you uninstall, so a reinstall keeps them. No Spotify account credentials, track names, playlist data, or listening history are intentionally read or stored.

## Local API

The server binds only to `127.0.0.1`, checks the request host and Spotify HTTPS origin, and requires an internal per-install bearer token. The API is not designed to be exposed to a network interface.

## Dependencies and installation

The current server is written in Rust with no third-party runtime dependencies and does not require a global Node.js installation or Koffi. Node.js 22+ may be requested temporarily only to restore limits recorded by a legacy Node-based installation. The source and installer are public so the behavior can be audited. The installer does not ask for administrator privileges and does not disable operating-system security controls.

## User control

Automatic optimization can be switched off in the dashboard. When disabled or when Spotify becomes visible, the server attempts to restore saved original working-set limits. The package includes an uninstaller and recovery procedure. A forced process termination or system crash can interrupt cleanup, so original settings are persisted before applying changes and recovery is attempted on the next launch.
