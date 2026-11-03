//! The Spotify process monitor. This module owns the polling loop and everything derived from it:
//! classifying processes into roles and PID-free identities, applying and restoring working-set
//! ceilings, tracking the foreground/background window state, and driving the adaptive policy
//! decisions via the Core crate. The shared types, constants and path/time helpers live in the
//! parent module and reach this one through `use super::*;`.

use memory_sentry_core::{
    decide_adjustment, estimate_cpu_percent, estimate_faults_per_second, faults_since, memory_tier,
    usage_ewma, AdjustmentReason, AppProfile, ProcessRole, HIGH_CPU_PERCENT,
    LOW_CPU_PERCENT, MAX_CAP_MB, MIN_CAP_MB,
};
use memory_sentry_spotify::SpotifyProfile;
use memory_sentry_windows as windows;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use super::*;

// The one place the managed application is chosen. Every app-specific decision (which executable to
// enumerate, how its command lines map to roles, how to name sibling processes) flows through this
// profile, so retargeting the server to another desktop app means swapping this static for a different
// `AppProfile` implementor crate and nothing else in the generic monitor changes.
static APP_PROFILE: SpotifyProfile = SpotifyProfile;

pub(crate) fn app_profile() -> &'static dyn AppProfile {
    &APP_PROFILE
}


pub(crate) fn build_state(base: PathBuf, settings: Settings) -> AppState {
    AppState {
        settings_path: base.join("settings.json"),
        backup_path: base.join("working-set-backup.tsv"),
        log_path: base.join("logs").join("helper.log"),
        settings,
        started_at: iso_now(),
        snapshot: windows::Snapshot::default(),
        snapshot_ok: false,
        snapshot_at_ms: 0,
        snapshot_serial: 0,
        root_pid: None,
        hwnd: 0,
        window_title: String::new(),
        window_state: "not-detected".to_string(),
        window_state_reason: "startup".to_string(),
        hidden_since_ms: 0,
        last_restore_attempt_at_ms: 0,
        last_memory_sample_at_ms: 0,
        last_status_request_ms: 0,
        active_caps: BTreeMap::new(),
        profiles: BTreeMap::new(),
        target_retry_at_ms: BTreeMap::new(),
        page_fault_baselines: BTreeMap::new(),
        cpu_baselines: BTreeMap::new(),
        audio_pids: BTreeSet::new(),
        audio_pids_error: String::new(),
        telemetry: Telemetry::default(),
        recent_events: VecDeque::new(),
        last_error: String::new(),
        last_action: None,
        shutdown_requested: false,
    }
}

pub fn run_monitor() -> Result<(), String> {
    windows::detect_supported_client_os()?;
    migrate_legacy_layout()?;
    let base = data_dir()?;
    fs::create_dir_all(base.join("logs")).map_err(|e| e.to_string())?;
    let settings_path = base.join("settings.json");
    let settings = match load_settings(&settings_path) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!(
                "Recovering unusable settings at {}: {error}",
                settings_path.display()
            );
            bootstrap_default_settings(&settings_path)?
        }
    };
    let _instance = windows::acquire_single_instance()?;
    let mut state = build_state(base.clone(), settings);
    for record in load_backup_file(&state.backup_path) {
        state.active_caps.insert(record.pid, record);
    }
    if !state.active_caps.is_empty() {
        let result = restore_all(&mut state, "recovery at startup");
        log_event(
            &mut state,
            format!(
                "Startup recovery: {} restored/gone, {} pending.",
                result.0, result.1
            ),
            if result.1 > 0 { "warn" } else { "info" },
        );
    }
    let pid_file = base.join("helper.pid");
    let _ = fs::write(&pid_file, windows::current_process_id().to_string());

    let shared = Arc::new(Mutex::new(state));
    if let Err(error) = start_api(Arc::clone(&shared)) {
        let _ = fs::remove_file(&pid_file);
        return Err(error);
    }
    let (tx, rx) = mpsc::channel::<Result<windows::Snapshot, String>>();
    let snapshot_shared = Arc::clone(&shared);
    thread::spawn(move || loop {
        // Prefer pure Win32 discovery (no per-tick subprocess). Fall back to the proven PowerShell/WMI
        // path whenever native enumeration fails outright, OR finds processes but reads none of their
        // command lines - a broad PEB-read failure would collapse every identity to the bare role, so we
        // would rather pay the old cost for that pass than mis-attribute caps. An empty native result
        // with no processes is trusted (Spotify simply is not running) so idle passes stay cheap.
        let native = windows::native_process_snapshot(app_profile().target_executables());
        let peb_failed_broadly = native
            .as_ref()
            .map(|snapshot| !snapshot.processes.is_empty() && snapshot.processes.iter().all(|p| p.cmd.is_empty()))
            .unwrap_or(false);
        let result = match native {
            Ok(snapshot) if !peb_failed_broadly => Ok(snapshot),
            _ => windows::powershell_snapshot(app_profile().process_image_name()),
        };
        // Stamp the observation instant before handing the snapshot over: the main loop drains the
        // channel up to a poll interval later, and CPU windows measured from that acceptance time
        // would be too short, inflating the next percentage.
        let result = result.map(|mut snapshot| {
            snapshot.captured_at_ms = now_ms();
            snapshot
        });
        if tx.send(result).is_err() {
            break;
        }
        // Refresh the process list sooner while the dashboard is watching; otherwise keep the economical
        // background interval. Native discovery is cheap enough that neither cadence spawns a subprocess.
        let watched = snapshot_shared
            .lock()
            .map(|s| now_ms().saturating_sub(s.last_status_request_ms) < WATCH_STALE_MS)
            .unwrap_or(false);
        let interval = if watched { WATCH_SNAPSHOT_MS } else { SNAPSHOT_INTERVAL_MS };
        thread::sleep(Duration::from_millis(interval));
    });

    {
        let mut s = shared.lock().map_err(|_| "State lock poisoned")?;
        log_event(&mut s, format!("Memory Sentry {VERSION} started. Supported OS: Windows 10/11. Adaptive cap={MIN_CAP_MB}-{MAX_CAP_MB} MB. No external telemetry."), "info");
    }

    loop {
        while let Ok(snapshot_result) = rx.try_recv() {
            let mut s = match shared.lock() {
                Ok(s) => s,
                Err(_) => return Err("State lock poisoned".to_string()),
            };
            match snapshot_result {
                Ok(snapshot) => accept_snapshot(&mut s, snapshot),
                Err(error) => {
                    s.snapshot_ok = false;
                    if s.last_error != error {
                        s.last_error = error.clone();
                        log_event(
                            &mut s,
                            format!("Process discovery failed: {error}"),
                            "error",
                        );
                    }
                }
            }
        }

        let should_shutdown = {
            let mut s = match shared.lock() {
                Ok(s) => s,
                Err(_) => return Err("State lock poisoned".to_string()),
            };
            let is_foreground = spotify_process_is_foreground(
                windows::foreground_process_id(),
                s.snapshot.processes.iter().map(|process| process.pid),
            );
            let (current_window_state, reason) = observe_window_state(&mut s, is_foreground);
            if current_window_state != s.window_state {
                let message = format!("Spotify window state: {current_window_state} ({reason})");
                s.window_state = current_window_state.clone();
                s.window_state_reason = reason;
                log_event(&mut s, message, "info");
            }
            let now = now_ms();
            let watched = now.saturating_sub(s.last_status_request_ms) < WATCH_STALE_MS;
            // The watched memory cadence is now a user-tunable setting (the dashboard refresh rate),
            // already clamped on load; the idle cadence stays the economical fixed interval.
            let memory_interval = if watched {
                s.settings.refresh_interval_ms
            } else {
                MEMORY_SAMPLE_INTERVAL_MS
            };
            if now.saturating_sub(s.last_memory_sample_at_ms) >= memory_interval {
                refresh_memory(&mut s, now);
            }
            maybe_auto_optimize(&mut s, now);
            s.shutdown_requested
        };
        if should_shutdown {
            break;
        }
        thread::sleep(Duration::from_millis(FOCUS_POLL_INTERVAL_MS));
    }

    let mut s = shared.lock().map_err(|_| "State lock poisoned")?;
    let result = restore_all(&mut s, "server shutdown");
    log_event(
        &mut s,
        format!(
            "Shutdown recovery: {} restored/gone, {} pending.",
            result.0, result.1
        ),
        if result.1 > 0 { "error" } else { "info" },
    );
    // Persist the cross-session usage EWMA on the way out. It is also flushed whenever a cap
    // adjusts, but a mostly-quiet process may never trigger one - this guarantees the learned
    // profile survives to the next launch so the first minutes are already personalized.
    if let Err(error) = write_settings(&s.settings_path, &s.settings) {
        log_event(&mut s, format!("Could not persist usage profile at shutdown: {error}"), "warn");
    }
    let _ = fs::remove_file(&pid_file);
    Ok(())
}

