//! Debug-only HTTP surface for tools/DebugConsole: window candidates as the product reads them,
//! per-process memory truth, and a way to arm or release a cap on demand.
//!
//! Compiled ONLY with `--features debug-api`. The default build - and therefore every CI run,
//! release and installed MSI - has none of these routes, so a final user's server answers them with
//! 404 and cannot be asked to move a working-set limit. That is why this module is separate instead
//! of folded into Api.rs: the gate is the module, not a runtime check.
//!
//! Everything here delegates to the functions the monitor already uses. It never implements a second
//! policy, so a debug build cannot cap or restore something the shipped engine would not touch, and a
//! test result measured through these routes is measured through the real path.

use super::*;

const MB_BYTES: f64 = 1024.0 * 1024.0;

/// Age of the last dashboard focus beacon in seconds, or None if the renderer has never answered.
fn client_age_seconds(state: &AppState) -> Option<f64> {
    if state.client_state_at_ms == 0 {
        return None;
    }
    Some(now_ms().saturating_sub(state.client_state_at_ms) as f64 / 1000.0)
}

/// The window manager's raw view, exactly as the monitor sees it: the candidate pool, the OS
/// foreground process, and the state the classification rule derives from those candidates right
/// now - kept next to the state the server last stored, so a stale read is visible as a difference.
pub(crate) fn windows_view(state: &AppState) -> String {
    let managed: Vec<u32> = descendants(state).into_iter().map(|(process, _)| process.pid).collect();
    let candidates = windows::window_candidates(&managed);
    let foreground_pid = windows::foreground_process_id();
    let foreground_hwnd = windows::foreground_window_hwnd();
    let is_foreground = spotify_process_is_foreground(foreground_pid, managed.iter().copied());
    let (derived_state, representative) = windows::classify_windows(&candidates, is_foreground);
    let candidate_json = candidates
        .iter()
        .map(|candidate| {
            format!(
                "{{\"pid\":{},\"hwnd\":{},\"title\":{},\"onScreen\":{},\"iconic\":{},\"area\":{}}}",
                candidate.pid,
                candidate.hwnd,
                json_quote(&candidate.title),
                candidate.on_screen,
                candidate.iconic,
                candidate.area
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let representative_json = match representative {
        Some(candidate) => format!(
            "{{\"pid\":{},\"hwnd\":{},\"area\":{},\"onScreen\":{},\"iconic\":{}}}",
            candidate.pid, candidate.hwnd, candidate.area, candidate.on_screen, candidate.iconic
        ),
        None => "null".to_string(),
    };
    format!(
        "{{\"ok\":true,\"stored\":{{\"windowState\":{},\"windowStateReason\":{},\"rootPid\":{},\"hwnd\":{},\"windowTitle\":{},\"clientForeground\":{},\"clientStateAgeSeconds\":{}}},\"live\":{{\"foregroundPid\":{},\"foregroundHwnd\":{},\"spotifyIsForeground\":{},\"derivedState\":\"{}\",\"representative\":{}}},\"managedPids\":[{}],\"candidates\":[{}]}}",
        json_quote(&state.window_state),
        json_quote(&state.window_state_reason),
        json_optional_u64(state.root_pid.map(u64::from)),
        state.hwnd,
        json_quote(&state.window_title),
        state.client_foreground,
        json_optional_number(client_age_seconds(state)),
        json_optional_u64(foreground_pid.map(u64::from)),
        foreground_hwnd.map(|hwnd| hwnd.to_string()).unwrap_or_else(|| "null".to_string()),
        is_foreground,
        derived_state,
        representative_json,
        managed
            .iter()
            .map(|pid| pid.to_string())
            .collect::<Vec<_>>()
            .join(","),
        candidate_json
    )
}

/// Per-process memory figures plus the two things a cap test needs: what the OS is enforcing right
/// now, and the recovery record the server holds for that process. Reading the enforced limits back
/// is what turns "the log says it capped" into evidence.
pub(crate) fn probe(state: &AppState, body: &str) -> String {
    let wanted_pid = parse_json_number(body, "pid");
    let rows: Vec<String> = descendants(state)
        .into_iter()
        .filter(|(process, _)| {
            wanted_pid.map(|pid| pid == process.pid as u64).unwrap_or(true)
        })
        .map(|(process, role)| {
            let limits_json = match windows::read_original_limits(process.pid, process.creation) {
                Ok(limits) => format!(
                    "{{\"minimumMB\":{},\"maximumMB\":{},\"flags\":{},\"hardMaxEnabled\":{}}}",
                    json_num(limits.minimum as f64 / MB_BYTES),
                    json_num(limits.maximum as f64 / MB_BYTES),
                    limits.flags,
                    limits.flags & windows::QUOTA_LIMITS_HARDWS_MAX_ENABLE != 0
                ),
                Err(error) => format!("{{\"error\":{}}}", json_quote(&error)),
            };
            let recovery_json = match state.active_caps.get(&process.pid) {
                Some(record) => format!(
                    "{{\"appliedMaximumMB\":{},\"savedMinimumMB\":{},\"savedMaximumMB\":{},\"savedFlags\":{},\"manual\":{},\"appliedAt\":{}}}",
                    json_optional_u64(record.applied_maximum_mb.map(u64::from)),
                    json_num(record.minimum as f64 / MB_BYTES),
                    json_num(record.maximum as f64 / MB_BYTES),
                    record.flags,
                    record.manual,
                    json_quote(&record.applied_at)
                ),
                None => "null".to_string(),
            };
            format!(
                "{{\"pid\":{},\"creation\":{},\"role\":{},\"identity\":{},\"workingSetMB\":{},\"privateWorkingSetMB\":{},\"privateMB\":{},\"pageFaultCount\":{},\"pageFaultsPerSec\":{},\"cpuPercent\":{},\"isAudio\":{},\"capped\":{},\"osLimits\":{},\"recovery\":{}}}",
                process.pid,
                process.creation,
                json_quote(role_name(role)),
                json_quote(&process_identity(&process.cmd, role)),
                json_optional_number(process.working_set_mb),
                json_optional_number(process.private_working_set_mb),
                json_optional_number(process.private_mb),
                json_optional_u64(process.page_fault_count),
                json_optional_number(process.page_faults_per_sec),
                json_optional_number(process.cpu_percent),
                state.audio_pids.contains(&process.pid),
                state.active_caps.contains_key(&process.pid),
                limits_json,
                recovery_json
            )
        })
        .collect();
    format!(
        "{{\"ok\":true,\"nowMs\":{},\"snapshotOk\":{},\"snapshotAgeMs\":{},\"windowState\":{},\"clientForeground\":{},\"clientStateAgeSeconds\":{},\"hiddenSinceMs\":{},\"processes\":[{}]}}",
        now_ms(),
        state.snapshot_ok,
        now_ms().saturating_sub(state.snapshot_at_ms),
        json_quote(&state.window_state),
        state.client_foreground,
        json_optional_number(client_age_seconds(state)),
        state.hidden_since_ms,
        rows.join(",")
    )
}

/// One managed process, addressed by PID or by its stable identity - never by a bare index, so a
/// script cannot cap the wrong thing after Spotify restarts.
fn find_target(state: &AppState, pid: Option<u64>, identity: Option<&str>) -> Result<(windows::ProcessRow, ProcessRole), String> {
    let targets = descendants(state);
    if targets.is_empty() {
        return Err("No managed processes: the app is not running or the snapshot is stale.".to_string());
    }
    match (pid, identity) {
        (Some(pid), _) => targets
            .into_iter()
            .find(|(process, _)| process.pid as u64 == pid)
            .ok_or_else(|| format!("PID {pid} is not a managed process right now.")),
        (None, Some(identity)) => live_target_for_identity(state, identity)
            .ok_or_else(|| format!("No live process has identity {identity}.")),
        (None, None) => Err("Provide either pid or identity.".to_string()),
    }
}

/// Arm a cap on one process through the product's own apply path and leave no configuration behind.
///
/// The engine reads its baseline from `autoRoleCaps`, so this writes that entry only for the instant
/// `apply_initial_cap` needs it, then puts the previous value back and persists the revert. The result
/// is a genuine engine-owned cap: it sits in the recovery map, it is durable in the backup file, and
/// the normal foreground/restore path is the only thing that can take it away. That is what makes a
/// restore test meaningful instead of self-referential.
pub(crate) fn force_cap(state: &mut AppState, body: &str) -> Result<String, String> {
    let maximum_mb = parse_json_number(body, "maximumMB").ok_or("maximumMB is required.")?;
    if maximum_mb < MIN_CAP_MB as u64 || maximum_mb > MAX_CAP_MB as u64 {
        return Err(format!(
            "maximumMB must be between {MIN_CAP_MB} and {MAX_CAP_MB}."
        ));
    }
    let identity = parse_json_string(body, "identity");
    let pid = parse_json_number(body, "pid");
    let (process, role) = find_target(state, pid, identity.as_deref())?;
    let target_identity = process_identity(&process.cmd, role);

    // Drop any cap the engine already owns here first, so the new baseline goes through the initial
    // apply rather than stacking on top of a record whose original limits were captured earlier.
    if let Some(record) = state.active_caps.get(&process.pid).cloned() {
        restore_one_record(state, &record, "debug re-arm")?;
    }

    let previous = state
        .settings
        .auto_role_caps
        .insert(target_identity.clone(), maximum_mb as u32);
    let applied = apply_initial_cap(state, &process, role, now_ms());
    match previous {
        Some(value) => {
            state.settings.auto_role_caps.insert(target_identity.clone(), value);
        }
        None => {
            state.settings.auto_role_caps.remove(&target_identity);
        }
    }
    applied?;
    // The temporary baseline may have reached disk inside the apply; persist the revert so the
    // settings file ends exactly as it began.
    let _ = write_settings(&state.settings_path, &state.settings);
    Ok(probe(state, &format!("{{\"pid\":{}}}", process.pid)))
}

/// Release caps through the product's restore path - one process or the whole fleet.
pub(crate) fn release_cap(state: &mut AppState, body: &str) -> Result<String, String> {
    let pid = parse_json_number(body, "pid");
    if parse_json_bool(body, "all").unwrap_or(false) || pid.is_none() {
        let (restored, failed) = restore_all(state, "debug request");
        if failed > 0 {
            return Err(format!("{failed} process(es) kept their limits."));
        }
        return Ok(format!("{{\"ok\":true,\"restored\":{restored}}}"));
    }
    let pid = pid.unwrap_or_default() as u32;
    let record = state
        .active_caps
        .get(&pid)
        .cloned()
        .ok_or_else(|| format!("No cap is recorded for PID {pid}."))?;
    restore_one_record(state, &record, "debug request")?;
    Ok(probe(state, &format!("{{\"pid\":{pid}}}")))
}

/// Empty a process's working set on demand. Used to see how fast a capped process is trimmed, which
/// is the visible half of a cap: a limit that never binds is not the same thing as a small working set.
pub(crate) fn empty_working_set(state: &AppState, body: &str) -> Result<String, String> {
    let pid = parse_json_number(body, "pid").ok_or("pid is required.")? as u32;
    let creation = descendants(state)
        .into_iter()
        .find(|(process, _)| process.pid == pid)
        .map(|(process, _)| process.creation)
        .ok_or_else(|| format!("PID {pid} is not a managed process right now."))?;
    windows::empty_working_set(pid, creation)?;
    Ok(format!("{{\"ok\":true,\"pid\":{pid}}}"))
}

/// Drive a real window for the debug console - minimize/restore/maximize/bottom/top/focus/blur - all
/// through the unsafe helpers in the Windows crate. Always answers 200 with an `ok` flag rather than
/// a 400, because a scenario's cleanup must be able to attempt a verb on a window that may already be
/// gone without the whole run throwing; the caller reads `ok`/`path`/`error` and decides what it means.
pub(crate) fn window_control(body: &str) -> String {
    let hwnd = match parse_json_number(body, "hwnd") {
        Some(hwnd) => hwnd,
        None => return format!("{{\"ok\":false,\"error\":{}}}", json_quote("hwnd is required.")),
    };
    let action = match parse_json_string(body, "action") {
        Some(action) => action,
        None => return format!("{{\"ok\":false,\"error\":{}}}", json_quote("action is required.")),
    };
    match windows::control_window(hwnd as usize, &action) {
        Ok(path) => format!(
            "{{\"ok\":true,\"action\":{},\"hwnd\":\"0x{hwnd:x}\",\"path\":{}}}",
            json_quote(&action),
            json_quote(&path)
        ),
        Err(error) => format!(
            "{{\"ok\":false,\"action\":{},\"hwnd\":\"0x{hwnd:x}\",\"error\":{}}}",
            json_quote(&action),
            json_quote(&error)
        ),
    }
}

/// System-level memory figures plus the settings that actually gate optimization on this machine. The
/// debug console's scenarios and its benchmark both tell the operator to check the `optimizeWhen*` flags
/// when a cap is handed back early, so they belong in this view or that advice points at a field the view
/// never prints. `windows::system_memory()` already reports megabytes, so totals pass through unchanged.
pub(crate) fn system_view(state: &AppState) -> String {
    let (total, available, load) = windows::system_memory();
    format!(
        "{{\"ok\":true,\"totalMB\":{},\"availableMB\":{},\"loadPercent\":{},\"pressureLevel\":\"{:?}\",\"enabled\":{},\"customMode\":{},\"hiddenDelaySeconds\":{},\"autoOptimize\":{},\"optimizeWhenBackground\":{},\"optimizeWhenMinimized\":{},\"optimizeWhenHidden\":{},\"activeCaps\":{}}}",
        json_num(total as f64),
        json_num(available as f64),
        load,
        windows::memory_pressure(),
        state.settings.enabled,
        state.settings.custom_mode,
        json_num(state.settings.hidden_delay_seconds),
        state.settings.auto_optimize,
        state.settings.optimize_when_background,
        state.settings.optimize_when_minimized,
        state.settings.optimize_when_hidden,
        state.active_caps.len()
    )
}
