//! Install, configuration, autostart and upgrade operations for the Memory Sentry server. These
//! are the CLI-time entry points (configure / swap / pre-uninstall / restore-limits / check-os)
//! plus Spicetify integration. They reuse the shared runtime state and persistence helpers that
//! remain in the parent module (visible to descendants through `use super::*;`).

use memory_sentry_windows as windows;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use super::*;

fn load_app_token(index_path: &Path) -> Option<String> {
    let source = fs::read_to_string(index_path).ok()?;
    let marker = "const MSS_TOKEN = '";
    let pos = source.find(marker)? + marker.len();
    let rest = &source[pos..];
    let end = rest.find('\'')?;
    let token = rest[..end].to_string();
    (token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit())).then_some(token)
}

fn write_configured_app(index_path: &Path, token: &str) -> Result<(), String> {
    let source =
        fs::read_to_string(index_path).map_err(|e| format!("Custom App file is missing: {e}"))?;
    let updated = if source.contains("__MSS_TOKEN__") {
        source.replace("__MSS_TOKEN__", token)
    } else if load_app_token(index_path).as_deref() == Some(token) {
        source
    } else {
        return Err("The Custom App has no token placeholder and does not match the saved settings. Reinstall the signed-off package and retry.".to_string());
    };
    let temp = index_path.with_extension("js.tmp");
    fs::write(&temp, updated).map_err(|e| format!("Could not stage the Custom App: {e}"))?;
    replace_file(&temp, index_path)
}