pub(crate) fn accept_snapshot(state: &mut AppState, mut snapshot: windows::Snapshot) {
    let sampled_at = now_ms();
    // Baselines pair with the moment the counters were captured, not with this acceptance time: the
    // channel can hold a snapshot for up to a full poll interval, and stamping it late would shrink
    // the window the next writer divides against and inflate its percentage.
    let observed_at = if snapshot.captured_at_ms > 0 { snapshot.captured_at_ms } else { sampled_at };
    for process in &mut snapshot.processes {
        let key = target_key(process.pid, process.creation);
        // The percentage divides by the exact age of the shared cpu baseline, never by the nominal
        // snapshot window. refresh_memory advances that baseline on its own (faster) cadence, so
        // reusing the previous snapshot's accumulated cpu_time over this wider window would dilute
        // every reading. Both writers store (cpu ms, observed at) so each delta pairs with its own
        // elapsed time.
        process.cpu_percent = state
            .cpu_baselines
            .get(&key)
            .copied()
            .and_then(|(previous_cpu, previous_at)| {
                estimate_cpu_percent(
                    previous_cpu,
                    process.cpu_time_ms as u64,
                    observed_at.saturating_sub(previous_at),
                )
            })
            .map(|value| (value * 10.0).round() / 10.0);
        state
            .cpu_baselines
            .insert(key, (process.cpu_time_ms as u64, observed_at));
        // Page faults follow the same contract as the cpu percentage above: the delta and the rate are
        // taken against the exact baseline this snapshot read, and the baseline carries when it was
        // captured so the next reader - whichever cadence it runs on - divides by its own window.
        let previous_faults = state.page_fault_baselines.get(&key).copied();
        process.page_faults_delta = previous_faults
            .zip(process.page_fault_count)
            .map(|((previous, _), current)| faults_since(previous, current));
        process.page_faults_per_sec = previous_faults
            .zip(process.page_fault_count)
            .and_then(|((previous, previous_at), current)| {
                estimate_faults_per_second(
                    previous,
                    current,
                    observed_at.saturating_sub(previous_at),
                )
            });
        if let Some(current) = process.page_fault_count {
            state
                .page_fault_baselines
                .insert(key, (current, observed_at));
        }
    }

    state.snapshot = snapshot;
    state.snapshot_at_ms = sampled_at;
    state.snapshot_serial += 1;
    state.snapshot_ok = true;
    let is_foreground = spotify_process_is_foreground(
        windows::foreground_process_id(),
        state.snapshot.processes.iter().map(|process| process.pid),
    );
    let (window_state, reason) = observe_window_state(state, is_foreground);
    state.window_state = window_state;
    state.window_state_reason = reason;
    state.last_error.clear();
    // Drop stale profiles only when the snapshot proves that a PID has a new creation identity.
    let active: BTreeMap<u32, u64> = state
        .snapshot
        .processes
        .iter()
        .map(|p| (p.pid, p.creation))
        .collect();
    state
        .profiles
        .retain(|pid, profile| active.get(pid).copied() == Some(profile.creation));
    state
        .target_retry_at_ms
        .retain(|(pid, creation), _| active.get(pid).copied() == Some(*creation));
    state
        .cpu_baselines
        .retain(|(pid, creation), _| active.get(pid).copied() == Some(*creation));
    refresh_audio_session_pids(state);
}

