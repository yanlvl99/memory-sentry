//! Windows server runtime in Rust: adaptive working-set management, recovery, and a loopback-only API.
//! The UI contract intentionally mirrors the legacy Node.js server so the Custom App stays usable.

use memory_sentry_windows as windows;
use memory_sentry_core::{
    pressure_level, ProcessRole, HIGH_CPU_PERCENT, MAX_CAP_MB, MIN_CAP_MB,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::File;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

mod Setup;
mod Store;
mod Api;
mod Monitor;

// Re-export the CLI-time entry points so the `Runtime::` command dispatch in main.rs keeps working
// now that the install/upgrade operations live in the Setup submodule.
pub use Setup::{
    check_supported_os, configure_install, pre_uninstall, restore_limits_command, swap_server,
};

// The persistence helpers are shared by every other module (and the parent itself), so re-export
// them into the `Runtime` namespace where descendants can reach them through `use super::*;`.
pub(crate) use Store::*;

// The loopback API is invoked from the monitor loop (start_api) and the launcher (request_status),
// so its entry points are shared back into the `Runtime` namespace.
pub(crate) use Api::*;

// The monitor loop and its helpers are driven from here and from the API handlers, so re-export
// them into the `Runtime` namespace as well.
pub(crate) use Monitor::*;

const VERSION: &str = "1.0.0-rc.6";
const DEFAULT_PORT: u16 = 38472;
const DEFAULT_HIDDEN_DELAY_SECONDS: f64 = 2.0;
const DEFAULT_REFRESH_INTERVAL_MS: u64 = 700;
const MIN_MANUAL_CAP_MB: u32 = 8;
const MAX_MANUAL_CAP_MB: u32 = 512;
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 32 * 1024;
const SNAPSHOT_INTERVAL_MS: u64 = 2500;
const SNAPSHOT_STALE_MS: u64 = 6500;
const MEMORY_SAMPLE_INTERVAL_MS: u64 = 2000;
// "Watched" boost: while the dashboard is actively polling /api/status (the Memory Sentry screen is
// open and visible), sample per-process memory far more often and refresh the process list a bit
// sooner so the numbers feel live. When polling stops (tab hidden or closed) the loop falls back to
// the economical cadences above. The status handler stamps last_status_request_ms on every read; a
// read within WATCH_STALE_MS counts as "watched". Both signals are in-process Win32 calls now, so the
// boosted cadences cost microseconds per pass and never spawn a helper process.
const WATCH_STALE_MS: u64 = 2500;
const WATCH_SNAPSHOT_MS: u64 = 1300;
const FOCUS_POLL_INTERVAL_MS: u64 = 100;
const TARGET_RETRY_DELAY_MS: u64 = 2500;
const TRIM_COOLDOWN_MS: u64 = 15_000;
const MINIMUM_BACKUP_HEADER: &str = "# Memory Sentry recovery records v1";
const APP_NAME: &str = "MemorySentryForSpicetify";
const LEGACY_APP_NAME: &str = "SpicetifyMemorySentry";
const CURRENT_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    port: u16,
    token: String,
    enabled: bool,
    auto_optimize: bool,
    custom_mode: bool,
    hidden_delay_seconds: f64,
    refresh_interval_ms: u64,
    trim_to_cap: bool,
    optimize_when_background: bool,
    optimize_when_minimized: bool,
    optimize_when_hidden: bool,
    manual_role_caps: BTreeMap<String, u32>,
    auto_role_caps: BTreeMap<String, u32>,
    // Cross-session local usage telemetry: a per-identity busy-fraction score (0..=100) kept as an
    // EWMA so a rarely-used identity is already treated as Rare in the first minutes after launch,
    // before this session has its own samples. Keyed by process_identity (role + sub-label), NEVER a
    // PID. Local-only, never uploaded (see PRIVACY.md).
    usage_scores: BTreeMap<String, u32>,
    // Learned safe edge per identity: the lowest automatic ceiling that did not make the process
    // thrash. A thrash-relief increase records the new ceiling here, and the engine never descends
    // below it again, which is what prevents an 8 -> 16 -> 8 oscillation once the per-role floors are
    // gone. Keyed by process_identity, NEVER a PID. Local-only.
    safe_floors: BTreeMap<String, u32>,
    schema_version: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct BackupRecord {
    pid: u32,
    creation: u64,
    role: String,
    minimum: usize,
    maximum: usize,
    flags: u32,
    applied_maximum_mb: Option<u32>,
    applied_at: String,
    manual: bool,
}

impl BackupRecord {
    fn capture_original(
        pid: u32,
        creation: u64,
        role: ProcessRole,
        limits: windows::OriginalLimits,
        manual: bool,
    ) -> Self {
        Self {
            pid,
            creation,
            role: role_name(role).to_string(),
            minimum: limits.minimum,
            maximum: limits.maximum,
            flags: limits.flags,
            applied_maximum_mb: None,
            applied_at: iso_now(),
            manual,
        }
    }

    fn record_applied_cap(&mut self, role: ProcessRole, cap_mb: u32, manual: bool) {
        self.role = role_name(role).to_string();
        self.applied_maximum_mb = Some(cap_mb);
        self.applied_at = iso_now();
        self.manual = manual;
    }
}

#[derive(Clone, Debug)]
struct AdaptiveProfile {
    creation: u64,
    current_cap_mb: u32,
    high_cpu_ms: u64,
    low_cpu_ms: u64,
    last_evaluated_snapshot_at_ms: u64,
    last_adjustment_at_ms: u64,
    last_reason: String,
    last_cpu_percent: Option<f64>,
    last_working_set_mb: Option<f64>,
    last_trim_at_ms: u64,
}

#[derive(Clone, Debug)]
struct Event {
    time: String,
    level: String,
    message: String,
}

#[derive(Clone, Debug)]
struct LastAction {
    time: String,
    reason: String,
    applied: usize,
    failures: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct Telemetry {
    last_restore_duration_ms: Option<f64>,
    last_restore_at: Option<String>,
    last_restore_restored: u32,
    last_restore_failures: u32,
    page_fault_delta: Option<u64>,
    page_faults_per_second: Option<f64>,
    sample_interval_ms: Option<u64>,
    adaptive_adjustments: u64,
    last_adaptive_adjustment_at: Option<String>,
}

pub(crate) struct AppState {
    settings_path: PathBuf,
    backup_path: PathBuf,
    log_path: PathBuf,
    settings: Settings,
    started_at: String,
    snapshot: windows::Snapshot,
    snapshot_ok: bool,
    snapshot_at_ms: u64,
    snapshot_serial: u64,
    root_pid: Option<u32>,
    hwnd: usize,
    window_title: String,
    window_state: String,
    window_state_reason: String,
    hidden_since_ms: u64,
    last_restore_attempt_at_ms: u64,
    last_memory_sample_at_ms: u64,
    last_status_request_ms: u64,
    active_caps: BTreeMap<u32, BackupRecord>,
    profiles: BTreeMap<u32, AdaptiveProfile>,
    target_retry_at_ms: BTreeMap<(u32, u64), u64>,
    page_fault_baselines: BTreeMap<(u32, u64), (u64, u64)>,
    cpu_baselines: BTreeMap<(u32, u64), (u64, u64)>,
    audio_pids: BTreeSet<u32>,
    audio_pids_error: String,
    telemetry: Telemetry,
    recent_events: VecDeque<Event>,
    last_error: String,
    last_action: Option<LastAction>,
    shutdown_requested: bool,
}

pub(crate) type SharedState = Arc<Mutex<AppState>>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn iso_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    iso_from_unix_seconds(seconds)
}

// Gregorian date conversion, avoiding a runtime dependency for timestamp formatting.
fn iso_from_unix_seconds(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn appdata_root() -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    Ok(PathBuf::from(local))
}

fn install_dir() -> Result<PathBuf, String> {
    Ok(appdata_root()?.join("SpicetifyMemorySentry"))
}

/// Data directory (settings, backup, pid, logs), separate from the program directory.
fn data_dir() -> Result<PathBuf, String> {
    Ok(appdata_root()?.join("MemorySentryForSpicetify"))
}

/// Program directory (the executable and its bundled docs) used by the MSI install. The MSI
/// removes the previous product on upgrade, so the program lives in its own stable folder under
/// Programs, separate from the user data directory.
fn program_dir() -> Result<PathBuf, String> {
    Ok(appdata_root()?.join("Programs").join("MemorySentryForSpicetify"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn foreground_child_process_is_recognized_as_spotify() {
        let spotify_pids = [42, 84, 126];
        assert!(spotify_process_is_foreground(Some(84), spotify_pids));
        assert!(!spotify_process_is_foreground(Some(7), spotify_pids));
        assert!(!spotify_process_is_foreground(None, spotify_pids));
    }

    #[test]
    fn renderer_and_utility_sub_labels_disambiguate_same_role_processes() {
        let renderer_a = "spotify.exe --type=renderer --renderer-client-id=7".to_string();
        let renderer_b = "spotify.exe --type=renderer --renderer-client-id=3".to_string();
        assert_eq!(process_sub_label(&renderer_a, ProcessRole::Renderer).as_deref(), Some("#7"));
        assert_eq!(process_sub_label(&renderer_b, ProcessRole::Renderer).as_deref(), Some("#3"));
        let utility = "spotify.exe --type=utility --utility-sub-type=network.mojom.NetworkService".to_string();
        assert_eq!(process_sub_label(&utility, ProcessRole::Utility).as_deref(), Some("network"));
        assert_eq!(process_sub_label("spotify.exe --type=renderer", ProcessRole::Renderer), None);
        assert_eq!(process_sub_label("spotify.exe", ProcessRole::Main), None);
    }

    #[test]
    fn caps_are_keyed_by_identity_with_role_default_fallback() {
        assert_eq!(process_identity("spotify.exe --type=renderer --renderer-client-id=7", ProcessRole::Renderer), "Renderer:#7");
        assert_eq!(process_identity("spotify.exe --type=utility --utility-sub-type=network.mojom.NetworkService", ProcessRole::Utility), "Utility:network");
        assert_eq!(process_identity("spotify.exe", ProcessRole::Main), "Main");
        let row = |pid: u32, cmd: &str| windows::ProcessRow {
            pid,
            ppid: 1,
            cmd: cmd.to_string(),
            creation: 100,
            cpu_time_ms: 0.0,
            cpu_percent: None,
            working_set_mb: None,
            private_working_set_mb: None,
            private_mb: None,
            page_fault_count: None,
            page_faults_delta: None,
            page_faults_per_sec: None,
            error_code: 0,
        };
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: false,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::from([
                ("Renderer".to_string(), 40),
                ("Renderer:#7".to_string(), 80),
            ]),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
        };
        // The individually-pinned renderer keeps its own cap; a sibling falls back to the role default.
        let pinned = row(11, "spotify.exe --type=renderer --renderer-client-id=7");
        let sibling = row(12, "spotify.exe --type=renderer --renderer-client-id=3");
        assert_eq!(configured_manual_cap(&settings, &pinned, ProcessRole::Renderer), Some(80));
        assert_eq!(configured_manual_cap(&settings, &sibling, ProcessRole::Renderer), Some(40));
    }

    #[test]
    fn custom_pin_requires_exact_identity_match() {
        let row = |cmd: &str| windows::ProcessRow {
            pid: 1,
            ppid: 1,
            cmd: cmd.to_string(),
            creation: 100,
            cpu_time_ms: 0.0,
            cpu_percent: None,
            working_set_mb: None,
            private_working_set_mb: None,
            private_mb: None,
            page_fault_count: None,
            page_faults_delta: None,
            page_faults_per_sec: None,
            error_code: 0,
        };
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: true,
            custom_mode: true,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::from([("Renderer".to_string(), 40), ("Renderer:#7".to_string(), 90)]),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
        };
        // Only the exact identity is a pin; a bare-role default must not pin every sibling.
        assert_eq!(identity_pin_cap(&settings, &row("spotify.exe --type=renderer --renderer-client-id=7"), ProcessRole::Renderer), Some(90));
        assert_eq!(identity_pin_cap(&settings, &row("spotify.exe --type=renderer --renderer-client-id=3"), ProcessRole::Renderer), None);
    }

    #[test]
    fn settings_write_and_load_round_trip_the_schema_version() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mss-schema-{}-{}.json", std::process::id(), unique));
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "0".repeat(32),
            enabled: true,
            auto_optimize: true,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::new(),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
        };
        write_settings(&path, &settings).expect("write settings");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"schemaVersion\""));
        let loaded = load_settings(&path).expect("load settings");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn updating_caps_preserves_the_first_captured_original_limits() {
        let originals = windows::OriginalLimits {
            minimum: 1024,
            maximum: 8 * 1024 * 1024,
            flags: 0x1234,
        };
        let mut record =
            BackupRecord::capture_original(42, 100, ProcessRole::Main, originals, false);

        record.record_applied_cap(ProcessRole::Main, 36, false);
        record.record_applied_cap(ProcessRole::Main, 64, false);

        assert_eq!(record.minimum, originals.minimum);
        assert_eq!(record.maximum, originals.maximum);
        assert_eq!(record.flags, originals.flags);
        assert_eq!(record.applied_maximum_mb, Some(64));
    }

    #[test]
    fn manual_policy_uses_role_defaults_and_honors_saved_overrides() {
        assert_eq!(manual_cap_for(ProcessRole::Main, None), 36);
        assert_eq!(manual_cap_for(ProcessRole::Audio, None), 36);
        assert_eq!(manual_cap_for(ProcessRole::Gpu, None), 28);
        assert_eq!(manual_cap_for(ProcessRole::Renderer, None), 28);
        assert_eq!(manual_cap_for(ProcessRole::Utility, None), 24);
        assert_eq!(manual_cap_for(ProcessRole::Helper, None), 24);
        assert_eq!(manual_cap_for(ProcessRole::Crashpad, None), 16);
        assert_eq!(manual_cap_for(ProcessRole::Renderer, Some(40)), 40);
    }

    #[test]
    fn background_delay_is_shared_by_automatic_and_manual_management() {
        assert!(!background_delay_elapsed(0, 10_000, 2.0));
        assert!(!background_delay_elapsed(10_000, 11_999, 2.0));
        assert!(background_delay_elapsed(10_000, 12_000, 2.0));
        assert!(background_delay_elapsed(10_000, 15_000, 2.0));
        // Sub-second delays are now supported (the dashboard exposes a half-step slider).
        assert!(!background_delay_elapsed(10_000, 10_499, 0.5));
        assert!(background_delay_elapsed(10_000, 10_500, 0.5));
    }

    #[test]
    fn managed_limits_are_active_only_outside_the_foreground() {
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: true,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::new(),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
        schema_version: CURRENT_SCHEMA_VERSION,
        };
        assert!(should_manage_limits(&settings, "minimized"));
        assert!(should_manage_limits(&settings, "hidden"));
        assert!(should_manage_limits(&settings, "background-visible"));
        assert!(!should_manage_limits(&settings, "foreground"));
        assert!(!should_manage_limits(&settings, "window-not-found"));
    }

    #[test]
    fn an_unreadable_window_state_never_restarts_the_background_wait() {
        assert!(window_state_is_unknown("window-not-found"));
        assert!(window_state_is_unknown("not-detected"));
        assert!(!window_state_is_unknown("hidden"));
        assert!(!window_state_is_unknown("minimized"));
        assert!(!window_state_is_unknown("foreground"));

        let base = std::env::temp_dir().join(format!(
            "mss-unknown-window-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: true,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: false,
            optimize_when_minimized: false,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::new(),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
        schema_version: CURRENT_SCHEMA_VERSION,
        };
        let mut state = build_state(base.clone(), settings);
        state.window_state = "window-not-found".to_string();
        state.hidden_since_ms = 10_000;
        maybe_auto_optimize(&mut state, 12_000);
        assert_eq!(state.hidden_since_ms, 10_000);

        state.settings.auto_optimize = false;
        maybe_auto_optimize(&mut state, 13_000);
        assert_eq!(state.hidden_since_ms, 10_000);

        state.window_state = "foreground".to_string();
        maybe_auto_optimize(&mut state, 14_000);
        assert_eq!(state.hidden_since_ms, 0);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn each_trigger_state_can_be_disabled_but_never_all_three() {
        let base = std::env::temp_dir().join(format!(
            "mss-triggers-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: true,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::new(),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
        schema_version: CURRENT_SCHEMA_VERSION,
        };
        let mut state = build_state(base.clone(), settings);
        update_config(
            &mut state,
            r#"{"optimizeWhenMinimized":false,"optimizeWhenHidden":false}"#,
        )
        .expect("partial trigger change should save");
        assert!(!state.settings.optimize_when_minimized);
        assert!(!state.settings.optimize_when_hidden);
        assert!(state.settings.optimize_when_background);
        update_config(&mut state, r#"{"optimizeWhenBackground":false}"#)
            .expect("an all-off trigger set should be coerced");
        assert!(state.settings.optimize_when_background);
        let loaded = load_settings(&state.settings_path).expect("trigger states should persist");
        assert!(loaded.optimize_when_background);
        assert!(!loaded.optimize_when_minimized);
        assert!(!loaded.optimize_when_hidden);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn role_cap_parser_keeps_only_known_roles_and_valid_limits() {
        let serialized = r#"{"manualRoleCaps":["Renderer:52","Main:7","Unknown:40","Audio:512"]}"#;
        let caps = parse_manual_role_caps(serialized);
        assert_eq!(caps.len(), 2);
        assert_eq!(caps.get("Renderer"), Some(&52));
        assert_eq!(caps.get("Audio"), Some(&512));
    }

    #[test]
    fn usage_score_parser_keeps_known_identities_and_valid_scores() {
        // Identity may itself contain ':' (e.g. Renderer:#7), so the score is the LAST segment.
        let serialized = r#"{"usageScores":["Main:65","Renderer:#3:12","Unknown:40","GPU:101","Audio:notanumber","Utility:0"]}"#;
        let scores = parse_usage_scores(serialized);
        assert_eq!(scores.get("Main"), Some(&65));
        assert_eq!(scores.get("Renderer:#3"), Some(&12));
        assert_eq!(scores.get("Utility"), Some(&0));
        // Unknown role, a score over 100, and a non-numeric score are all dropped.
        assert!(!scores.contains_key("Unknown"));
        assert!(!scores.contains_key("GPU"));
        assert!(!scores.contains_key("Audio"));
        assert_eq!(scores.len(), 3);
    }

    #[test]
    fn safe_floor_parser_keeps_known_identities_and_valid_caps() {
        // Identity may contain ':' (e.g. Renderer:#7), so the cap is the LAST segment. Values below
        // the absolute floor or above the automatic cap range are dropped.
        let serialized = r#"{"safeFloors":["Main:12","Renderer:#3:20","Unknown:40","GPU:3","Audio:65","Utility:notanumber"]}"#;
        let floors = parse_safe_floors(serialized);
        assert_eq!(floors.get("Main"), Some(&12));
        assert_eq!(floors.get("Renderer:#3"), Some(&20));
        assert!(!floors.contains_key("Unknown"));
        assert!(!floors.contains_key("GPU"));
        assert!(!floors.contains_key("Audio"));
        assert!(!floors.contains_key("Utility"));
        assert_eq!(floors.len(), 2);
    }

    #[test]
    fn role_override_is_reused_when_a_new_pid_replaces_an_old_process() {
        let mut settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: false,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::from([("Renderer".to_string(), 52)]),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
        };
        let restarted_process = windows::ProcessRow {
            pid: 99,
            ppid: 1,
            cmd: "spotify.exe --type=renderer".to_string(),
            creation: 200,
            cpu_time_ms: 0.0,
            cpu_percent: None,
            working_set_mb: None,
            private_working_set_mb: None,
            private_mb: None,
            page_fault_count: None,
            page_faults_delta: None,
            page_faults_per_sec: None,
            error_code: 0,
        };

        assert_eq!(
            configured_manual_cap(&settings, &restarted_process, ProcessRole::Renderer),
            Some(52)
        );
        settings.manual_role_caps.clear();
        assert_eq!(
            configured_manual_cap(&settings, &restarted_process, ProcessRole::Renderer),
            None
        );
    }

    #[test]
    fn capmb_number_parser_requires_whole_positive_values() {
        assert_eq!(parse_json_number(r#"{"capMB":36}"#, "capMB"), Some(36));
        assert_eq!(parse_json_number(r#"{"capMB":36.5}"#, "capMB"), None);
    }

    #[test]
    fn legacy_settings_default_the_global_toggle_to_enabled() {
        let path = std::env::temp_dir().join(format!(
            "mss-legacy-settings-{}-{}.json",
            std::process::id(),
            now_ms()
        ));
        let raw = format!(r#"{{"token":"{}"}}"#, "a".repeat(64));
        fs::write(&path, raw).expect("legacy settings should be written");
        let loaded = load_settings(&path).expect("legacy settings should load");
        assert!(loaded.enabled);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn global_toggle_disables_management_and_persists() {
        let base = std::env::temp_dir().join(format!(
            "mss-enabled-toggle-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: true,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::new(),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
        schema_version: CURRENT_SCHEMA_VERSION,
        };
        let mut state = build_state(base.clone(), settings);
        update_config(&mut state, r#"{"enabled":false}"#)
            .expect("plugin should disable with no active process caps");
        assert!(!state.settings.enabled);
        assert!(public_config(&state.settings).contains("\"enabled\":false"));
        let persisted =
            load_settings(&state.settings_path).expect("disabled setting should persist");
        assert!(!persisted.enabled);
        assert!(set_manual_cap(&mut state, "{}").is_err());
        assert!(clear_manual_cap(&mut state, "{}").is_err());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn settings_round_trip_preserves_manual_process_limits() {
        let path = std::env::temp_dir().join(format!(
            "mss-settings-{}-{}.json",
            std::process::id(),
            now_ms()
        ));
        let mut settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: false,
            auto_optimize: false,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
                refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::new(),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
        schema_version: CURRENT_SCHEMA_VERSION,
        };
        settings.manual_role_caps.insert("Renderer".to_string(), 52);
        settings.safe_floors.insert("Renderer:#7".to_string(), 20);
        write_settings(&path, &settings).expect("settings should be written");
        let loaded = load_settings(&path).expect("settings should load");
        assert_eq!(loaded.manual_role_caps.get("Renderer"), Some(&52));
        assert_eq!(loaded.safe_floors.get("Renderer:#7"), Some(&20));
        assert!(!loaded.enabled);
        assert!(!loaded.auto_optimize);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn status_payload_exposes_the_per_process_fault_rate() {
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: false,
            auto_optimize: false,
            custom_mode: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
            refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_role_caps: BTreeMap::new(),
            auto_role_caps: BTreeMap::new(),
            usage_scores: BTreeMap::new(),
            safe_floors: BTreeMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
        };
        let mut state = build_state(std::env::temp_dir().join("mss-status-payload-test"), settings);
        state.root_pid = Some(10);
        state.snapshot.processes.push(windows::ProcessRow {
            pid: 10,
            ppid: 1,
            cmd: "spotify.exe".to_string(),
            creation: 5,
            cpu_time_ms: 0.0,
            cpu_percent: Some(0.0),
            working_set_mb: Some(20.0),
            private_working_set_mb: None,
            private_mb: None,
            page_fault_count: Some(100),
            page_faults_delta: Some(30),
            page_faults_per_sec: Some(15.5),
            error_code: 0,
        });
        let payload = status_payload(&state);
        // The engine consumes the rate, not the delta, so the debug surface must report the same
        // number: this is what a real-machine calibration of the thrash threshold reads.
        assert!(
            payload.contains("\"pageFaultsPerSec\":15.500"),
            "payload must expose the per-process fault rate: {payload}"
        );
    }
}