fn find_spicetify() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PATH") {
        for folder in std::env::split_paths(&path) {
            let candidate = folder.join("spicetify.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
            let candidate = folder.join("spicetify");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for key in ["LOCALAPPDATA", "APPDATA"] {
        if let Some(root) = std::env::var_os(key) {
            let candidate = PathBuf::from(root).join("spicetify").join("spicetify.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn configure_spicetify(app_name: &str) -> Result<PathBuf, String> {
    let cli = find_spicetify().ok_or(
        "Spicetify CLI was not found. Install Spicetify for the same Windows user that runs Spotify, then rerun Memory Sentry Setup."
    )?;
    let current = Command::new(&cli)
        .args(["config", "custom_apps"])
        .output()
        .map_err(|e| {
            format!(
                "Could not read Spicetify configuration using {}: {e}",
                cli.display()
            )
        })?;
    if !current.status.success() {
        return Err(format!(
            "Spicetify could not read its custom_apps configuration: {}",
            String::from_utf8_lossy(&current.stderr).trim()
        ));
    }
    let current_value = String::from_utf8_lossy(&current.stdout);
    let already_listed = current_value
        .split(|ch: char| ch == '|' || ch.is_whitespace() || ch == '=')
        .any(|item| item.eq_ignore_ascii_case(app_name));
    if !already_listed {
        let output = Command::new(&cli)
            .args(["config", "custom_apps", app_name])
            .output()
            .map_err(|e| format!("Could not register the Custom App with Spicetify: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "Spicetify could not register the Custom App: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(cli)
}

fn apply_spicetify(cli: &Path) -> Result<(), String> {
    let applied = Command::new(cli)
        .arg("apply")
        .output()
        .map_err(|e| format!("Could not run spicetify apply using {}: {e}", cli.display()))?;
    if !applied.status.success() {
        return Err(format!(
            "Spicetify apply failed (exit code {:?}): {}",
            applied.status.code(),
            String::from_utf8_lossy(&applied.stderr).trim()
        ));
    }
    Ok(())
}

fn stop_monitor_after_setup_failure(context: &str) -> String {
    match prepare_upgrade() {
        Ok(()) => format!("{context}; the server was stopped and working-set recovery was checked."),
        Err(cleanup_error) => format!("{context}; automatic cleanup could not be fully confirmed: {cleanup_error}. Keep the recovery files and retry Setup."),
    }
}

/// Best-effort removal of the pre-split Custom App so an upgrade does not leave a duplicate
/// sidebar icon. It drops the old `custom_apps` entry and deletes the old folder; the pending
/// `spicetify apply` refreshes the client. Missing entries are ignored.
fn cleanup_legacy_custom_app(cli: &Path) {
    let removal = format!("{LEGACY_APP_NAME}-");
    let _ = Command::new(cli)
        .args(["config", "custom_apps", removal.as_str()])
        .output();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let legacy_dir = PathBuf::from(appdata)
            .join("spicetify")
            .join("CustomApps")
            .join(LEGACY_APP_NAME);
        if legacy_dir.exists() {
            let _ = fs::remove_dir_all(&legacy_dir);
        }
    }
}

pub fn configure_install(app_dir_arg: &str) -> Result<(), String> {
    let (os_name, build) = windows::detect_supported_client_os()?;
    // Reject a mismatched-architecture installer (e.g. the x64 package chosen on an ARM64 PC that
    // cannot emulate it) before touching any file. The MSI ConfigureApp custom action runs with
    // Return="check", so returning Err rolls the whole install back with this message.
    let built_arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    let native_arch = windows::native_arch()?;
    if built_arch != native_arch {
        return Err(format!(
            "This Memory Sentry installer is built for {built_arch} PCs, but this machine is {native_arch}. Download and run the matching {native_arch} installer instead; nothing was changed."
        ));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let base = exe
        .parent()
        .ok_or("Could not determine installation directory")?
        .to_path_buf();
    let data = data_dir()?;
    let legacy = install_dir()?;
    let settings_path = data.join("settings.json");
    // Tolerate a stray wrapping quote or trailing backslash from the caller's command line so a
    // well-intentioned path never looks like a missing package.
    let app_dir = PathBuf::from(app_dir_arg.trim().trim_matches('"').trim_end_matches('\\'));
    let index_path = app_dir.join("index.js");
    if !index_path.is_file() || !app_dir.join("manifest.json").is_file() {
        return Err(
            "The Spicetify Custom App package is incomplete (index.js or manifest.json missing)."
                .to_string(),
        );
    }

    // Legacy Node installations must restore before this server takes over.
    let legacy_backup = legacy.join("working-set-backup.json");
    let legacy_helper = legacy.join("helper.js");
    if legacy_backup.exists() {
        if !legacy_helper.is_file() {
            return Err("A legacy Node recovery backup exists but helper.js is missing. Restore the old limits before migrating; recovery data was preserved.".to_string());
        }
        stop_legacy_node_helper(&legacy)?;
        let node = find_node().ok_or("A legacy Node recovery backup exists. Install Node.js 22+ temporarily so the old helper can restore limits, then rerun setup.")?;
        let result = Command::new(node)
            .arg(&legacy_helper)
            .arg("--restore-limits")
            .output()
            .map_err(|e| format!("Could not start legacy working-set recovery: {e}"))?;
        if !result.status.success() {
            return Err("The previous helper could not restore every working-set limit. Installation stopped and the recovery backup was preserved.".to_string());
        }
        cleanup_legacy_node_files(&legacy)?;
    } else if legacy_helper.exists() {
        // No legacy backup means the previous helper is idle; stop only this project's
        // matching Node process and remove its now-obsolete runtime files.
        cleanup_legacy_node_files(&legacy)?;
    }

    // Carry the previous single-directory settings/logs into the data folder before reading them,
    // so an upgrade keeps the user's configuration. Copy-only and idempotent; the installer removes
    // the legacy folder afterwards.
    migrate_legacy_layout()?;

    let existing_raw = fs::read_to_string(&settings_path).unwrap_or_default();
    let existing_token = parse_json_string(&existing_raw, "token");
    let app_token = load_app_token(&index_path);
    let token = existing_token
        .filter(|value| value.len() >= 32)
        .or(app_token)
        .unwrap_or(windows::random_token()?);
    write_configured_app(&index_path, &token)?;
    let settings = Settings {
        port: DEFAULT_PORT,
        token,
        enabled: parse_json_bool(&existing_raw, "enabled").unwrap_or(true),
        auto_optimize: parse_json_bool(&existing_raw, "autoOptimize").unwrap_or(true),
        custom_mode: parse_json_bool(&existing_raw, "customMode").unwrap_or(false),
        hidden_delay_seconds: parse_json_number_f64(&existing_raw, "hiddenDelaySeconds")
            .unwrap_or(DEFAULT_HIDDEN_DELAY_SECONDS)
            .clamp(0.5, 60.0),
        refresh_interval_ms: parse_json_number(&existing_raw, "refreshIntervalMs")
            .unwrap_or(DEFAULT_REFRESH_INTERVAL_MS)
            .clamp(250, 10_000),
        trim_to_cap: parse_json_bool(&existing_raw, "trimToCap").unwrap_or(true),
        optimize_when_background: parse_json_bool(&existing_raw, "optimizeWhenBackground")
            .unwrap_or(true),
        optimize_when_minimized: parse_json_bool(&existing_raw, "optimizeWhenMinimized")
            .unwrap_or(true),
        optimize_when_hidden: parse_json_bool(&existing_raw, "optimizeWhenHidden").unwrap_or(true),
        manual_role_caps: parse_manual_role_caps(&existing_raw),
        auto_role_caps: parse_auto_role_caps(&existing_raw),
        usage_scores: parse_usage_scores(&existing_raw),
        safe_floors: parse_safe_floors(&existing_raw),
        schema_version: CURRENT_SCHEMA_VERSION,
    };
    write_settings(&settings_path, &settings)?;
    // Start the local API before "spicetify apply" restarts Spotify, so the
    // Custom App does not race server startup during its first render.
    let cli = configure_spicetify(APP_NAME)?;
    cleanup_legacy_custom_app(&cli);
    if let Err(error) = launch_monitor() {
        return Err(stop_monitor_after_setup_failure(&format!(
            "Could not start the server before applying Spicetify: {error}"
        )));
    }
    if let Err(error) = apply_spicetify(&cli) {
        return Err(stop_monitor_after_setup_failure(&error));
    }
    if let Err(error) = register_startup(&exe) {
        return Err(stop_monitor_after_setup_failure(&format!(
            "Spicetify was applied, but current-user startup could not be registered: {error}"
        )));
    }
    cleanup_legacy_startup();
    println!("{APP_NAME} configured for {os_name} build {build}.");
    println!("Server: {}", base.display());
    println!("Dashboard: {}", app_dir.display());
    println!("Current-user startup is registered and the local API passed its health check. No telemetry is uploaded.");
    Ok(())
}

fn find_node() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PATH") {
        for folder in std::env::split_paths(&path) {
            let candidate = folder.join("node.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn register_startup(exe: &Path) -> Result<(), String> {
    let value = format!("\"{}\" --launch", exe.display());
    let output = Command::new("reg.exe")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "Memory Sentry",
            "/t",
            "REG_SZ",
            "/d",
            &value,
            "/f",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not register current-user startup: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Could not register Memory Sentry startup: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn unregister_startup() -> Result<(), String> {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let query = Command::new("reg.exe")
        .args(["query", key, "/v", "Memory Sentry"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not inspect current-user startup registration: {e}"))?;
    if !query.success() {
        return Ok(());
    }
    let removed = Command::new("reg.exe")
        .args(["delete", key, "/v", "Memory Sentry", "/f"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not remove current-user startup registration: {e}"))?;
    if !removed.success() {
        return Err("Could not remove Memory Sentry from current-user startup. Uninstall stopped to avoid a broken startup entry.".to_string());
    }
    Ok(())
}

fn unregister_spicetify() -> Result<(), String> {
    let Some(cli) = find_spicetify() else {
        return Ok(());
    };
    let removal = format!("{APP_NAME}-");
    let output = Command::new(&cli)
        .args(["config", "custom_apps", removal.as_str()])
        .output()
        .map_err(|e| format!("Could not remove the Memory Sentry Custom App registration: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Spicetify could not unregister the Custom App: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let apply = Command::new(&cli)
        .arg("apply")
        .output()
        .map_err(|e| format!("Could not apply the Spicetify configuration: {e}"))?;
    if !apply.status.success() {
        return Err(format!(
            "Could not apply the Spicetify removal: {}",
            String::from_utf8_lossy(&apply.stderr).trim()
        ));
    }
    Ok(())
}

fn read_legacy_node_backup() -> bool {
    if let Ok(dir) = install_dir() {
        dir.join("working-set-backup.json").exists()
    } else {
        false
    }
}

pub fn prepare_upgrade() -> Result<(), String> {
    let program = program_dir()?;
    let data = data_dir()?;
    let legacy = install_dir()?;
    // Settings may still live in the old single directory until this upgrade migrates them.
    let settings_path = if data.join("settings.json").is_file() {
        data.join("settings.json")
    } else {
        legacy.join("settings.json")
    };

    // Migrate the previous Node helper only after its own recovery command succeeds.
    if read_legacy_node_backup() {
        let legacy_helper = legacy.join("helper.js");
        if !legacy_helper.is_file() {
            return Err("A legacy Node recovery backup exists but helper.js is missing. Recovery data was preserved.".to_string());
        }
        stop_legacy_node_helper(&legacy)?;
        let node = find_node().ok_or("A legacy Node recovery backup exists. Install Node.js 22+ temporarily so the old helper can restore limits.".to_string())?;
        let output = Command::new(node)
            .arg(&legacy_helper)
            .arg("--restore-limits")
            .output()
            .map_err(|e| format!("Could not start legacy working-set recovery: {e}"))?;
        if !output.status.success() {
            return Err(
                "Legacy recovery failed. Upgrade stopped and the backup was preserved.".to_string(),
            );
        }
        cleanup_legacy_node_files(&legacy)?;
    }

    // Ask a running instance of the server to restore limits and exit. This is best-effort: an
    // unreadable settings file must not block the forced stop that follows.
    if settings_path.is_file() {
        if let Ok(settings) = load_settings(&settings_path) {
            let _ = request_shutdown(settings.port, &settings.token);
        }
        thread::sleep(Duration::from_millis(400));
    }

    // Stop any running server before migrating data or replacing files, matching either the old
    // single-directory executable or the new program-directory executable so a lingering
    // pre-split process releases its file lock and the single-instance mutex.
    let old_exe = legacy.join("MemorySentry.exe");
    let installed_exe = program.join("MemorySentry.exe");
    windows::force_terminate_installed_process(&old_exe)?;
    windows::force_terminate_installed_process(&installed_exe)?;

    // With the previous instance stopped, the old directory is unlocked and data can move.
    migrate_legacy_layout()?;

    // Recover any durable records left behind by a crash or an unresponsive helper.
    let backup_path = data.join("working-set-backup.tsv");
    let records = load_backup_file(&backup_path);
    let mut failures = Vec::new();
    for record in &records {
        match windows::restore_working_set_limits(
            record.pid,
            record.creation,
            record.minimum,
            record.maximum,
            record.flags,
        ) {
            Ok(_) => {}
            Err(error) => failures.push(format!("PID {}: {error}", record.pid)),
        }
    }
    if !failures.is_empty() {
        return Err(format!("Upgrade stopped because some working-set limits could not be restored; recovery data was preserved:\n{}", failures.join("\n")));
    }
    let _ = fs::remove_file(&backup_path);
    let _ = fs::remove_file(backup_path.with_extension("previous"));
    let _ = fs::remove_file(backup_path.with_extension("tsv.tmp"));
    Ok(())
}

/// Replace the installed server with a new build while Spotify keeps running.
/// The running instance restores its limits and exits, the new executable is put in
/// place, and the monitor is relaunched; the plugin reconnects on its next poll.
pub fn swap_server(new_exe_arg: &str) -> Result<(), String> {
    let source = PathBuf::from(new_exe_arg);
    if !source.is_file() {
        return Err(format!(
            "New server executable not found at {}.",
            source.display()
        ));
    }
    let program = program_dir()?;
    let data = data_dir()?;
    let settings_path = data.join("settings.json");
    let installed_exe = program.join("MemorySentry.exe");
    let pid_file = data.join("helper.pid");

    // Stop the running instance first; it restores limits before exiting.
    if settings_path.exists() {
        let settings = load_settings(&settings_path)?;
        let _ = request_shutdown(settings.port, &settings.token);
        thread::sleep(Duration::from_millis(400));
    }
    for _ in 0..50 {
        let active_pid = fs::read_to_string(&pid_file)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        if active_pid
            .map(|pid| windows::process_matches_executable(pid, &installed_exe))
            .unwrap_or(false)
        {
            thread::sleep(Duration::from_millis(100));
        } else {
            break;
        }
    }
    // A process that ignored the shutdown request is force-terminated before replacement.
    windows::force_terminate_installed_process(&installed_exe)?;

    // In-place swap is only possible after the old process released the executable.
    let same_file = source
        .canonicalize()
        .ok()
        .map(|value| installed_exe.canonicalize().ok() == Some(value))
        .unwrap_or(false);
    if !same_file {
        fs::create_dir_all(&program).map_err(|e| e.to_string())?;
        fs::copy(&source, &installed_exe)
            .map_err(|e| format!("Could not replace {}: {e}", installed_exe.display()))?;
    }

    let launch = Command::new(&installed_exe)
        .arg("--launch")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not relaunch the server: {e}"))?;
    if !launch.success() {
        return Err(
            "The new server did not pass its local API health check. See logs\\startup.log."
                .to_string(),
        );
    }
    println!(
        "Memory Sentry server swapped to the new build at {}.",
        installed_exe.display()
    );
    println!("Spotify was not restarted. The plugin reconnects by itself within a few seconds.");
    Ok(())
}

pub fn pre_uninstall() -> Result<(), String> {
    // Do not make the user close Spotify just to uninstall. Restore and stop the helper first,
    // remove its logon launch registration, then let spicetify apply refresh/restart Spotify.
    let base = data_dir()?;
    prepare_upgrade()?;
    unregister_startup()?;
    unregister_spicetify()?;
    cleanup_legacy_startup();
    let _ = fs::remove_file(base.join("helper.pid"));
    println!("Memory Sentry was stopped, recovery was attempted, and startup/app registration was removed. Logs remain in the data folder.");
    Ok(())
}

fn request_shutdown(port: u16, token: &str) -> Result<(), String> {
    use std::io::{Read, Write};
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("Could not reach the running server: {e}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
    let body = r#"{"action":"shutdown"}"#;
    let request = format!(
        "POST /api/action HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: https://xpui.app.spotify.com\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    if !response.starts_with("HTTP/1.1 200") {
        return Err(format!(
            "Server did not confirm restoration: {}",
            response.lines().next().unwrap_or("no response")
        ));
    }
    Ok(())
}

pub fn check_supported_os() -> Result<(), String> {
    let (name, build) = windows::detect_supported_client_os()?;
    println!("Supported system detected: {name} build {build}.");
    Ok(())
}

fn cleanup_legacy_startup() {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let startup = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup");
        let _ = fs::remove_file(startup.join("Spicetify Memory Sentry.vbs"));
        let _ = fs::remove_file(startup.join("Spicetify Memory Sentry.lnk"));
    }
}

fn stop_legacy_node_helper(base: &Path) -> Result<(), String> {
    let helper = base.join("helper.js");
    if !helper.exists() {
        return Ok(());
    }
    let safe_path = helper.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$target = '{}'; Get-CimInstance Win32_Process -Filter \"Name='node.exe'\" -ErrorAction SilentlyContinue | Where-Object {{ $_.CommandLine -and $_.CommandLine.Contains($target) -and $_.CommandLine.Contains('helper.js') }} | ForEach-Object {{ Stop-Process -Id ([int]$_.ProcessId) -Force -ErrorAction SilentlyContinue }}; exit 0",
        safe_path
    );
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not stop the legacy Memory Sentry Node process: {e}"))?;
    if !output.success() {
        return Err("Could not stop the legacy Memory Sentry Node process safely; recovery files were preserved.".to_string());
    }
    Ok(())
}

fn cleanup_legacy_node_files(base: &Path) -> Result<(), String> {
    stop_legacy_node_helper(base)?;
    let _ = fs::remove_file(base.join("helper.js"));
    let _ = fs::remove_file(base.join("working-set-backup.json"));
    let _ = fs::remove_dir_all(base.join("node_modules"));
    cleanup_legacy_startup();
    Ok(())
}

pub fn restore_limits_command() -> Result<(), String> {
    let base = data_dir()?;
    if read_legacy_node_backup() {
        return Err("This installation still has the legacy Node recovery format. Use its installed Node helper to restore limits before changing versions.".to_string());
    }
    let backup_path = base.join("working-set-backup.tsv");
    let records = load_backup_file(&backup_path);
    let mut failed = Vec::new();
    for record in &records {
        match windows::restore_working_set_limits(
            record.pid,
            record.creation,
            record.minimum,
            record.maximum,
            record.flags,
        ) {
            Ok(windows::RestoreOutcome::Restored) => println!("Restored PID {}", record.pid),
            Ok(windows::RestoreOutcome::ProcessGone | windows::RestoreOutcome::PidReused) => {
                println!("Skipped exited/reused PID {}", record.pid)
            }
            Err(error) => failed.push(format!("PID {}: {error}", record.pid)),
        }
    }
    if !failed.is_empty() {
        return Err(format!(
            "Some limits could not be restored; recovery data was preserved:\n{}",
            failed.join("\n")
        ));
    }
    let _ = fs::remove_file(&backup_path);
    let _ = fs::remove_file(backup_path.with_extension("previous"));
    let _ = fs::remove_file(backup_path.with_extension("tsv.tmp"));
    println!("Recovery completed. {} record(s) processed.", records.len());
    Ok(())
}