pub(crate) fn refresh_audio_session_pids(state: &mut AppState) {
    match windows::audio_session_pids() {
        Ok(pids) => {
            let pids: BTreeSet<u32> = pids.into_iter().collect();
            if pids != state.audio_pids {
                let listed = if pids.is_empty() {
                    "none".to_string()
                } else {
                    pids.iter()
                        .map(|pid| pid.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                state.audio_pids = pids;
                log_event(
                    state,
                    format!("Audio session processes updated: {listed}."),
                    "info",
                );
            }
            state.audio_pids_error.clear();
        }
        Err(error) => {
            if state.audio_pids_error != error {
                state.audio_pids_error = error.clone();
                log_event(
                    state,
                    format!(
                        "Audio session discovery failed: {error}; keeping {} known PID(s).",
                        state.audio_pids.len()
                    ),
                    "warn",
                );
            }
        }
    }
}

pub(crate) fn is_child_process_command(cmd: &str) -> bool {
    app_profile().is_child_process(cmd)
}

pub(crate) fn spotify_process_is_foreground(
    foreground_pid: Option<u32>,
    spotify_process_ids: impl IntoIterator<Item = u32>,
) -> bool {
    foreground_pid
        .map(|pid| {
            spotify_process_ids
                .into_iter()
                .any(|spotify_pid| spotify_pid == pid)
        })
        .unwrap_or(false)
}

/// Classify a process into its role by delegating to the active application profile, which owns the
/// app-specific command-line tokens. The audio-session PIDs (discovered from the OS, not the command
/// line) are passed through so the profile can give them precedence over any stray token.
pub(crate) fn process_role(
    process: &windows::ProcessRow,
    root_pid: Option<u32>,
    audio_pids: &BTreeSet<u32>,
) -> ProcessRole {
    app_profile().classify_role(&process.cmd, process.pid, root_pid, audio_pids)
}

pub(crate) fn role_name(role: ProcessRole) -> &'static str {
    role.label()
}

/// A short sub-identity that disambiguates processes sharing a coarse role (e.g. two renderers, or
/// network vs storage utilities), delegated to the application profile. It is PID-free, so it survives
/// a restart and can key a per-process cap.
pub(crate) fn process_sub_label(cmd: &str, role: ProcessRole) -> Option<String> {
    app_profile().sub_label(cmd, role)
}

/// The persistence key for per-process caps: role plus the live sub-label when one exists
/// (`Renderer:#7`, `Utility:network`), otherwise the bare role name. Never a PID (which changes on
/// every launch), so a cap survives a restart of the managed application.
pub(crate) fn process_identity(cmd: &str, role: ProcessRole) -> String {
    app_profile().identity(cmd, role)
}

/// Whether an identity string's leading segment is a known role. Guards against malformed keys
/// surviving in the settings file; identities may themselves contain ':' (e.g. `Renderer:#7`).
pub(crate) fn identity_role_prefix_ok(identity: &str) -> bool {
    ProcessRole::is_known_label(identity.split(':').next().unwrap_or(""))
}

/// In Custom mode a manual cap stored under an EXACT identity key is a "pin": that process keeps
/// its fixed ceiling while the rest of the fleet stays adaptive. Only an exact identity match
/// counts, so a bare-role default never pins every sibling of the role.
pub(crate) fn identity_pin_cap(settings: &Settings, process: &windows::ProcessRow, role: ProcessRole) -> Option<u32> {
    settings
        .manual_role_caps
        .get(&process_identity(&process.cmd, role))
        .copied()
}

pub(crate) fn descendants(state: &AppState) -> Vec<(windows::ProcessRow, ProcessRole)> {
    let Some(root_pid) = state.root_pid else {
        return Vec::new();
    };
    let mut ids = BTreeSet::new();
    ids.insert(root_pid);
    loop {
        let before = ids.len();
        for process in &state.snapshot.processes {
            if ids.contains(&process.ppid) {
                ids.insert(process.pid);
            }
        }
        if ids.len() == before {
            break;
        }
    }
    state
        .snapshot
        .processes
        .iter()
        .filter(|p| ids.contains(&p.pid))
        .map(|p| {
            (
                p.clone(),
                process_role(p, Some(root_pid), &state.audio_pids),
            )
        })
        .collect()
}

// In-memory bookkeeping key for one concrete process instance. The tuple is Copy, so the hot paths
// (every focus poll while hidden, every snapshot accept, every memory refresh) can key retry timers
// and fault baselines without allocating a String per process per pass.
pub(crate) fn target_key(pid: u32, creation: u64) -> (u32, u64) {
    (pid, creation)
}

pub(crate) fn ensure_profile(state: &mut AppState, pid: u32, creation: u64, role: ProcessRole, now: u64) {
    if state.profiles.get(&pid).map(|p| p.creation) == Some(creation) {
        return;
    }
    let baseline = role.baseline_cap_mb().unwrap_or(24);
    state.profiles.insert(
        pid,
        AdaptiveProfile {
            creation,
            current_cap_mb: baseline,
            high_cpu_ms: 0,
            low_cpu_ms: 0,
            last_evaluated_snapshot_at_ms: state.snapshot_at_ms,
            last_adjustment_at_ms: 0,
            last_reason: "baseline".to_string(),
            last_cpu_percent: None,
            last_working_set_mb: None,
            last_trim_at_ms: now.saturating_sub(TRIM_COOLDOWN_MS),
        },
    );
}

pub(crate) fn persist_or_log(state: &mut AppState) -> bool {
    match persist_backups(state) {
        Ok(()) => true,
        Err(error) => {
            log_event(
                state,
                format!("Could not persist recovery data: {error}"),
                "error",
            );
            false
        }
    }
}

pub(crate) fn apply_initial_cap(
    state: &mut AppState,
    process: &windows::ProcessRow,
    role: ProcessRole,
    now: u64,
) -> Result<bool, String> {
    if state.active_caps.get(&process.pid).map(|r| r.creation) == Some(process.creation) {
        ensure_profile(state, process.pid, process.creation, role, now);
        return Ok(false);
    }
    // Do not restore a stale backup to a process that has reused the PID.
    if state.active_caps.contains_key(&process.pid) {
        state.active_caps.remove(&process.pid);
        let _ = persist_or_log(state);
    }
    let limits = windows::read_original_limits(process.pid, process.creation)?;
    let before = process.working_set_mb;
    let baseline = configured_auto_cap(&state.settings, process, role)
        .unwrap_or_else(|| role.baseline_cap_mb().unwrap_or(24));
    let record = BackupRecord::capture_original(process.pid, process.creation, role, limits, false);
    state.active_caps.insert(process.pid, record);
    // Recovery must be durable before the working-set limit is changed.
    if !persist_or_log(state) {
        state.active_caps.remove(&process.pid);
        return Err("Recovery record could not be saved; no memory limit was changed.".to_string());
    }
    let cap_bytes = baseline as usize * 1024 * 1024;
    let minimum = windows::MIN_WORKING_SET_BYTES.min(cap_bytes);
    if let Err(error) = windows::set_working_set_limits(
        process.pid,
        process.creation,
        minimum,
        cap_bytes,
        windows::HARD_CAP_FLAGS,
    ) {
        state.active_caps.remove(&process.pid);
        let _ = persist_or_log(state);
        return Err(error);
    }
    if let Some(record) = state.active_caps.get_mut(&process.pid) {
        record.record_applied_cap(role, baseline, false);
    }
    if !persist_or_log(state) {
        // Limits were changed, so keep the in-memory record and stop applying further caps.
        return Err("Cap was applied, but the backup update failed. The in-memory recovery record remains active.".to_string());
    }
    ensure_profile(state, process.pid, process.creation, role, now);
    let mut trimmed = false;
    if !state.settings.trim_to_cap && before.map(|mb| mb > baseline as f64).unwrap_or(false) {
        trimmed = windows::empty_working_set(process.pid, process.creation).is_ok();
    }
    let after = windows::inspect_process(process.pid)
        .ok()
        .map(|m| m.working_set_mb);
    log_event(
        state,
        format!(
            "Applied automatic cap to {} PID {}: {} MB before, {} MB after{}.",
            role_name(role),
            process.pid,
            before
                .map(|v| format!("{v:.1}"))
                .unwrap_or_else(|| "unknown".to_string()),
            after
                .map(|v| format!("{v:.1}"))
                .unwrap_or_else(|| "unknown".to_string()),
            if trimmed {
                ", one-time trim requested"
            } else {
                ""
            }
        ),
        "info",
    );
    Ok(true)
}

pub(crate) fn apply_manual_cap(
    state: &mut AppState,
    process: &windows::ProcessRow,
    role: ProcessRole,
    cap_mb: u32,
) -> Result<(), String> {
    if !(MIN_MANUAL_CAP_MB..=MAX_MANUAL_CAP_MB).contains(&cap_mb) {
        return Err(format!(
            "Manual limits must be between {MIN_MANUAL_CAP_MB} and {MAX_MANUAL_CAP_MB} MB."
        ));
    }
    let previous = state
        .active_caps
        .get(&process.pid)
        .filter(|record| record.creation == process.creation)
        .cloned();
    if state.active_caps.contains_key(&process.pid) && previous.is_none() {
        state.active_caps.remove(&process.pid);
        if !persist_or_log(state) {
            return Err(
                "Could not clear stale recovery data; no manual limit was changed.".to_string(),
            );
        }
    }
    let mut record = if let Some(record) = previous.clone() {
        record
    } else {
        let limits = windows::read_original_limits(process.pid, process.creation)?;
        BackupRecord::capture_original(process.pid, process.creation, role, limits, true)
    };
    record.record_applied_cap(role, cap_mb, true);
    state.active_caps.insert(process.pid, record);
    if !persist_or_log(state) {
        if let Some(previous) = previous {
            state.active_caps.insert(process.pid, previous);
        } else {
            state.active_caps.remove(&process.pid);
        }
        return Err("Recovery data could not be saved; no memory limit was changed.".to_string());
    }
    let cap_bytes = cap_mb as usize * 1024 * 1024;
    let minimum = windows::MIN_WORKING_SET_BYTES.min(cap_bytes);
    if let Err(error) = windows::set_working_set_limits(
        process.pid,
        process.creation,
        minimum,
        cap_bytes,
        windows::HARD_CAP_FLAGS,
    ) {
        if let Some(previous) = previous {
            state.active_caps.insert(process.pid, previous);
        } else {
            state.active_caps.remove(&process.pid);
        }
        let _ = persist_or_log(state);
        return Err(error);
    }
    let trimmed = !state.settings.trim_to_cap
        && process
            .working_set_mb
            .map(|working_set_mb| working_set_mb > cap_mb as f64)
            .unwrap_or(false)
        && windows::empty_working_set(process.pid, process.creation).is_ok();
    log_event(
        state,
        format!(
            "Applied manual {} limit to PID {}: {cap_mb} MB{}.",
            role_name(role),
            process.pid,
            if trimmed {
                "; one-time trim requested"
            } else {
                ""
            }
        ),
        "info",
    );
    Ok(())
}

pub(crate) fn restore_one_record(
    state: &mut AppState,
    record: &BackupRecord,
    reason: &str,
) -> Result<(), String> {
    match windows::restore_working_set_limits(
        record.pid,
        record.creation,
        record.minimum,
        record.maximum,
        record.flags,
    ) {
        Ok(windows::RestoreOutcome::Restored) => {
            log_event(
                state,
                format!(
                    "Restored original working-set limits for {} PID {} ({reason}).",
                    record.role, record.pid
                ),
                "info",
            );
        }
        Ok(windows::RestoreOutcome::ProcessGone) => {
            log_event(
                state,
                format!("Removed recovery for exited PID {} ({reason}).", record.pid),
                "warn",
            );
        }
        Ok(windows::RestoreOutcome::PidReused) => {
            log_event(
                state,
                format!(
                    "PID {} was reused; did not touch the new process ({reason}).",
                    record.pid
                ),
                "warn",
            );
        }
        Err(error) => return Err(error),
    }
    state.active_caps.remove(&record.pid);
    state.profiles.remove(&record.pid);
    if !persist_or_log(state) {
        return Err(
            "Original limits were restored, but recovery data could not be updated.".to_string(),
        );
    }
    Ok(())
}

pub(crate) fn restore_all(state: &mut AppState, reason: &str) -> (usize, usize) {
    let records: Vec<BackupRecord> = state.active_caps.values().cloned().collect();
    if records.is_empty() {
        return (0, 0);
    }
    // Resolve liveness from the current snapshot so we never hand limits back to a process instance
    // that is gone (exited or PID reused). Recovery is keyed to the live identity: if that exact
    // (pid, creation) is no longer running there is nothing to restore, and attempting it is what
    // produced spurious "could not restore PID" noise. Only trust this when discovery succeeded; on a
    // stale/failed snapshot fall through to the Windows call, which still reports gone/reused safely.
    let trust_snapshot = state.snapshot_ok;
    let live: std::collections::HashSet<(u32, u64)> = if trust_snapshot {
        state
            .snapshot
            .processes
            .iter()
            .map(|p| (p.pid, p.creation))
            .collect()
    } else {
        std::collections::HashSet::new()
    };
    let started = std::time::Instant::now();
    let mut restored_or_gone = 0usize;
    let mut failed = 0usize;
    for record in records {
        if trust_snapshot && !live.contains(&(record.pid, record.creation)) {
            state.active_caps.remove(&record.pid);
            state.profiles.remove(&record.pid);
            restored_or_gone += 1;
            log_event(
                state,
                format!(
                    "Skipped restore for {} PID {}: that process is no longer running ({reason}).",
                    record.role, record.pid
                ),
                "info",
            );
            continue;
        }
        match windows::restore_working_set_limits(
            record.pid,
            record.creation,
            record.minimum,
            record.maximum,
            record.flags,
        ) {
            Ok(windows::RestoreOutcome::Restored) => {
                state.active_caps.remove(&record.pid);
                state.profiles.remove(&record.pid);
                restored_or_gone += 1;
                log_event(
                    state,
                    format!(
                        "Restored original working-set limits for {} PID {} ({reason}).",
                        record.role, record.pid
                    ),
                    "info",
                );
            }
            Ok(windows::RestoreOutcome::ProcessGone) => {
                state.active_caps.remove(&record.pid);
                state.profiles.remove(&record.pid);
                restored_or_gone += 1;
                log_event(
                    state,
                    format!("Removed recovery for exited PID {} ({reason}).", record.pid),
                    "warn",
                );
            }
            Ok(windows::RestoreOutcome::PidReused) => {
                state.active_caps.remove(&record.pid);
                state.profiles.remove(&record.pid);
                restored_or_gone += 1;
                log_event(
                    state,
                    format!(
                        "PID {} was reused; did not touch the new process ({reason}).",
                        record.pid
                    ),
                    "warn",
                );
            }
            Err(error) => {
                // Win32 error 5 (access denied) means we can no longer modify that process - it is
                // exiting, already gone, or protected. The durable backup is intact and restoration is
                // retried, so this is benign and must NOT raise the red helper-error banner or count as
                // a failure. Any other error is a genuine restore problem and still surfaces as an error.
                let benign = error.contains("error 5");
                if !benign {
                    failed += 1;
                }
                log_event(
                    state,
                    format!("{error}; recovery was preserved."),
                    if benign { "warn" } else { "error" },
                );
            }
        }
    }
    let _ = persist_or_log(state);
    state.telemetry.last_restore_duration_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
    state.telemetry.last_restore_at = Some(iso_now());
    state.telemetry.last_restore_restored = restored_or_gone as u32;
    state.telemetry.last_restore_failures = failed as u32;
    (restored_or_gone, failed)
}

pub(crate) fn update_caps(state: &mut AppState, now: u64) {
    if !state.snapshot_ok
        || state.snapshot_at_ms == 0
        || now.saturating_sub(state.snapshot_at_ms) > SNAPSHOT_STALE_MS
    {
        return;
    }
    let targets = descendants(state);
    // Installed-RAM tier is read once per pass and fed to every sample so headroom scales with the
    // machine. GlobalMemoryStatusEx is cheap; no need to cache it across the loop.
    let (system_total_mb, _, _) = windows::system_memory();
    for (process, role) in targets {
        let record = state.active_caps.get(&process.pid).cloned();
        let Some(record) = record else {
            continue;
        };
        if record.creation != process.creation {
            continue;
        }
        // A manual (pinned) cap is a fixed ceiling: leave it for the manual path, never adapt it.
        if record.manual {
            continue;
        }
        let identity = process_identity(&process.cmd, role);
        ensure_profile(state, process.pid, process.creation, role, now);
        let profile_snapshot_at = state
            .profiles
            .get(&process.pid)
            .map(|p| p.last_evaluated_snapshot_at_ms)
            .unwrap_or(0);
        if profile_snapshot_at == state.snapshot_at_ms {
            continue;
        }
        let interval = if profile_snapshot_at > 0 {
            state.snapshot_at_ms.saturating_sub(profile_snapshot_at)
        } else {
            0
        };
        let current_cap = record
            .applied_maximum_mb
            .or_else(|| state.profiles.get(&process.pid).map(|p| p.current_cap_mb))
            .unwrap_or(role.baseline_cap_mb().unwrap_or(24));
        // A stored cap can fall outside the current role's range when the process is reclassified
        // (Spotify moves the audio session between Main and a dedicated Audio process, for example).
        // The policy refuses to act on an out-of-range cap, so without this the process would be
        // frozen at an invalid ceiling forever. Re-base it into range and let the next pass adapt.
        let role_min = role.minimum_cap_mb().unwrap_or(MIN_CAP_MB);
        let role_max = role
            .maximum_cap_mb()
            .unwrap_or(MAX_CAP_MB)
            .saturating_add(memory_tier(system_total_mb).maximum_bonus_mb())
            .min(MAX_CAP_MB);
        // Effective floor for this identity in this window state: the absolute minimum, the role's
        // state floor (Audio 25, a visible-background Renderer 38) and the learned safe edge.
        let effective_floor = state
            .settings
            .safe_floors
            .get(&identity)
            .copied()
            .unwrap_or(MIN_CAP_MB)
            .max(role.state_floor_mb(state.window_state == "background-visible"))
            .max(role_min)
            .min(role_max);
        let rebased = current_cap.clamp(effective_floor, role_max);
        if rebased != current_cap {
            let cap_bytes = rebased as usize * 1024 * 1024;
            let minimum_bytes = windows::MIN_WORKING_SET_BYTES.min(cap_bytes);
            match windows::set_working_set_limits(
                process.pid,
                process.creation,
                minimum_bytes,
                cap_bytes,
                windows::HARD_CAP_FLAGS,
            ) {
                Ok(()) => {
                    if let Some(record) = state.active_caps.get_mut(&process.pid) {
                        record.record_applied_cap(role, rebased, false);
                    }
                    if let Some(profile) = state.profiles.get_mut(&process.pid) {
                        profile.current_cap_mb = rebased;
                        profile.last_reason = "role-rebased".to_string();
                        profile.high_cpu_ms = 0;
                        profile.low_cpu_ms = 0;
                    }
                    let _ = persist_or_log(state);
                    if let Err(error) = write_settings(&state.settings_path, &state.settings) {
                        log_event(
                            state,
                            format!("Could not persist the re-based cap for PID {}: {error}", process.pid),
                            "warn",
                        );
                    }
                    log_event(
                        state,
                        format!(
                            "Re-based {} PID {} cap from {} MB to {} MB for its current role.",
                            role_name(role),
                            process.pid,
                            current_cap,
                            rebased
                        ),
                        "info",
                    );
                }
                Err(error) => log_event(
                    state,
                    format!("Could not re-base PID {} cap: {error}", process.pid),
                    "warn",
                ),
            }
            continue;
        }
        let cpu = process.cpu_percent;
        let working_set = process.working_set_mb;
        if let Some(profile) = state.profiles.get_mut(&process.pid) {
            profile.last_evaluated_snapshot_at_ms = state.snapshot_at_ms;
            profile.last_cpu_percent = cpu;
            profile.last_working_set_mb = working_set;
            match cpu {
                Some(cpu) if cpu >= HIGH_CPU_PERCENT => {
                    profile.high_cpu_ms = profile.high_cpu_ms.saturating_add(interval);
                    profile.low_cpu_ms = 0;
                }
                Some(cpu) if cpu <= LOW_CPU_PERCENT => {
                    profile.low_cpu_ms = profile.low_cpu_ms.saturating_add(interval);
                    profile.high_cpu_ms = 0;
                }
                _ => {
                    profile.high_cpu_ms = 0;
                    profile.low_cpu_ms = 0;
                }
            }
        }
        // Cross-session usage telemetry: fold this window's instantaneous busy
        // fraction - the process's own CPU busy percent, independent of the cap - into a slow EWMA
        // keyed by identity, persisted with the settings. Skipped on cold/zero-length windows so a
        // fresh launch seeds from the learned prior, not a bogus first sample. Local-only, never
        // uploaded (see PRIVACY.md).
        if let (Some(cpu), true) = (cpu, interval > 0) {
            let blended = usage_ewma(state.settings.usage_scores.get(&identity).copied(), cpu);
            state.settings.usage_scores.insert(identity.clone(), blended);
        }
        let profile = match state.profiles.get(&process.pid) {
            Some(p) => p.clone(),
            None => continue,
        };
        let cpu = cpu.unwrap_or(0.0);
        let ws = working_set.unwrap_or(current_cap as f64);
        // Page faults per second for the exact window this row's delta came from, stamped by the
        // sampler that took it. Never divide that delta by the snapshot interval: the memory refresh
        // advances the counter faster, so the two windows differ and the quotient would be diluted.
        let faults_per_sec = process.page_faults_per_sec.unwrap_or(0.0);
        // Personalize the squeeze from the learned cross-session score once we have one; before the
        // first persisted sample fall back to this session's high/low ratio, then the neutral default
        // (Frequent) so an identity with no history behaves exactly as it did before this feature.
        let busy_percent = match state.settings.usage_scores.get(&identity) {
            Some(score) => *score as f64,
            None => {
                let sum = profile.high_cpu_ms.saturating_add(profile.low_cpu_ms);
                if sum > 0 {
                    ((profile.high_cpu_ms as f64) / (sum as f64)) * 100.0
                } else {
                    10.0
                }
            }
        };
        let profile_sample = memory_sentry_core::PolicySample {
            role,
            current_cap_mb: current_cap,
            working_set_mb: ws,
            cpu_percent: cpu,
            busy_percent,
            high_cpu_ms: profile.high_cpu_ms,
            low_cpu_ms: profile.low_cpu_ms,
            faults_per_sec,
            minimum_cap_mb: effective_floor,
            system_total_mb,
            last_adjustment_ms: profile.last_adjustment_at_ms,
            now_ms: now,
        };
        if let Some(decision) = decide_adjustment(profile_sample) {
            let old_cap = current_cap;
            let next_cap = decision.next_cap_mb;
            let cap_bytes = next_cap as usize * 1024 * 1024;
            let minimum = windows::MIN_WORKING_SET_BYTES.min(cap_bytes);
            match windows::set_working_set_limits(
                process.pid,
                process.creation,
                minimum,
                cap_bytes,
                windows::HARD_CAP_FLAGS,
            ) {
                Ok(()) => {
                    if let Some(record) = state.active_caps.get_mut(&process.pid) {
                        record.record_applied_cap(role, next_cap, false);
                    }
                    state.settings.auto_role_caps.insert(process_identity(&process.cmd, role), next_cap);
                    // A relief increase is the engine finding this identity's safe edge: remember it
                    // so the next quiet window cannot descend back into the cap that just starved it.
                    if matches!(
                        decision.reason,
                        AdjustmentReason::ThrashRelief | AdjustmentReason::CpuStarvation
                    ) {
                        let floor = state
                            .settings
                            .safe_floors
                            .entry(identity.clone())
                            .or_insert(MIN_CAP_MB);
                        if next_cap > *floor {
                            *floor = next_cap;
                        }
                    }
                    if let Some(profile) = state.profiles.get_mut(&process.pid) {
                        profile.current_cap_mb = next_cap;
                        profile.last_adjustment_at_ms = now;
                        profile.last_reason = match decision.reason {
                            AdjustmentReason::SustainedHighCpu => "sustained-high-cpu",
                            AdjustmentReason::LowCpuAndMemoryHeadroom => "low-cpu-and-memory-headroom",
                            AdjustmentReason::ThrashRelief => "thrash-relief",
                            AdjustmentReason::CpuStarvation => "cpu-starvation",
                            AdjustmentReason::IdleProactive => "idle-proactive",
                        }
                        .to_string();
                        profile.high_cpu_ms = 0;
                        profile.low_cpu_ms = 0;
                    }
                    state.telemetry.adaptive_adjustments += 1;
                    state.telemetry.last_adaptive_adjustment_at = Some(iso_now());
                    let mut trimmed = false;
                    if !state.settings.trim_to_cap
                        && next_cap < old_cap
                        && ws > next_cap as f64 + 4.0
                        && now.saturating_sub(profile.last_trim_at_ms) >= TRIM_COOLDOWN_MS
                    {
                        trimmed =
                            windows::empty_working_set(process.pid, process.creation).is_ok();
                        if let Some(p) = state.profiles.get_mut(&process.pid) {
                            p.last_trim_at_ms = now;
                        }
                    }
                    if let Err(error) = persist_backups(state) {
                        log_event(state, format!("Adjusted PID {} to {next_cap} MB but could not persist updated recovery state: {error}", process.pid), "error");
                    } else if let Err(error) = write_settings(&state.settings_path, &state.settings) {
                        log_event(state, format!("Adjusted PID {} to {next_cap} MB but could not persist settings: {error}", process.pid), "error");
                    } else {
                        log_event(state, format!(
                            "Adaptive cap {} {} PID {} from {old_cap} MB to {next_cap} MB (CPU {:.1}%, working set {:.1} MB{}).",
                            if next_cap > old_cap { "raised" } else { "lowered" }, role_name(role), process.pid, cpu, ws,
                            if trimmed { ", one-time trim requested" } else { "" }
                        ), "info");
                    }
                }
                Err(error) => log_event(
                    state,
                    format!("Adaptive update skipped for PID {}: {error}", process.pid),
                    "warn",
                ),
            }
        }
    }
}

pub(crate) fn manual_cap_for(role: ProcessRole, configured: Option<u32>) -> u32 {
    configured.unwrap_or_else(|| role.manual_default_cap_mb())
}

/// Rebuild the identity key from a role and an already-parsed sub-label, so a caller that has the
/// sub-label in hand (the status payload does) never re-parses the command line to derive it. Mirrors
/// exactly what `AppProfile::identity` produces from `sub_label`.
pub(crate) fn identity_from_parts(role: ProcessRole, sub_label: Option<&str>) -> String {
    match sub_label {
        Some(sub) => format!("{}:{}", role.label(), sub),
        None => role.label().to_string(),
    }
}

/// Manual-cap lookup keyed on a precomputed identity: the exact per-process identity wins, else the
/// bare-role entry acts as the shared default. Split from `configured_manual_cap` so the per-poll
/// status builder can supply the identity it already derived instead of re-parsing the command line.
pub(crate) fn manual_cap_for_identity(settings: &Settings, identity: &str, role: ProcessRole) -> Option<u32> {
    settings
        .manual_role_caps
        .get(identity)
        .copied()
        .or_else(|| settings.manual_role_caps.get(role.label()).copied())
}

pub(crate) fn configured_manual_cap(
    settings: &Settings,
    process: &windows::ProcessRow,
    role: ProcessRole,
) -> Option<u32> {
    // Per-process identity wins; a bare-role entry (older data, or roles with no sub-label) then
    // acts as the shared default; finally a legacy PID-scoped cap is honoured for its exact PID.
    let identity = process_identity(&process.cmd, role);
    manual_cap_for_identity(settings, &identity, role)
}

pub(crate) fn configured_auto_cap(
    settings: &Settings,
    process: &windows::ProcessRow,
    role: ProcessRole,
) -> Option<u32> {
    let identity = process_identity(&process.cmd, role);
    settings
        .auto_role_caps
        .get(&identity)
        .copied()
        .or_else(|| settings.auto_role_caps.get(role_name(role)).copied())
}

pub(crate) fn reconcile_manual_caps(state: &mut AppState) {
    if !state.snapshot_ok
        || state.snapshot_at_ms == 0
        || now_ms().saturating_sub(state.snapshot_at_ms) > SNAPSHOT_STALE_MS
    {
        return;
    }
    let targets = descendants(state);
    let wanted: BTreeSet<(u32, u64)> = targets
        .iter()
        .map(|(process, _)| target_key(process.pid, process.creation))
        .collect();
    let records: Vec<BackupRecord> = state.active_caps.values().cloned().collect();
    let mut pruned = false;
    for record in records {
        let is_live = wanted.contains(&target_key(record.pid, record.creation));
        if !is_live {
            // The process is gone or its PID was reused: its original limits are moot and a restore
            // can never succeed. Drop the record (once, at warn) instead of retrying every poll.
            state.active_caps.remove(&record.pid);
            state.profiles.remove(&record.pid);
            pruned = true;
            continue;
        }
        if !record.manual {
            if let Err(error) = restore_one_record(state, &record, "manual policy reconciliation") {
                log_event(
                    state,
                    format!("Could not restore live PID {}: {error}", record.pid),
                    "warn",
                );
            }
        }
    }
    if pruned {
        let _ = persist_or_log(state);
    }
    for (process, role) in targets {
        let cap_mb = manual_cap_for(
            role,
            configured_manual_cap(&state.settings, &process, role),
        );
        let current = state.active_caps.get(&process.pid).filter(|record| {
            record.creation == process.creation
                && record.manual
                && record.applied_maximum_mb == Some(cap_mb)
        });
        if current.is_some() {
            continue;
        }
        if let Err(error) = apply_manual_cap(state, &process, role, cap_mb) {
            log_event(
                state,
                format!("Manual limit skipped for PID {}: {error}", process.pid),
                "warn",
            );
        }
    }
}

pub(crate) fn should_manage_limits(settings: &Settings, window_state: &str) -> bool {
    match window_state {
        "background-visible" => settings.optimize_when_background,
        "minimized" => settings.optimize_when_minimized,
        "hidden" => settings.optimize_when_hidden,
        _ => false,
    }
}

pub(crate) fn background_delay_elapsed(since: u64, now: u64, delay_seconds: f64) -> bool {
    since > 0 && (now.saturating_sub(since) as f64) >= delay_seconds * 1000.0
}

/// Reads the live window list and records which window represents Spotify now.
/// Detection no longer depends on a remembered handle: the window manager is asked
/// on every pass, so hiding to the tray, a destroyed window and a recreated one are
/// all classified from current facts instead of from cached state.
pub(crate) fn observe_window_state(state: &mut AppState, is_foreground: bool) -> (String, String) {
    let pids: Vec<u32> = state.snapshot.processes.iter().map(|p| p.pid).collect();
    let evidence = |rule: &str, list: &[windows::WindowCandidate]| {
        format!(
            "{rule} candidates={} onScreen={} iconic={} foreground={}",
            list.len(),
            list.iter().filter(|window| window.on_screen).count(),
            list.iter().filter(|window| window.iconic).count(),
            is_foreground,
        )
    };
    if pids.is_empty() {
        state.root_pid = None;
        state.hwnd = 0;
        state.window_title.clear();
        return ("not-running".to_string(), evidence("no-processes", &[]));
    }
    let candidates = windows::window_candidates(&pids);
    if candidates.is_empty() {
        return ("window-not-found".to_string(), evidence("no-titled-window", &[]));
    }
    // Helper processes can own titled windows too; the Spotify main process decides
    // the state whenever one of its windows is in the list.
    let root_pids: Vec<u32> = state
        .snapshot
        .processes
        .iter()
        .filter(|process| !is_child_process_command(&process.cmd))
        .map(|process| process.pid)
        .collect();
    let preferred: Vec<windows::WindowCandidate> = candidates
        .iter()
        .filter(|window| root_pids.contains(&window.pid))
        .cloned()
        .collect();
    let pool: &[windows::WindowCandidate] = if preferred.is_empty() { &candidates } else { &preferred };
    let (name, chosen) = windows::classify_windows(pool, is_foreground);
    let rule = match name {
        "foreground" => "foreground-window",
        "background-visible" => "visible-window",
        "minimized" => "iconic-window",
        _ => "window-off-screen",
    };
    let reason = evidence(rule, pool);
    if let Some(window) = chosen {
        state.hwnd = window.hwnd;
        state.root_pid = Some(window.pid);
        state.window_title = window.title.clone();
    }
    (name.to_string(), reason)
}

pub(crate) fn window_state_is_unknown(window_state: &str) -> bool {
    matches!(
        window_state,
        "window-not-found" | "not-detected" | "window-check-error"
    )
}

pub(crate) fn maybe_auto_optimize(state: &mut AppState, now: u64) {
    if !state.settings.enabled {
        state.hidden_since_ms = 0;
        if !state.active_caps.is_empty()
            && now.saturating_sub(state.last_restore_attempt_at_ms) >= 1500
        {
            state.last_restore_attempt_at_ms = now;
            let (_, failed) = restore_all(state, "plugin disabled");
            if failed > 0 {
                log_event(
                    state,
                    format!(
                        "Could not restore {failed} process limit(s) while the plugin is disabled."
                    ),
                    "warn",
                );
            }
        }
        return;
    }
    if !state.settings.auto_optimize {
        // An unreadable window state says nothing about eligibility. Restoring here
        // would drop limits during the instant a tray hide is destroying the window,
        // and the wait would restart on the next sample instead of finishing.
        if window_state_is_unknown(&state.window_state) {
            return;
        }
        if should_manage_limits(&state.settings, &state.window_state) {
            if state.hidden_since_ms == 0 {
                state.hidden_since_ms = now;
                log_event(
                    state,
                    format!(
                        "Spotify entered {}; manual limits wait {} second(s).",
                        state.window_state, state.settings.hidden_delay_seconds
                    ),
                    "info",
                );
            }
            if !background_delay_elapsed(
                state.hidden_since_ms,
                now,
                state.settings.hidden_delay_seconds,
            ) {
                return;
            }
            reconcile_manual_caps(state);
        } else {
            state.hidden_since_ms = 0;
            if !state.active_caps.is_empty()
                && now.saturating_sub(state.last_restore_attempt_at_ms) >= 1500
            {
                state.last_restore_attempt_at_ms = now;
                restore_all(
                    state,
                    &format!("manual policy window {}", state.window_state),
                );
            }
        }
        return;
    }
    if window_state_is_unknown(&state.window_state) {
        return;
    }
    let hidden = should_manage_limits(&state.settings, &state.window_state);
    if !hidden {
        state.hidden_since_ms = 0;
        if !state.active_caps.is_empty()
            && now.saturating_sub(state.last_restore_attempt_at_ms) >= 1500
        {
            state.last_restore_attempt_at_ms = now;
            restore_all(state, &format!("window {}", state.window_state));
        }
        return;
    }

    if state.hidden_since_ms == 0 {
        state.hidden_since_ms = now;
        log_event(
            state,
            format!(
                "Spotify entered {}; optimization waits {} second(s).",
                state.window_state, state.settings.hidden_delay_seconds
            ),
            "info",
        );
    }
    if !background_delay_elapsed(
        state.hidden_since_ms,
        now,
        state.settings.hidden_delay_seconds,
    ) {
        return;
    }
    if !state.snapshot_ok
        || state.snapshot_at_ms == 0
        || now.saturating_sub(state.snapshot_at_ms) > SNAPSHOT_STALE_MS
    {
        return;
    }
    let targets = descendants(state);
    let wanted: BTreeSet<(u32, u64)> = targets
        .iter()
        .map(|(p, _)| target_key(p.pid, p.creation))
        .collect();
    let no_longer_eligible: Vec<BackupRecord> = state
        .active_caps
        .values()
        .filter(|record| !wanted.contains(&target_key(record.pid, record.creation)))
        .cloned()
        .collect();
    if !no_longer_eligible.is_empty() {
        for record in no_longer_eligible {
            let outcome = windows::restore_working_set_limits(
                record.pid,
                record.creation,
                record.minimum,
                record.maximum,
                record.flags,
            );
            match outcome {
                Ok(_) => {}
                Err(error) => log_event(
                    state,
                    format!(
                        "Dropped no-longer-eligible PID {}: {error}",
                        record.pid
                    ),
                    "warn",
                ),
            }
            // A record outside the live target set can never be acted on again; remove it so it is
            // not retried every poll (a dead/PID-reused process cannot have its limits restored).
            state.active_caps.remove(&record.pid);
            state.profiles.remove(&record.pid);
        }
        let _ = persist_or_log(state);
    }
    let mut applied = 0usize;
    let mut failures = Vec::new();
    for (process, role) in targets {
        let has_record =
            state.active_caps.get(&process.pid).map(|r| r.creation) == Some(process.creation);
        // In Custom mode an exact identity pin is a hard ceiling that outranks the adaptive engine.
        // Resolved every poll, so editing or clearing a pin takes effect live without a restart.
        let pin = state
            .settings
            .custom_mode
            .then(|| identity_pin_cap(&state.settings, &process, role))
            .flatten();
        if let Some(cap) = pin {
            // apply_manual_cap preserves the original limits even when converting an existing
            // adaptive record, and is idempotent once that exact cap is already applied.
            let already = state
                .active_caps
                .get(&process.pid)
                .map(|r| r.creation == process.creation && r.manual && r.applied_maximum_mb == Some(cap))
                .unwrap_or(false);
            if already {
                continue;
            }
            match apply_manual_cap(state, &process, role, cap) {
                Ok(()) => {
                    if !has_record {
                        applied += 1;
                    }
                }
                Err(error) => failures.push(format!("PID {}: {error}", process.pid)),
            }
            continue;
        }

        // Adaptive path. A process that carries a live record either keeps adapting (ensure the
        // profile) or, if it is a manual record with no pin behind it any more, was just released
        // from a fixed ceiling: restore it so the next pass re-caps from the learned baseline
        // instead of leaving it pinned.
        if has_record {
            let was_manual = state
                .active_caps
                .get(&process.pid)
                .map(|r| r.manual)
                .unwrap_or(false);
            if was_manual {
                let record = state
                    .active_caps
                    .get(&process.pid)
                    .cloned()
                    .expect("record present");
                let _ = restore_one_record(state, &record, "manual cap released");
            } else {
                ensure_profile(state, process.pid, process.creation, role, now);
                continue;
            }
        }
        let retry_key = target_key(process.pid, process.creation);
        if state
            .target_retry_at_ms
            .get(&retry_key)
            .copied()
            .unwrap_or(0)
            > now
        {
            continue;
        }
        state
            .target_retry_at_ms
            .insert(retry_key, now + TARGET_RETRY_DELAY_MS);
        match apply_initial_cap(state, &process, role, now) {
            Ok(true) => {
                state.target_retry_at_ms.remove(&retry_key);
                applied += 1;
            }
            Ok(false) => {
                state.target_retry_at_ms.remove(&retry_key);
            }
            Err(error) => failures.push(format!("PID {}: {error}", process.pid)),
        }
    }
    update_caps(state, now_ms());
    if applied > 0 || !failures.is_empty() {
        state.last_action = Some(LastAction {
            time: iso_now(),
            reason: "automatic".to_string(),
            applied,
            failures: failures.clone(),
        });
        for failure in failures.iter().take(6) {
            log_event(state, failure.clone(), "warn");
        }
    }
}

pub(crate) fn refresh_memory(state: &mut AppState, now: u64) {
    let previous = state.last_memory_sample_at_ms;
    let interval = if previous > 0 {
        Some(now.saturating_sub(previous))
    } else {
        None
    };
    let mut fault_delta = 0u64;
    let mut fault_rate = 0.0f64;
    let mut measured = 0u64;
    for process in &mut state.snapshot.processes {
        if let Ok(stats) = windows::inspect_process(process.pid) {
            if stats.creation == process.creation {
                process.working_set_mb = Some(stats.working_set_mb);
                // The dashboard shows the private working set whenever Windows provides it, so the
                // native fast refresh must update that field too - not just working_set_mb, or the
                // "Atualizacao" slider would only move RAM numbers when a discovery fallback landed.
                process.private_working_set_mb = stats.private_working_set_mb;
                // Native CPU on the cheap memory cadence: the percentage divides the cpu-time delta by
                // the exact age of the shared baseline (now - sampled at), not by this loop's nominal
                // interval, and both writers refresh it - so the snapshot path and this one can never
                // dilute each other's windows. GetProcessTimes and the WMI CPU fields share the same
                // 100 ns -> ms basis.
                let key = target_key(process.pid, process.creation);
                if let Some((previous_cpu, previous_at)) = state.cpu_baselines.get(&key).copied() {
                    let elapsed = now.saturating_sub(previous_at);
                    if elapsed > 0 {
                        if let Some(percent) =
                            estimate_cpu_percent(previous_cpu, stats.cpu_time_ms as u64, elapsed)
                        {
                            process.cpu_percent = Some((percent * 10.0).round() / 10.0);
                        }
                    }
                }
                state
                    .cpu_baselines
                    .insert(key, (stats.cpu_time_ms as u64, now));
                process.cpu_time_ms = stats.cpu_time_ms;
                if let Some((previous_faults, previous_at)) =
                    state.page_fault_baselines.get(&key).copied()
                {
                    let delta = faults_since(previous_faults, stats.page_fault_count);
                    process.page_faults_delta = Some(delta);
                    if let Some(rate) = estimate_faults_per_second(
                        previous_faults,
                        stats.page_fault_count,
                        now.saturating_sub(previous_at),
                    ) {
                        process.page_faults_per_sec = Some(rate);
                        fault_rate += rate;
                    }
                    fault_delta = fault_delta.saturating_add(delta);
                    measured += 1;
                }
                process.page_fault_count = Some(stats.page_fault_count);
                process.error_code = 0;
                state
                    .page_fault_baselines
                    .insert(key, (stats.page_fault_count, now));
            }
        }
    }
    if let Some(interval_ms) = interval.filter(|_| measured > 0) {
        state.telemetry.page_fault_delta = Some(fault_delta);
        // Aggregate of each row's own rate, so rows whose baseline a snapshot advanced are still
        // counted over their real window instead of the refresh loop's nominal one.
        state.telemetry.page_faults_per_second = Some(fault_rate);
        state.telemetry.sample_interval_ms = Some(interval_ms);
    }
    state.last_memory_sample_at_ms = now;
    // Clear fault baselines only for process identities no longer present.
    let active_keys: BTreeSet<(u32, u64)> = state
        .snapshot
        .processes
        .iter()
        .map(|p| target_key(p.pid, p.creation))
        .collect();
    state
        .page_fault_baselines
        .retain(|key, _| active_keys.contains(key));
}

pub(crate) fn start_background_copy() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Write the monitor's output into the data directory, never the program directory: the
    // installer owns the program folder and removes it on uninstall, so runtime files there would
    // leave the folder behind.
    let log_dir = data_dir()?.join("logs");
    fs::create_dir_all(&log_dir)
        .map_err(|e| format!("Could not create startup log directory: {e}"))?;
    let stdout_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("startup.log"))
        .map_err(|e| format!("Could not open startup log: {e}"))?;
    let stderr_log = stdout_log
        .try_clone()
        .map_err(|e| format!("Could not duplicate startup log handle: {e}"))?;
    Command::new(exe)
        .arg("--monitor")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start the server: {e}"))
}

pub(crate) fn append_startup_log(path: &Path, message: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("Could not determine startup log directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Could not create startup log directory {}: {error}",
            parent.display()
        )
    })?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Could not open startup log {}: {error}", path.display()))?;
    writeln!(file, "[{}] [LAUNCHER] {}", iso_now(), message)
        .map_err(|error| format!("Could not write startup log {}: {error}", path.display()))?;
    Ok(())
}

pub fn launch_monitor() -> Result<(), String> {
    let base = data_dir()?;
    let startup_path = base.join("logs").join("startup.log");

    // Create and write the launch log before reading settings. This records failures
    // such as a missing/corrupt settings file that occur before the detached monitor runs.
    append_startup_log(&startup_path, "Launcher invoked.")?;

    let settings_path = base.join("settings.json");
    let settings = match load_settings(&settings_path) {
        Ok(settings) => settings,
        Err(error) => {
            let detail = format!(
                "Settings at {} were unusable ({error}); booting with fresh defaults",
                settings_path.display()
            );
            let _ = append_startup_log(&startup_path, &detail);
            match bootstrap_default_settings(&settings_path) {
                Ok(settings) => settings,
                Err(recovery_error) => {
                    return Err(format!(
                        "{detail}, but recovery failed: {recovery_error}. Diagnostic log: {}",
                        startup_path.display()
                    ));
                }
            }
        }
    };

    // The monitor might already be running after login or a previous launch attempt.
    if request_status(settings.port, &settings.token).is_ok() {
        let _ = append_startup_log(
            &startup_path,
            "An existing server instance passed the local API health check.",
        );
        return Ok(());
    }

    // Do not let the detached monitor inherit our stdout/stderr: when this runs inside a waiting
    // installer custom action, an inherited output pipe would keep the action from ever returning.
    windows::detach_stdio_from_children();
    if let Err(error) = start_background_copy() {
        let detail = format!("Could not start the detached monitor: {error}");
        let _ = append_startup_log(&startup_path, &detail);
        return Err(format!(
            "{detail}. Diagnostic log: {}",
            startup_path.display()
        ));
    }
    let _ = append_startup_log(
        &startup_path,
        "Detached monitor process spawned; waiting for the local API.",
    );

    let mut last_error = String::from("The server did not respond before the timeout.");
    for _ in 0..50 {
        match request_status(settings.port, &settings.token) {
            Ok(()) => {
                let _ = append_startup_log(&startup_path, "Local API health check succeeded.");
                return Ok(());
            }
            Err(error) => last_error = error,
        }
        thread::sleep(Duration::from_millis(100));
    }

    let _ = append_startup_log(
        &startup_path,
        &format!("Readiness timeout. Last health-check error: {last_error}"),
    );
    let startup_detail = fs::read_to_string(&startup_path).unwrap_or_default();
    let startup_tail = startup_detail
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    if startup_tail.is_empty() {
        Err(format!(
            "Memory Sentry failed to become ready after 5 seconds: {last_error}. See {}",
            startup_path.display()
        ))
    } else {
        Err(format!("Memory Sentry failed to become ready after 5 seconds: {last_error}. Startup log: {startup_tail}"))
    }
}
