//! Windows server runtime in Rust: adaptive working-set management, recovery, and a loopback-only API.
//! The UI contract intentionally mirrors the legacy Node.js server so the Custom App stays usable.

use crate::platform;
use memory_sentry_core::{
    decide_adjustment, estimate_cpu_percent, working_set_at_ceiling, AdjustmentDirection,
    ProcessRole, ADJUSTMENT_COOLDOWN_MS, DECREASE_STEP_MB, HIGH_CPU_DURATION_MS, HIGH_CPU_PERCENT,
    INCREASE_STEP_MB, LOW_CPU_DURATION_MS, LOW_CPU_PERCENT, LOW_WORKING_SET_MARGIN_MB, MAX_CAP_MB,
    MIN_CAP_MB,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const VERSION: &str = "1.0.0-rc.3";
const DEFAULT_PORT: u16 = 38472;
const DEFAULT_HIDDEN_DELAY_SECONDS: u64 = 2;
const MIN_MANUAL_CAP_MB: u32 = 8;
const MAX_MANUAL_CAP_MB: u32 = 512;
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 32 * 1024;
const SNAPSHOT_INTERVAL_MS: u64 = 2500;
const SNAPSHOT_STALE_MS: u64 = 6500;
const MEMORY_SAMPLE_INTERVAL_MS: u64 = 2000;
const FOCUS_POLL_INTERVAL_MS: u64 = 100;
const TARGET_RETRY_DELAY_MS: u64 = 2500;
const TRIM_COOLDOWN_MS: u64 = 15_000;
const MINIMUM_BACKUP_HEADER: &str = "# Memory Sentry recovery records v1";
const APP_NAME: &str = "SpicetifyMemorySentry";

#[derive(Clone, Debug)]
struct Settings {
    port: u16,
    token: String,
    enabled: bool,
    auto_optimize: bool,
    hidden_delay_seconds: u64,
    trim_to_cap: bool,
    optimize_when_background: bool,
    optimize_when_minimized: bool,
    optimize_when_hidden: bool,
    manual_caps: BTreeMap<(u32, u64), u32>,
    manual_role_caps: BTreeMap<String, u32>,
}

#[derive(Clone, Debug)]
struct BackupRecord {
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
        limits: platform::OriginalLimits,
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

#[derive(Clone, Debug)]
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

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            last_restore_duration_ms: None,
            last_restore_at: None,
            last_restore_restored: 0,
            last_restore_failures: 0,
            page_fault_delta: None,
            page_faults_per_second: None,
            sample_interval_ms: None,
            adaptive_adjustments: 0,
            last_adaptive_adjustment_at: None,
        }
    }
}

struct AppState {
    install_dir: PathBuf,
    settings_path: PathBuf,
    backup_path: PathBuf,
    log_path: PathBuf,
    settings: Settings,
    started_at: String,
    snapshot: platform::Snapshot,
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
    active_caps: BTreeMap<u32, BackupRecord>,
    profiles: BTreeMap<u32, AdaptiveProfile>,
    target_retry_at_ms: BTreeMap<String, u64>,
    page_fault_baselines: BTreeMap<String, u64>,
    audio_pids: BTreeSet<u32>,
    audio_pids_error: String,
    telemetry: Telemetry,
    recent_events: VecDeque<Event>,
    last_error: String,
    last_action: Option<LastAction>,
    shutdown_requested: bool,
}

type SharedState = Arc<Mutex<AppState>>;

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

fn parse_json_string(text: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let pos = text.find(&marker)? + marker.len();
    let tail = text[pos..].trim_start();
    let tail = tail.strip_prefix(':')?.trim_start();
    let mut chars = tail.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            escaped = false;
        } else {
            match ch {
                '\\' => escaped = true,
                '"' => return Some(out),
                other => out.push(other),
            }
        }
    }
    None
}

fn parse_json_number(text: &str, key: &str) -> Option<u64> {
    let marker = format!("\"{key}\"");
    let pos = text.find(&marker)? + marker.len();
    let tail = text[pos..].trim_start().strip_prefix(':')?.trim_start();
    let end = tail
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(tail.len());
    if end == 0
        || tail[end..]
            .chars()
            .next()
            .map(|ch| !ch.is_ascii_whitespace() && !matches!(ch, ',' | '}' | ']'))
            .unwrap_or(false)
    {
        return None;
    }
    tail[..end].parse().ok()
}

fn parse_json_bool(text: &str, key: &str) -> Option<bool> {
    let marker = format!("\"{key}\"");
    let pos = text.find(&marker)? + marker.len();
    let tail = text[pos..].trim_start().strip_prefix(':')?.trim_start();
    if tail.starts_with("true") {
        Some(true)
    } else if tail.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_json_string_array(text: &str, key: &str) -> Option<Vec<String>> {
    let marker = format!("\"{key}\"");
    let pos = text.find(&marker)? + marker.len();
    let mut chars = text[pos..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .chars();
    if chars.next()? != '[' {
        return None;
    }
    let mut values = Vec::new();
    loop {
        while chars
            .clone()
            .next()
            .map(char::is_whitespace)
            .unwrap_or(false)
        {
            chars.next();
        }
        match chars.next()? {
            ']' => return Some(values),
            '"' => {
                let mut value = String::new();
                let mut escaped = false;
                loop {
                    let ch = chars.next()?;
                    if escaped {
                        value.push(match ch {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            other => other,
                        });
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        values.push(value);
                        break;
                    } else {
                        value.push(ch);
                    }
                }
                while chars
                    .clone()
                    .next()
                    .map(char::is_whitespace)
                    .unwrap_or(false)
                {
                    chars.next();
                }
                match chars.next()? {
                    ',' => {}
                    ']' => return Some(values),
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
}

fn parse_manual_caps(text: &str) -> BTreeMap<(u32, u64), u32> {
    parse_json_string_array(text, "manualCaps")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let mut parts = entry.split(':');
            let (Ok(pid), Ok(creation), Ok(cap_mb)) = (
                parts.next()?.parse::<u32>(),
                parts.next()?.parse::<u64>(),
                parts.next()?.parse::<u32>(),
            ) else {
                return None;
            };
            if parts.next().is_some()
                || pid == 0
                || creation == 0
                || !(MIN_MANUAL_CAP_MB..=MAX_MANUAL_CAP_MB).contains(&cap_mb)
            {
                return None;
            }
            Some(((pid, creation), cap_mb))
        })
        .collect()
}

fn parse_manual_role_caps(text: &str) -> BTreeMap<String, u32> {
    parse_json_string_array(text, "manualRoleCaps")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let (role, cap) = entry.split_once(':')?;
            let cap = cap.parse::<u32>().ok()?;
            if !matches!(role, "Main" | "GPU" | "Renderer" | "Utility" | "Crashpad" | "Helper" | "Audio")
                || !(MIN_MANUAL_CAP_MB..=MAX_MANUAL_CAP_MB).contains(&cap)
            {
                return None;
            }
            Some((role.to_string(), cap))
        })
        .collect()
}

fn load_settings(path: &Path) -> Result<Settings, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let token = parse_json_string(&raw, "token").ok_or("settings.json has no token")?;
    if token.len() < 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("settings.json token is invalid".to_string());
    }
    let port = parse_json_number(&raw, "port")
        .unwrap_or(DEFAULT_PORT as u64)
        .clamp(1, 65535) as u16;
    let enabled = parse_json_bool(&raw, "enabled").unwrap_or(true);
    let auto_optimize = parse_json_bool(&raw, "autoOptimize").unwrap_or(true);
    let hidden_delay_seconds = parse_json_number(&raw, "hiddenDelaySeconds")
        .unwrap_or(DEFAULT_HIDDEN_DELAY_SECONDS)
        .clamp(1, 30);
    let trim_to_cap = parse_json_bool(&raw, "trimToCap").unwrap_or(true);
    let optimize_when_background = parse_json_bool(&raw, "optimizeWhenBackground").unwrap_or(true);
    let optimize_when_minimized = parse_json_bool(&raw, "optimizeWhenMinimized").unwrap_or(true);
    let optimize_when_hidden = parse_json_bool(&raw, "optimizeWhenHidden").unwrap_or(true);
    let manual_caps = parse_manual_caps(&raw);
    let manual_role_caps = parse_manual_role_caps(&raw);
    Ok(Settings {
        port,
        token,
        enabled,
        auto_optimize,
        hidden_delay_seconds,
        trim_to_cap,
        optimize_when_background,
        optimize_when_minimized,
        optimize_when_hidden,
        manual_caps,
        manual_role_caps,
    })
}

fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("json.tmp");
    let manual_caps = settings
        .manual_caps
        .iter()
        .map(|((pid, creation), cap)| json_quote(&format!("{pid}:{creation}:{cap}")))
        .collect::<Vec<_>>()
        .join(", ");
    let manual_role_caps = settings
        .manual_role_caps
        .iter()
        .map(|(role, cap)| json_quote(&format!("{role}:{cap}")))
        .collect::<Vec<_>>()
        .join(", ");
    let contents = format!(
        "{{\n  \"port\": {},\n  \"token\": {},\n  \"enabled\": {},\n  \"autoOptimize\": {},\n  \"hiddenDelaySeconds\": {},\n  \"trimToCap\": {},\n  \"optimizeWhenBackground\": {},\n  \"optimizeWhenMinimized\": {},\n  \"optimizeWhenHidden\": {},\n  \"manualCaps\": [{}],\n  \"manualRoleCaps\": [{}],\n  \"version\": \"{}\"\n}}\n",
        settings.port, json_quote(&settings.token), settings.enabled, settings.auto_optimize, settings.hidden_delay_seconds, settings.trim_to_cap, settings.optimize_when_background, settings.optimize_when_minimized, settings.optimize_when_hidden, manual_caps, manual_role_caps, VERSION
    );
    let mut file = File::create(&temp).map_err(|e| format!("Could not stage settings: {e}"))?;
    file.write_all(contents.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    replace_file(&temp, path)?;
    Ok(())
}

fn replace_file(temp: &Path, target: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        if target.exists() {
            let previous = target.with_extension("previous");
            let _ = fs::remove_file(&previous);
            fs::rename(target, &previous)
                .map_err(|e| format!("Could not preserve previous file: {e}"))?;
            match fs::rename(temp, target) {
                Ok(()) => {
                    let _ = fs::remove_file(previous);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(previous, target);
                    Err(format!("Could not replace file: {error}"))
                }
            }
        } else {
            fs::rename(temp, target).map_err(|e| format!("Could not install staged file: {e}"))
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(temp, target).map_err(|e| format!("Could not replace file: {e}"))
    }
}

fn log_event(state: &mut AppState, message: impl Into<String>, level: &str) {
    let message = message.into();
    let event = Event {
        time: iso_now(),
        level: level.to_string(),
        message: message.clone(),
    };
    state.recent_events.push_back(event);
    while state.recent_events.len() > 14 {
        state.recent_events.pop_front();
    }
    if level.eq_ignore_ascii_case("error") {
        state.last_error = message.clone();
    }
    let line = format!(
        "[{}] [{}] {}\n",
        iso_now(),
        level.to_ascii_uppercase(),
        message
    );
    if let Some(parent) = state.log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::metadata(&state.log_path)
        .map(|m| m.len() > 512 * 1024)
        .unwrap_or(false)
    {
        if let Some(parent) = state.log_path.parent() {
            let old = parent.join(format!("helper-{}.previous.log", now_ms()));
            let _ = fs::rename(&state.log_path, old);
        }
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

fn load_backup_file(path: &Path) -> Vec<BackupRecord> {
    let candidates = [
        path.to_path_buf(),
        path.with_extension("previous"),
        path.with_extension("tsv.tmp"),
    ];
    let mut records = BTreeMap::<(u32, u64), BackupRecord>::new();
    for candidate in candidates {
        let Ok(raw) = fs::read_to_string(&candidate) else {
            continue;
        };
        for line in raw.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 8 {
                continue;
            }
            let (Ok(pid), Ok(creation), Ok(minimum), Ok(maximum), Ok(flags)) = (
                fields[0].parse::<u32>(),
                fields[1].parse::<u64>(),
                fields[3].parse::<usize>(),
                fields[4].parse::<usize>(),
                fields[5].parse::<u32>(),
            ) else {
                continue;
            };
            if pid == 0 || creation == 0 {
                continue;
            }
            let record = BackupRecord {
                pid,
                creation,
                role: fields[2].to_string(),
                minimum,
                maximum,
                flags,
                applied_maximum_mb: fields[6].parse::<u32>().ok(),
                applied_at: fields[7].to_string(),
                manual: fields
                    .get(8)
                    .map(|value| *value == "manual")
                    .unwrap_or(false),
            };
            records.insert((pid, creation), record);
        }
    }
    records.into_values().collect()
}

fn persist_backups(state: &AppState) -> Result<(), String> {
    if let Some(parent) = state.backup_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if state.active_caps.is_empty() {
        let _ = fs::remove_file(&state.backup_path);
        let _ = fs::remove_file(state.backup_path.with_extension("previous"));
        let _ = fs::remove_file(state.backup_path.with_extension("tsv.tmp"));
        return Ok(());
    }
    let temp = state.backup_path.with_extension("tsv.tmp");
    let mut contents = format!("{MINIMUM_BACKUP_HEADER}\n");
    for r in state.active_caps.values() {
        contents.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.pid,
            r.creation,
            r.role,
            r.minimum,
            r.maximum,
            r.flags,
            r.applied_maximum_mb
                .map(|v| v.to_string())
                .unwrap_or_default(),
            r.applied_at
                .replace('\t', " ")
                .replace('\r', " ")
                .replace('\n', " "),
            if r.manual { "manual" } else { "automatic" }
        ));
    }
    let mut file =
        File::create(&temp).map_err(|e| format!("Could not stage recovery data: {e}"))?;
    file.write_all(contents.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    replace_file(&temp, &state.backup_path)
}

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

pub fn configure_install(app_dir_arg: &str) -> Result<(), String> {
    let (os_name, build) = platform::detect_supported_client_os()?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let base = exe
        .parent()
        .ok_or("Could not determine installation directory")?
        .to_path_buf();
    let settings_path = base.join("settings.json");
    let app_dir = PathBuf::from(app_dir_arg);
    let index_path = app_dir.join("index.js");
    if !index_path.is_file() || !app_dir.join("manifest.json").is_file() {
        return Err(
            "The Spicetify Custom App package is incomplete (index.js or manifest.json missing)."
                .to_string(),
        );
    }

    // Legacy Node installations must restore before this server takes over.
    let legacy_backup = base.join("working-set-backup.json");
    let legacy_helper = base.join("helper.js");
    if legacy_backup.exists() {
        if !legacy_helper.is_file() {
            return Err("A legacy Node recovery backup exists but helper.js is missing. Restore the old limits before migrating; recovery data was preserved.".to_string());
        }
        stop_legacy_node_helper(&base)?;
        let node = find_node().ok_or("A legacy Node recovery backup exists. Install Node.js 22+ temporarily so the old helper can restore limits, then rerun setup.")?;
        let result = Command::new(node)
            .arg(&legacy_helper)
            .arg("--restore-limits")
            .output()
            .map_err(|e| format!("Could not start legacy working-set recovery: {e}"))?;
        if !result.status.success() {
            return Err("The previous helper could not restore every working-set limit. Installation stopped and the recovery backup was preserved.".to_string());
        }
        cleanup_legacy_node_files(&base)?;
    } else if legacy_helper.exists() {
        // No legacy backup means the previous helper is idle; stop only this project's
        // matching Node process and remove its now-obsolete runtime files.
        cleanup_legacy_node_files(&base)?;
    }

    let existing_raw = fs::read_to_string(&settings_path).unwrap_or_default();
    let existing_token = parse_json_string(&existing_raw, "token");
    let app_token = load_app_token(&index_path);
    let token = existing_token
        .filter(|value| value.len() >= 32)
        .or(app_token)
        .unwrap_or(platform::random_token()?);
    write_configured_app(&index_path, &token)?;
    let settings = Settings {
        port: DEFAULT_PORT,
        token,
        enabled: parse_json_bool(&existing_raw, "enabled").unwrap_or(true),
        auto_optimize: parse_json_bool(&existing_raw, "autoOptimize").unwrap_or(true),
        hidden_delay_seconds: parse_json_number(&existing_raw, "hiddenDelaySeconds")
            .unwrap_or(DEFAULT_HIDDEN_DELAY_SECONDS)
            .clamp(1, 30),
        trim_to_cap: parse_json_bool(&existing_raw, "trimToCap").unwrap_or(true),
        optimize_when_background: parse_json_bool(&existing_raw, "optimizeWhenBackground")
            .unwrap_or(true),
        optimize_when_minimized: parse_json_bool(&existing_raw, "optimizeWhenMinimized")
            .unwrap_or(true),
        optimize_when_hidden: parse_json_bool(&existing_raw, "optimizeWhenHidden").unwrap_or(true),
        manual_caps: parse_manual_caps(&existing_raw),
        manual_role_caps: parse_manual_role_caps(&existing_raw),
    };
    write_settings(&settings_path, &settings)?;
    // Start the local API before "spicetify apply" restarts Spotify, so the
    // Custom App does not race server startup during its first render.
    let cli = configure_spicetify(APP_NAME)?;
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
    let base = install_dir()?;
    let settings_path = base.join("settings.json");

    // Migrate the previous Node helper only after its own recovery command succeeds.
    if read_legacy_node_backup() {
        let legacy_helper = base.join("helper.js");
        if !legacy_helper.is_file() {
            return Err("A legacy Node recovery backup exists but helper.js is missing. Recovery data was preserved.".to_string());
        }
        stop_legacy_node_helper(&base)?;
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
        cleanup_legacy_node_files(&base)?;
    }

    // Ask a running instance of the server to restore limits and exit. A live process that
    // does not acknowledge shutdown blocks replacement of the executable.
    if settings_path.exists() {
        let settings = load_settings(&settings_path)?;
        let _ = request_shutdown(settings.port, &settings.token);
        thread::sleep(Duration::from_millis(400));
    }

    let pid_file = base.join("helper.pid");
    let installed_exe = base.join("MemorySentry.exe");
    for _ in 0..50 {
        let active_pid = fs::read_to_string(&pid_file)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        if active_pid
            .map(|pid| platform::process_matches_executable(pid, &installed_exe))
            .unwrap_or(false)
        {
            thread::sleep(Duration::from_millis(100));
        } else {
            break;
        }
    }
    if let Some(pid) = fs::read_to_string(&pid_file)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    {
        if platform::process_matches_executable(pid, &installed_exe) {
            return Err(format!("Memory Sentry is still active (PID {pid}) after the shutdown request. The recovery data was preserved."));
        }
    }

    // Recover any durable records left behind by a crash or an unresponsive helper.
    let backup_path = base.join("working-set-backup.tsv");
    let records = load_backup_file(&backup_path);
    let mut failures = Vec::new();
    for record in &records {
        match platform::restore_working_set_limits(
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
        return Err(format!("Upgrade stopped because some working-set limits could not be restored; recovery data was preserved:\\n{}", failures.join("\\n")));
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
    let base = install_dir()?;
    let settings_path = base.join("settings.json");
    let installed_exe = base.join("MemorySentry.exe");
    let pid_file = base.join("helper.pid");

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
            .map(|pid| platform::process_matches_executable(pid, &installed_exe))
            .unwrap_or(false)
        {
            thread::sleep(Duration::from_millis(100));
        } else {
            break;
        }
    }
    if let Some(pid) = fs::read_to_string(&pid_file)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    {
        if platform::process_matches_executable(pid, &installed_exe) {
            return Err(format!("Memory Sentry is still active (PID {pid}) after the shutdown request. Nothing was replaced."));
        }
    }

    // In-place swap is only possible after the old process released the executable.
    let same_file = source
        .canonicalize()
        .ok()
        .map(|value| installed_exe.canonicalize().ok() == Some(value))
        .unwrap_or(false);
    if !same_file {
        fs::create_dir_all(&base).map_err(|e| e.to_string())?;
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
    let base = install_dir()?;
    prepare_upgrade()?;
    unregister_startup()?;
    unregister_spicetify()?;
    cleanup_legacy_startup();
    preserve_logs(&base);
    let _ = fs::remove_file(base.join("helper.pid"));
    println!("Memory Sentry was stopped, recovery was attempted, startup/app registration was removed, and logs were preserved outside the install folder.");
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
    let (name, build) = platform::detect_supported_client_os()?;
    println!("Supported system detected: {name} build {build}.");
    Ok(())
}

pub fn check_spicetify_cli() -> Result<(), String> {
    let cli = find_spicetify().ok_or(
        "Spicetify CLI was not found. Install Spicetify for the same Windows user that runs Spotify, then rerun Memory Sentry Setup."
    )?;
    println!("Spicetify CLI found at {}.", cli.display());
    Ok(())
}

pub fn check_install_prerequisites() -> Result<(), String> {
    check_supported_os()?;
    check_spicetify_cli()?;
    println!("Installation prerequisites passed. Spotify may restart automatically when Spicetify applies the Custom App.");
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

fn preserve_logs(base: &Path) {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let source = base.join("logs");
    if !source.is_dir() {
        return;
    }
    let dest = PathBuf::from(local).join("MemorySentry").join("logs");
    if fs::create_dir_all(&dest).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(&source) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let mut target = dest.join(name);
        if target.exists() {
            let renamed = format!(
                "{}-{}.log",
                path.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("helper"),
                now_ms()
            );
            target = dest.join(renamed);
        }
        let _ = fs::copy(path, target);
    }
}

pub fn restore_limits_command() -> Result<(), String> {
    let base = install_dir()?;
    if read_legacy_node_backup() {
        return Err("This installation still has the legacy Node recovery format. Use its installed Node helper to restore limits before changing versions.".to_string());
    }
    let backup_path = base.join("working-set-backup.tsv");
    let records = load_backup_file(&backup_path);
    let mut failed = Vec::new();
    for record in &records {
        match platform::restore_working_set_limits(
            record.pid,
            record.creation,
            record.minimum,
            record.maximum,
            record.flags,
        ) {
            Ok(platform::RestoreOutcome::Restored) => println!("Restored PID {}", record.pid),
            Ok(platform::RestoreOutcome::ProcessGone | platform::RestoreOutcome::PidReused) => {
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

fn build_state(base: PathBuf, settings: Settings) -> AppState {
    AppState {
        settings_path: base.join("settings.json"),
        backup_path: base.join("working-set-backup.tsv"),
        log_path: base.join("logs").join("helper.log"),
        install_dir: base,
        settings,
        started_at: iso_now(),
        snapshot: platform::Snapshot::default(),
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
        active_caps: BTreeMap::new(),
        profiles: BTreeMap::new(),
        target_retry_at_ms: BTreeMap::new(),
        page_fault_baselines: BTreeMap::new(),
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
    platform::detect_supported_client_os()?;
    let base = install_dir()?;
    fs::create_dir_all(base.join("logs")).map_err(|e| e.to_string())?;
    let settings_path = base.join("settings.json");
    let settings = load_settings(&settings_path)?;
    let _instance = platform::acquire_single_instance()?;
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
    let _ = fs::write(&pid_file, platform::current_process_id().to_string());

    let shared = Arc::new(Mutex::new(state));
    if let Err(error) = start_api(Arc::clone(&shared)) {
        let _ = fs::remove_file(&pid_file);
        return Err(error);
    }
    let (tx, rx) = mpsc::channel::<Result<platform::Snapshot, String>>();
    thread::spawn(move || loop {
        let result = platform::powershell_snapshot();
        if tx.send(result).is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(SNAPSHOT_INTERVAL_MS));
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
                platform::foreground_process_id(),
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
            if now.saturating_sub(s.last_memory_sample_at_ms) >= MEMORY_SAMPLE_INTERVAL_MS {
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
    let _ = fs::remove_file(&pid_file);
    Ok(())
}

fn accept_snapshot(state: &mut AppState, mut snapshot: platform::Snapshot) {
    let sampled_at = now_ms();
    let elapsed_ms = sampled_at.saturating_sub(state.snapshot_at_ms);
    for process in &mut snapshot.processes {
        process.cpu_percent = state
            .snapshot
            .processes
            .iter()
            .find(|previous| previous.pid == process.pid && previous.creation == process.creation)
            .and_then(|previous| {
                estimate_cpu_percent(
                    previous.cpu_time_ms as u64,
                    process.cpu_time_ms as u64,
                    elapsed_ms,
                )
            })
            .map(|value| (value * 10.0).round() / 10.0);
        let key = format!("{}:{}", process.pid, process.creation);
        process.page_faults_delta = state.page_fault_baselines.get(&key).map(|previous| {
            if let Some(current) = process.page_fault_count {
                if current >= *previous {
                    current - *previous
                } else {
                    current + (1u64 << 32) - *previous
                }
            } else {
                0
            }
        });
    }

    state.snapshot = snapshot;
    state.snapshot_at_ms = sampled_at;
    state.snapshot_serial += 1;
    state.snapshot_ok = true;
    let is_foreground = spotify_process_is_foreground(
        platform::foreground_process_id(),
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
    state.target_retry_at_ms.retain(|key, _| {
        let mut parts = key.split(':');
        let pid = parts.next().and_then(|s| s.parse::<u32>().ok());
        let creation = parts.next().and_then(|s| s.parse::<u64>().ok());
        match (pid, creation) {
            (Some(pid), Some(creation)) => active.get(&pid).copied() == Some(creation),
            _ => false,
        }
    });
    refresh_audio_session_pids(state);
}

fn refresh_audio_session_pids(state: &mut AppState) {
    match platform::audio_session_pids() {
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

fn is_child_process_command(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    lower.contains("--type=") || lower.contains("--type =")
}

fn spotify_process_is_foreground(
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

fn process_role(
    process: &platform::ProcessRow,
    root_pid: Option<u32>,
    audio_pids: &BTreeSet<u32>,
) -> ProcessRole {
    if audio_pids.contains(&process.pid) {
        return ProcessRole::Audio;
    }
    let lower = process.cmd.to_ascii_lowercase();
    if lower.contains("audio.mojom.audioservice")
        || lower.contains("--utility-sub-type=audio")
        || lower.contains("--utility-sub-type = audio")
        || lower.contains("audio-service")
    {
        return ProcessRole::Audio;
    }
    if Some(process.pid) == root_pid {
        return ProcessRole::Main;
    }
    if lower.contains("--type=renderer") || lower.contains("--type = renderer") {
        return ProcessRole::Renderer;
    }
    if lower.contains("--type=gpu-process") || lower.contains("--type = gpu-process") {
        return ProcessRole::Gpu;
    }
    if lower.contains("--type=utility") || lower.contains("--type = utility") {
        return ProcessRole::Utility;
    }
    if lower.contains("crashpad") {
        return ProcessRole::Crashpad;
    }
    ProcessRole::Helper
}

fn role_name(role: ProcessRole) -> &'static str {
    match role {
        ProcessRole::Main => "Main",
        ProcessRole::Gpu => "GPU",
        ProcessRole::Renderer => "Renderer",
        ProcessRole::Utility => "Utility",
        ProcessRole::Crashpad => "Crashpad",
        ProcessRole::Helper => "Helper",
        ProcessRole::Audio => "Audio",
    }
}

fn descendants(state: &AppState) -> Vec<(platform::ProcessRow, ProcessRole)> {
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

fn target_key(pid: u32, creation: u64) -> String {
    format!("{pid}:{creation}")
}

fn ensure_profile(state: &mut AppState, pid: u32, creation: u64, role: ProcessRole, now: u64) {
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

fn persist_or_log(state: &mut AppState) -> bool {
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

fn apply_initial_cap(
    state: &mut AppState,
    process: &platform::ProcessRow,
    role: ProcessRole,
    now: u64,
) -> Result<bool, String> {
    if state.active_caps.get(&process.pid).map(|r| r.creation) == Some(process.creation) {
        ensure_profile(state, process.pid, process.creation, role, now);
        return Ok(false);
    }
    // Do not restore a stale backup to a process that has reused the PID.
    if state.active_caps.get(&process.pid).is_some() {
        state.active_caps.remove(&process.pid);
        let _ = persist_or_log(state);
    }
    let limits = platform::read_original_limits(process.pid, process.creation)?;
    let before = process.working_set_mb;
    let baseline = role.baseline_cap_mb().unwrap_or(24);
    let record = BackupRecord::capture_original(process.pid, process.creation, role, limits, false);
    state.active_caps.insert(process.pid, record);
    // Recovery must be durable before the working-set limit is changed.
    if !persist_or_log(state) {
        state.active_caps.remove(&process.pid);
        return Err("Recovery record could not be saved; no memory limit was changed.".to_string());
    }
    let cap_bytes = baseline as usize * 1024 * 1024;
    let minimum = platform::MIN_WORKING_SET_BYTES.min(cap_bytes);
    if let Err(error) = platform::set_working_set_limits(
        process.pid,
        process.creation,
        minimum,
        cap_bytes,
        platform::HARD_CAP_FLAGS,
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
        trimmed = platform::empty_working_set(process.pid, process.creation).is_ok();
    }
    let after = platform::inspect_process(process.pid)
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

fn enforce_fixed_audio_cap(
    state: &mut AppState,
    process: &platform::ProcessRow,
) -> Result<bool, String> {
    let Some(record) = state
        .active_caps
        .get(&process.pid)
        .filter(|record| record.creation == process.creation && !record.manual)
        .cloned()
    else {
        return Ok(false);
    };
    if record.applied_maximum_mb == Some(36) {
        return Ok(false);
    }

    let cap_bytes = 36usize * 1024 * 1024;
    let minimum = platform::MIN_WORKING_SET_BYTES.min(cap_bytes);
    platform::set_working_set_limits(
        process.pid,
        process.creation,
        minimum,
        cap_bytes,
        platform::HARD_CAP_FLAGS,
    )?;
    if let Some(record) = state.active_caps.get_mut(&process.pid) {
        record.record_applied_cap(ProcessRole::Audio, 36, false);
    }
    if let Some(profile) = state.profiles.get_mut(&process.pid) {
        profile.current_cap_mb = 36;
        profile.last_reason = "fixed-audio-ceiling".to_string();
    }
    if !persist_or_log(state) {
        return Err("Audio cap was applied, but the recovery backup update failed.".to_string());
    }
    log_event(
        state,
        format!(
            "Applied fixed 36 MB automatic Audio ceiling to PID {}.",
            process.pid
        ),
        "info",
    );
    Ok(true)
}

fn apply_manual_cap(
    state: &mut AppState,
    process: &platform::ProcessRow,
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
    if state.active_caps.get(&process.pid).is_some() && previous.is_none() {
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
        let limits = platform::read_original_limits(process.pid, process.creation)?;
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
    let minimum = platform::MIN_WORKING_SET_BYTES.min(cap_bytes);
    if let Err(error) = platform::set_working_set_limits(
        process.pid,
        process.creation,
        minimum,
        cap_bytes,
        platform::HARD_CAP_FLAGS,
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
        && platform::empty_working_set(process.pid, process.creation).is_ok();
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

fn restore_one_record(
    state: &mut AppState,
    record: &BackupRecord,
    reason: &str,
) -> Result<(), String> {
    match platform::restore_working_set_limits(
        record.pid,
        record.creation,
        record.minimum,
        record.maximum,
        record.flags,
    ) {
        Ok(platform::RestoreOutcome::Restored) => {
            log_event(
                state,
                format!(
                    "Restored original working-set limits for {} PID {} ({reason}).",
                    record.role, record.pid
                ),
                "info",
            );
        }
        Ok(platform::RestoreOutcome::ProcessGone) => {
            log_event(
                state,
                format!("Removed recovery for exited PID {} ({reason}).", record.pid),
                "warn",
            );
        }
        Ok(platform::RestoreOutcome::PidReused) => {
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

fn restore_all(state: &mut AppState, reason: &str) -> (usize, usize) {
    let records: Vec<BackupRecord> = state.active_caps.values().cloned().collect();
    if records.is_empty() {
        return (0, 0);
    }
    let started = std::time::Instant::now();
    let mut restored_or_gone = 0usize;
    let mut failed = 0usize;
    for record in records {
        match platform::restore_working_set_limits(
            record.pid,
            record.creation,
            record.minimum,
            record.maximum,
            record.flags,
        ) {
            Ok(platform::RestoreOutcome::Restored) => {
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
            Ok(platform::RestoreOutcome::ProcessGone) => {
                state.active_caps.remove(&record.pid);
                state.profiles.remove(&record.pid);
                restored_or_gone += 1;
                log_event(
                    state,
                    format!("Removed recovery for exited PID {} ({reason}).", record.pid),
                    "warn",
                );
            }
            Ok(platform::RestoreOutcome::PidReused) => {
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
                failed += 1;
                log_event(
                    state,
                    format!(
                        "Could not restore PID {}: {error}; recovery was preserved.",
                        record.pid
                    ),
                    "error",
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

fn update_caps(state: &mut AppState, now: u64) {
    if !state.snapshot_ok
        || state.snapshot_at_ms == 0
        || now.saturating_sub(state.snapshot_at_ms) > SNAPSHOT_STALE_MS
    {
        return;
    }
    let targets = descendants(state);
    for (process, role) in targets {
        if role == ProcessRole::Audio {
            continue;
        }
        let record = state.active_caps.get(&process.pid).cloned();
        let Some(record) = record else {
            continue;
        };
        if record.creation != process.creation {
            continue;
        }
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
        let cpu = process.cpu_percent;
        let working_set = process.working_set_mb;
        if let Some(profile) = state.profiles.get_mut(&process.pid) {
            profile.last_evaluated_snapshot_at_ms = state.snapshot_at_ms;
            profile.last_cpu_percent = cpu;
            profile.last_working_set_mb = working_set;
            match cpu {
                Some(cpu)
                    if cpu >= HIGH_CPU_PERCENT
                        && working_set
                            .map(|ws| working_set_at_ceiling(ws, current_cap))
                            .unwrap_or(false) =>
                {
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
        let profile = match state.profiles.get(&process.pid) {
            Some(p) => p.clone(),
            None => continue,
        };
        let cpu = cpu.unwrap_or(0.0);
        let ws = working_set.unwrap_or(current_cap as f64);
        let profile_sample = memory_sentry_core::PolicySample {
            role,
            current_cap_mb: current_cap,
            working_set_mb: ws,
            cpu_percent: cpu,
            high_cpu_ms: profile.high_cpu_ms,
            low_cpu_ms: profile.low_cpu_ms,
            last_adjustment_ms: profile.last_adjustment_at_ms,
            now_ms: now,
        };
        if let Some(decision) = decide_adjustment(profile_sample) {
            let old_cap = current_cap;
            let next_cap = decision.next_cap_mb;
            let cap_bytes = next_cap as usize * 1024 * 1024;
            let minimum = platform::MIN_WORKING_SET_BYTES.min(cap_bytes);
            match platform::set_working_set_limits(
                process.pid,
                process.creation,
                minimum,
                cap_bytes,
                platform::HARD_CAP_FLAGS,
            ) {
                Ok(()) => {
                    if let Some(record) = state.active_caps.get_mut(&process.pid) {
                        record.record_applied_cap(role, next_cap, false);
                    }
                    if let Some(profile) = state.profiles.get_mut(&process.pid) {
                        profile.current_cap_mb = next_cap;
                        profile.last_adjustment_at_ms = now;
                        profile.last_reason = match decision.direction {
                            AdjustmentDirection::Increase => "sustained-high-cpu",
                            AdjustmentDirection::Decrease => "low-cpu-and-memory-headroom",
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
                            platform::empty_working_set(process.pid, process.creation).is_ok();
                        if let Some(p) = state.profiles.get_mut(&process.pid) {
                            p.last_trim_at_ms = now;
                        }
                    }
                    if let Err(error) = persist_backups(state) {
                        log_event(state, format!("Adjusted PID {} to {next_cap} MB but could not persist updated recovery state: {error}", process.pid), "error");
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

fn manual_cap_for(role: ProcessRole, configured: Option<u32>) -> u32 {
    configured.unwrap_or_else(|| role.manual_default_cap_mb())
}

fn configured_manual_cap(
    settings: &Settings,
    process: &platform::ProcessRow,
    role: ProcessRole,
) -> Option<u32> {
    settings
        .manual_role_caps
        .get(role_name(role))
        .copied()
        .or_else(|| settings.manual_caps.get(&(process.pid, process.creation)).copied())
}

fn reconcile_manual_caps(state: &mut AppState) {
    if !state.snapshot_ok
        || state.snapshot_at_ms == 0
        || now_ms().saturating_sub(state.snapshot_at_ms) > SNAPSHOT_STALE_MS
    {
        return;
    }
    let targets = descendants(state);
    let wanted: BTreeSet<String> = targets
        .iter()
        .map(|(process, _)| target_key(process.pid, process.creation))
        .collect();
    let records: Vec<BackupRecord> = state.active_caps.values().cloned().collect();
    for record in records {
        if !wanted.contains(&target_key(record.pid, record.creation)) || !record.manual {
            if let Err(error) = restore_one_record(state, &record, "manual policy reconciliation") {
                log_event(
                    state,
                    format!(
                        "Could not restore PID {} during manual policy reconciliation: {error}",
                        record.pid
                    ),
                    "error",
                );
            }
        }
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

fn should_manage_limits(settings: &Settings, window_state: &str) -> bool {
    match window_state {
        "background-visible" => settings.optimize_when_background,
        "minimized" => settings.optimize_when_minimized,
        "hidden" => settings.optimize_when_hidden,
        _ => false,
    }
}

fn background_delay_elapsed(since: u64, now: u64, delay_seconds: u64) -> bool {
    since > 0 && now.saturating_sub(since) >= delay_seconds.saturating_mul(1000)
}

/// Reads the live window list and records which window represents Spotify now.
/// Detection no longer depends on a remembered handle: the window manager is asked
/// on every pass, so hiding to the tray, a destroyed window and a recreated one are
/// all classified from current facts instead of from cached state.
fn observe_window_state(state: &mut AppState, is_foreground: bool) -> (String, String) {
    let pids: Vec<u32> = state.snapshot.processes.iter().map(|p| p.pid).collect();
    let evidence = |rule: &str, list: &[platform::WindowCandidate]| {
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
    let candidates = platform::window_candidates(&pids);
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
    let preferred: Vec<platform::WindowCandidate> = candidates
        .iter()
        .filter(|window| root_pids.contains(&window.pid))
        .cloned()
        .collect();
    let pool: &[platform::WindowCandidate] = if preferred.is_empty() { &candidates } else { &preferred };
    let (name, chosen) = platform::classify_windows(pool, is_foreground);
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

fn window_state_is_unknown(window_state: &str) -> bool {
    matches!(
        window_state,
        "window-not-found" | "not-detected" | "window-check-error"
    )
}

fn maybe_auto_optimize(state: &mut AppState, now: u64) {
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
    let wanted: BTreeSet<String> = targets
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
            let outcome = platform::restore_working_set_limits(
                record.pid,
                record.creation,
                record.minimum,
                record.maximum,
                record.flags,
            );
            match outcome {
                Ok(_) => {
                    state.active_caps.remove(&record.pid);
                    state.profiles.remove(&record.pid);
                }
                Err(error) => log_event(
                    state,
                    format!(
                        "Could not restore no-longer-eligible PID {}: {error}",
                        record.pid
                    ),
                    "error",
                ),
            }
        }
        let _ = persist_or_log(state);
    }
    let mut applied = 0usize;
    let mut failures = Vec::new();
    for (process, role) in targets {
        if state.active_caps.get(&process.pid).map(|r| r.creation) == Some(process.creation) {
            if role == ProcessRole::Audio {
                if let Err(error) = enforce_fixed_audio_cap(state, &process) {
                    failures.push(format!("Audio PID {}: {error}", process.pid));
                }
            } else {
                ensure_profile(state, process.pid, process.creation, role, now);
            }
            continue;
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
            .insert(retry_key.clone(), now + TARGET_RETRY_DELAY_MS);
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

fn refresh_memory(state: &mut AppState, now: u64) {
    let previous = state.last_memory_sample_at_ms;
    let interval = if previous > 0 {
        Some(now.saturating_sub(previous))
    } else {
        None
    };
    let mut fault_delta = 0u64;
    let mut measured = 0u64;
    for process in &mut state.snapshot.processes {
        if let Ok(stats) = platform::inspect_process(process.pid) {
            if stats.creation == process.creation {
                process.working_set_mb = Some(stats.working_set_mb);
                let key = target_key(process.pid, process.creation);
                if let Some(previous_faults) = state.page_fault_baselines.get(&key).copied() {
                    let delta = if stats.page_fault_count >= previous_faults {
                        stats.page_fault_count - previous_faults
                    } else {
                        stats.page_fault_count + (1u64 << 32) - previous_faults
                    };
                    process.page_faults_delta = Some(delta);
                    fault_delta = fault_delta.saturating_add(delta);
                    measured += 1;
                }
                process.page_fault_count = Some(stats.page_fault_count);
                process.error_code = 0;
                state
                    .page_fault_baselines
                    .insert(key, stats.page_fault_count);
            }
        }
    }
    if let Some(interval_ms) = interval.filter(|_| measured > 0) {
        state.telemetry.page_fault_delta = Some(fault_delta);
        state.telemetry.page_faults_per_second =
            Some(fault_delta as f64 * 1000.0 / interval_ms.max(1) as f64);
        state.telemetry.sample_interval_ms = Some(interval_ms);
    }
    state.last_memory_sample_at_ms = now;
    // Clear fault baselines only for process identities no longer present.
    let active_keys: BTreeSet<String> = state
        .snapshot
        .processes
        .iter()
        .map(|p| target_key(p.pid, p.creation))
        .collect();
    state
        .page_fault_baselines
        .retain(|key, _| active_keys.contains(key));
}

fn public_config(settings: &Settings) -> String {
    format!(
        "{{\"enabled\":{},\"autoOptimize\":{},\"hiddenDelaySeconds\":{},\"trimToCap\":{},\"optimizeWhenBackground\":{},\"optimizeWhenMinimized\":{},\"optimizeWhenHidden\":{},\"policy\":{{\"minMB\":{},\"maxMB\":{},\"cpuHighPercent\":{}}}}}",
        settings.enabled, settings.auto_optimize, settings.hidden_delay_seconds, settings.trim_to_cap, settings.optimize_when_background, settings.optimize_when_minimized, settings.optimize_when_hidden, MIN_CAP_MB, MAX_CAP_MB, HIGH_CPU_PERCENT
    )
}

fn status_payload(state: &AppState) -> String {
    let targets = descendants(state);
    let process_count = state.snapshot.processes.len();
    let private_working_set_available = process_count > 0
        && state
            .snapshot
            .processes
            .iter()
            .all(|p| p.private_working_set_mb.is_some());
    let displayed_memory_mb = |process: &platform::ProcessRow| {
        if private_working_set_available {
            process.private_working_set_mb
        } else {
            process.working_set_mb
        }
    };
    let spotify_total_mb: f64 = state
        .snapshot
        .processes
        .iter()
        .filter_map(displayed_memory_mb)
        .sum();
    let target_total_mb: f64 = targets
        .iter()
        .filter_map(|(p, _)| displayed_memory_mb(p))
        .sum();
    let capped_count = state
        .active_caps
        .values()
        .filter(|r| {
            state
                .snapshot
                .processes
                .iter()
                .any(|p| p.pid == r.pid && p.creation == r.creation)
        })
        .count();

    let target_json: Vec<String> = targets.iter().map(|(p, role)| {
        let record = state.active_caps.get(&p.pid).filter(|r| r.creation == p.creation);
        let profile = state.profiles.get(&p.pid).filter(|profile| profile.creation == p.creation);
        let capped = record.is_some();
        let protected_audio = *role == ProcessRole::Audio;
        let current_cap = record.and_then(|r| r.applied_maximum_mb)
            .or_else(|| profile.map(|p| p.current_cap_mb))
            .or_else(|| (state.settings.auto_optimize && protected_audio).then_some(36));
        let default_cap = role.manual_default_cap_mb();
        let manual_cap = configured_manual_cap(&state.settings, p, *role);
        let adaptive_status = profile.map(|p| p.last_reason.as_str()).unwrap_or("baseline");
        let original_limits = platform::read_original_limits(p.pid, p.creation).ok();
        let original_unlimited = original_limits
            .as_ref()
            .map(|limits| limits.maximum >= usize::MAX / 2)
            .unwrap_or(false);
        let original_maximum_mb = original_limits
            .as_ref()
            .filter(|_| !original_unlimited)
            .map(|limits| (limits.maximum as f64 / (1024.0 * 1024.0)).round());
        format!(
            "{{\"pid\":{},\"creation\":{},\"role\":{},\"workingSetMB\":{},\"privateWorkingSetMB\":{},\"privateMB\":{},\"cpuPercent\":{},\"pageFaultCount\":{},\"pageFaultsDelta\":{},\"errorCode\":{},\"capped\":{},\"manual\":{},\"included\":{},\"protectedAudio\":{},\"limitMB\":{},\"manualCapMB\":{},\"defaultLimitMB\":{},\"adaptiveStatus\":{},\"originalMaximumMB\":{},\"originalCeilingUnlimited\":{}}}",
            p.pid, json_quote(&p.creation.to_string()), json_quote(role_name(*role)),
            json_optional_number(p.working_set_mb),
            json_optional_number(p.private_working_set_mb),
            json_optional_number(p.private_mb),
            json_optional_number(p.cpu_percent),
            json_optional_u64(p.page_fault_count),
            json_optional_u64(p.page_faults_delta),
            p.error_code,
            capped,
            record.map(|r| r.manual).unwrap_or(false),
            true,
            protected_audio,
            current_cap.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            manual_cap.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            default_cap,
            json_quote(adaptive_status),
            json_optional_number(original_maximum_mb),
            original_unlimited
        )
    }).collect();

    let events: Vec<String> = state
        .recent_events
        .iter()
        .rev()
        .map(|e| {
            format!(
                "{{\"time\":{},\"level\":{},\"message\":{}}}",
                json_quote(&e.time),
                json_quote(&e.level),
                json_quote(&e.message)
            )
        })
        .collect();

    let last_action = state
        .last_action
        .as_ref()
        .map(|action| {
            let failure_json: Vec<String> = action.failures.iter().map(|e| json_quote(e)).collect();
            format!(
                "{{\"time\":{},\"reason\":{},\"applied\":{},\"processes\":[],\"failures\":[{}]}}",
                json_quote(&action.time),
                json_quote(&action.reason),
                action.applied,
                failure_json.join(",")
            )
        })
        .unwrap_or_else(|| "null".to_string());

    let hidden_seconds = if state.hidden_since_ms > 0 {
        now_ms().saturating_sub(state.hidden_since_ms) / 1000
    } else {
        0
    };
    let memory = format!(
        "{{\"spotifyTotalMB\":{},\"targetTotalMB\":{},\"processCount\":{},\"targetCount\":{},\"cappedCount\":{},\"privateWorkingSet\":{},\"targets\":[{}],\"sampledAt\":{}}}",
        json_num(spotify_total_mb), json_num(target_total_mb), process_count, targets.len(), capped_count,
        private_working_set_available, target_json.join(","), json_quote(&iso_now())
    );
    let telemetry = format!(
        "{{\"lastRestoreDurationMs\":{},\"lastRestoreAt\":{},\"lastRestoreRestored\":{},\"lastRestoreFailures\":{},\"pageFaultDelta\":{},\"pageFaultsPerSecond\":{},\"sampleIntervalMs\":{},\"adaptiveAdjustments\":{},\"lastAdaptiveAdjustmentAt\":{}}}",
        json_optional_number(state.telemetry.last_restore_duration_ms),
        json_optional_string(state.telemetry.last_restore_at.as_deref()),
        state.telemetry.last_restore_restored,
        state.telemetry.last_restore_failures,
        json_optional_u64(state.telemetry.page_fault_delta),
        json_optional_number(state.telemetry.page_faults_per_second),
        json_optional_u64(state.telemetry.sample_interval_ms),
        state.telemetry.adaptive_adjustments,
        json_optional_string(state.telemetry.last_adaptive_adjustment_at.as_deref())
    );

    format!(
        "{{\"ok\":true,\"helper\":{{\"running\":true,\"version\":{},\"node\":\"Server\",\"runtime\":\"Server\",\"startedAt\":{}}},\"spotify\":{{\"found\":{},\"rootPid\":{},\"windowTitle\":{},\"windowState\":{},\"windowStateReason\":{},\"hiddenSeconds\":{}}},\"config\":{},\"memory\":{},\"lastAction\":{},\"telemetry\":{},\"lastError\":{},\"recentEvents\":[{}],\"cap\":{{\"active\":{},\"cappedProcesses\":{},\"adaptive\":true,\"minMB\":{},\"maxMB\":{},\"audioProtected\":true,\"audioFixedCapMB\":36}}}}",
        json_quote(VERSION),
        json_quote(&state.started_at),
        state.root_pid.is_some(),
        state.root_pid.map(|p| p.to_string()).unwrap_or_else(|| "null".to_string()),
        json_quote(&state.window_title),
        json_quote(&state.window_state),
        json_quote(&state.window_state_reason),
        hidden_seconds,
        public_config(&state.settings),
        memory,
        last_action,
        telemetry,
        json_quote(&state.last_error),
        events.join(","),
        !state.active_caps.is_empty(),
        capped_count,
        MIN_CAP_MB,
        MAX_CAP_MB
    )
}

fn json_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn json_num(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.3}")
    } else {
        "0".to_string()
    }
}
fn json_optional_number(value: Option<f64>) -> String {
    value
        .filter(|v| v.is_finite())
        .map(json_num)
        .unwrap_or_else(|| "null".to_string())
}
fn json_optional_u64(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
}
fn json_optional_string(value: Option<&str>) -> String {
    value.map(json_quote).unwrap_or_else(|| "null".to_string())
}

struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: String,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let mut buffer = Vec::<u8>::new();
    let mut header_end = None;
    let mut content_length = 0usize;
    let mut chunk = [0u8; 4096];
    loop {
        if buffer.len() > MAX_HTTP_HEADER_BYTES + MAX_HTTP_BODY_BYTES {
            return Err("HTTP request exceeds the maximum size.".to_string());
        }
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("HTTP read failed: {e}"))?;
        if n == 0 {
            if buffer.is_empty() {
                return Err("Empty request.".to_string());
            }
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if header_end.is_none() {
            if let Some(pos) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                header_end = Some(pos + 4);
                if pos > MAX_HTTP_HEADER_BYTES {
                    return Err("HTTP headers are too large.".to_string());
                }
                let header_text = String::from_utf8_lossy(&buffer[..pos]);
                for line in header_text.lines().skip(1) {
                    if let Some((name, value)) = line.split_once(':') {
                        if name.trim().eq_ignore_ascii_case("content-length") {
                            content_length = value
                                .trim()
                                .parse::<usize>()
                                .map_err(|_| "Invalid Content-Length")?;
                        }
                        if name.trim().eq_ignore_ascii_case("transfer-encoding") {
                            return Err("Chunked request bodies are not supported.".to_string());
                        }
                    }
                }
                if content_length > MAX_HTTP_BODY_BYTES {
                    return Err("Request body is too large.".to_string());
                }
            }
        }
        if let Some(pos) = header_end {
            if buffer.len() >= pos + content_length {
                break;
            }
        }
    }
    let pos = header_end.ok_or("HTTP header terminator was not found.")?;
    let header_text = String::from_utf8_lossy(&buffer[..pos - 4]);
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or("Request line is missing.")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("").to_ascii_uppercase();
    let path = request_parts.next().unwrap_or("").to_string();
    if method.is_empty() || path.is_empty() {
        return Err("Invalid request line.".to_string());
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let body_bytes = buffer.get(pos..pos + content_length).unwrap_or(&[]);
    let body = String::from_utf8_lossy(body_bytes).into_owned();
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn trusted_origin(origin: &str) -> bool {
    let origin = origin.trim();
    let Some(rest) = origin.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("").to_ascii_lowercase();
    !authority.is_empty()
        && !authority.contains('@')
        && (authority == "spotify.com" || authority.ends_with(".spotify.com"))
        && !authority.contains(':')
}

fn constant_time_equals(left: &str, right: &str) -> bool {
    let a = left.as_bytes();
    let b = right.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for index in 0..a.len() {
        diff |= a[index] ^ b[index];
    }
    diff == 0
}

fn parse_bearer(value: &str) -> Option<&str> {
    let (scheme, token) = value.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
}

fn write_http_response(
    mut stream: TcpStream,
    status: u16,
    reason: &str,
    body: &str,
    origin: Option<&str>,
) {
    let cors = origin.filter(|value| trusted_origin(value)).map(|value| format!(
        "Access-Control-Allow-Origin: {value}\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Authorization, Content-Type\r\nAccess-Control-Allow-Private-Network: true\r\nAccess-Control-Max-Age: 300\r\nVary: Origin\r\n"
    )).unwrap_or_default();
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nContent-Length: {}\r\n{}\r\n{}",
        body.as_bytes().len(), cors, body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn manual_target(
    state: &AppState,
    pid: u32,
    creation: u64,
) -> Result<(platform::ProcessRow, ProcessRole), String> {
    if !state.snapshot_ok
        || state.snapshot_at_ms == 0
        || now_ms().saturating_sub(state.snapshot_at_ms) > SNAPSHOT_STALE_MS
    {
        return Err("The Spotify process list is stale; refresh it and try again.".to_string());
    }
    descendants(state).into_iter()
        .find(|(process, _)| process.pid == pid && process.creation == creation)
        .ok_or_else(|| "That Spotify process is no longer running with the same identity. Refresh and try again.".to_string())
}

fn set_manual_cap(state: &mut AppState, body: &str) -> Result<(), String> {
    if !state.settings.enabled {
        return Err("Enable Memory Sentry before setting a manual process limit.".to_string());
    }
    if state.settings.auto_optimize {
        return Err(
            "Turn off automatic optimization before setting a manual process limit.".to_string(),
        );
    }
    let pid = parse_json_number(body, "pid")
        .and_then(|value| u32::try_from(value).ok())
        .ok_or("A valid process ID is required.")?;
    let creation = parse_json_string(body, "creation")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or("A valid process creation identity is required.")?;
    let cap_mb = parse_json_number(body, "capMB")
        .and_then(|value| u32::try_from(value).ok())
        .ok_or("A whole-number limit is required.")?;
    if !(MIN_MANUAL_CAP_MB..=MAX_MANUAL_CAP_MB).contains(&cap_mb) {
        return Err(format!(
            "Manual limits must be between {MIN_MANUAL_CAP_MB} and {MAX_MANUAL_CAP_MB} MB."
        ));
    }
    let (process, role) = manual_target(state, pid, creation)?;
    let role_key = role_name(role).to_string();
    let previous = state.settings.manual_role_caps.insert(role_key.clone(), cap_mb);
    if let Err(error) = write_settings(&state.settings_path, &state.settings) {
        if let Some(previous) = previous {
            state.settings.manual_role_caps.insert(role_key, previous);
        } else {
            state.settings.manual_role_caps.remove(&role_key);
        }
        return Err(error);
    }
    if should_manage_limits(&state.settings, &state.window_state) {
        if let Err(error) = apply_manual_cap(state, &process, role, cap_mb) {
            if let Some(previous) = previous {
                state.settings.manual_role_caps.insert(role_key.clone(), previous);
            } else {
                state.settings.manual_role_caps.remove(&role_key);
            }
            if let Err(save_error) = write_settings(&state.settings_path, &state.settings) {
                log_event(
                    state,
                    format!("Could not roll back failed manual setting: {save_error}"),
                    "error",
                );
            }
            return Err(error);
        }
    }
    Ok(())
}

fn clear_manual_cap(state: &mut AppState, body: &str) -> Result<(), String> {
    if !state.settings.enabled {
        return Err("Enable Memory Sentry before clearing a manual process limit.".to_string());
    }
    if state.settings.auto_optimize {
        return Err(
            "Turn off automatic optimization before clearing a manual process limit.".to_string(),
        );
    }
    let pid = parse_json_number(body, "pid")
        .and_then(|value| u32::try_from(value).ok())
        .ok_or("A valid process ID is required.")?;
    let creation = parse_json_string(body, "creation")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or("A valid process creation identity is required.")?;
    let (_, role) = manual_target(state, pid, creation)?;
    let role_key = role_name(role).to_string();
    let previous_role_cap = state.settings.manual_role_caps.remove(&role_key);
    let affected: Vec<platform::ProcessRow> = descendants(state)
        .into_iter()
        .filter(|(_, candidate_role)| *candidate_role == role)
        .map(|(candidate, _)| candidate)
        .collect();
    let previous_process_caps: Vec<((u32, u64), u32)> = affected
        .iter()
        .filter_map(|candidate| {
            state
                .settings
                .manual_caps
                .remove(&(candidate.pid, candidate.creation))
                .map(|cap| ((candidate.pid, candidate.creation), cap))
        })
        .collect();
    if previous_role_cap.is_none() && previous_process_caps.is_empty() {
        return Ok(());
    }
    if let Err(error) = write_settings(&state.settings_path, &state.settings) {
        if let Some(previous) = previous_role_cap {
            state.settings.manual_role_caps.insert(role_key, previous);
        }
        state.settings.manual_caps.extend(previous_process_caps);
        return Err(error);
    }
    if should_manage_limits(&state.settings, &state.window_state) {
        reconcile_manual_caps(state);
    } else {
        let records: Vec<BackupRecord> = state
            .active_caps
            .values()
            .filter(|record| record.manual && record.role == role_key)
            .cloned()
            .collect();
        for record in records {
            restore_one_record(state, &record, "manual role profile cleared in foreground")?;
        }
    }
    Ok(())
}

fn update_config(state: &mut AppState, body: &str) -> Result<(), String> {
    let mut next = state.settings.clone();
    if let Some(value) = parse_json_bool(body, "enabled") {
        next.enabled = value;
    }
    if let Some(value) = parse_json_bool(body, "autoOptimize") {
        next.auto_optimize = value;
    }
    if let Some(value) = parse_json_bool(body, "trimToCap") {
        next.trim_to_cap = value;
    }
    if let Some(value) = parse_json_bool(body, "optimizeWhenBackground") {
        next.optimize_when_background = value;
    }
    if let Some(value) = parse_json_bool(body, "optimizeWhenMinimized") {
        next.optimize_when_minimized = value;
    }
    if let Some(value) = parse_json_bool(body, "optimizeWhenHidden") {
        next.optimize_when_hidden = value;
    }
    if !next.optimize_when_background && !next.optimize_when_minimized && !next.optimize_when_hidden {
        next.optimize_when_background = true;
    }
    next.hidden_delay_seconds = parse_json_number(body, "hiddenDelaySeconds")
        .unwrap_or(next.hidden_delay_seconds)
        .clamp(1, 30);

    let policy_changed = next.auto_optimize != state.settings.auto_optimize;
    let disabling = state.settings.enabled && !next.enabled;
    if policy_changed || disabling {
        let (_, failed) = restore_all(
            state,
            if disabling {
                "plugin disabled"
            } else {
                "policy mode changed"
            },
        );
        if failed > 0 && !disabling {
            return Err("The setting was not changed because some original process limits could not be restored.".to_string());
        }
        if failed > 0 {
            log_event(
                state,
                format!("Memory Sentry is disabled; retrying restoration of {failed} remaining process limit(s)."),
                "warn",
            );
        }
    }
    if policy_changed {
        state.hidden_since_ms = 0;
    }
    write_settings(&state.settings_path, &next)?;
    state.settings = next;
    log_event(
        state,
        format!(
            "Settings updated: enabled={}, autoOptimize={}, hiddenDelaySeconds={}, trimToCap={}, optimizeWhen(bg/min/hidden)={}/{}/{}",
            state.settings.enabled,
            state.settings.auto_optimize,
            state.settings.hidden_delay_seconds,
            state.settings.trim_to_cap,
            state.settings.optimize_when_background,
            state.settings.optimize_when_minimized,
            state.settings.optimize_when_hidden
        ),
        "info",
    );
    maybe_auto_optimize(state, now_ms());
    Ok(())
}

fn handle_http_request(stream: TcpStream, shared: SharedState) {
    let mut stream = stream;
    let request = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            write_http_response(
                stream,
                400,
                "Bad Request",
                &format!("{{\"ok\":false,\"error\":{}}}", json_quote(&error)),
                None,
            );
            return;
        }
    };
    let origin = request
        .headers
        .get("origin")
        .map(String::as_str)
        .unwrap_or("");
    let host = request
        .headers
        .get("host")
        .map(|h| h.to_ascii_lowercase())
        .unwrap_or_default();
    let port = shared
        .lock()
        .ok()
        .map(|state| state.settings.port)
        .unwrap_or(DEFAULT_PORT);
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        write_http_response(
            stream,
            403,
            "Forbidden",
            r#"{"ok":false,"error":"Host is not allowed."}"#,
            None,
        );
        return;
    }
    if !trusted_origin(origin) {
        write_http_response(
            stream,
            403,
            "Forbidden",
            r#"{"ok":false,"error":"Origin is not allowed."}"#,
            None,
        );
        return;
    }
    if request.method == "OPTIONS" {
        write_http_response(stream, 204, "No Content", "", Some(origin));
        return;
    }

    let mut state = match shared.lock() {
        Ok(state) => state,
        Err(_) => {
            write_http_response(
                stream,
                500,
                "Internal Server Error",
                r#"{"ok":false,"error":"State unavailable."}"#,
                Some(origin),
            );
            return;
        }
    };
    let auth = request
        .headers
        .get("authorization")
        .and_then(|value| parse_bearer(value));
    if !auth
        .map(|value| constant_time_equals(value, &state.settings.token))
        .unwrap_or(false)
    {
        write_http_response(
            stream,
            403,
            "Forbidden",
            r#"{"ok":false,"error":"Invalid local token."}"#,
            Some(origin),
        );
        return;
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/status") => {
            let payload = status_payload(&state);
            write_http_response(stream, 200, "OK", &payload, Some(origin));
        }
        ("POST", "/api/config") => match update_config(&mut state, &request.body) {
            Ok(()) => {
                let payload = format!(
                    "{{\"ok\":true,\"config\":{}}}",
                    public_config(&state.settings)
                );
                write_http_response(stream, 200, "OK", &payload, Some(origin));
            }
            Err(error) => {
                let payload = format!("{{\"ok\":false,\"error\":{}}}", json_quote(&error));
                write_http_response(stream, 400, "Bad Request", &payload, Some(origin));
            }
        },
        ("POST", "/api/action") => {
            let action = parse_json_string(&request.body, "action").unwrap_or_default();
            if action == "setManualCap" || action == "clearManualCap" {
                let result = if action == "setManualCap" {
                    set_manual_cap(&mut state, &request.body)
                } else {
                    clear_manual_cap(&mut state, &request.body)
                };
                match result {
                    Ok(()) => {
                        let payload =
                            format!("{{\"ok\":true,\"status\":{}}}", status_payload(&state));
                        write_http_response(stream, 200, "OK", &payload, Some(origin));
                    }
                    Err(error) => {
                        let payload = format!("{{\"ok\":false,\"error\":{}}}", json_quote(&error));
                        write_http_response(stream, 400, "Bad Request", &payload, Some(origin));
                    }
                }
                return;
            }
            if action != "restoreNow" && action != "shutdown" {
                write_http_response(
                    stream,
                    400,
                    "Bad Request",
                    r#"{"ok":false,"error":"Unknown action."}"#,
                    Some(origin),
                );
                return;
            }
            if action == "restoreNow" && !state.settings.auto_optimize {
                let manual_caps = std::mem::take(&mut state.settings.manual_caps);
                if let Err(error) = write_settings(&state.settings_path, &state.settings) {
                    state.settings.manual_caps = manual_caps;
                    let payload = format!("{{\"ok\":false,\"error\":{}}}", json_quote(&error));
                    write_http_response(
                        stream,
                        500,
                        "Internal Server Error",
                        &payload,
                        Some(origin),
                    );
                    return;
                }
            }
            let result = restore_all(
                &mut state,
                if action == "shutdown" {
                    "installer shutdown request"
                } else {
                    "manual request"
                },
            );
            let payload = format!(
                "{{\"ok\":{},\"restored\":{},\"gone\":0,\"failed\":{}}}",
                result.1 == 0,
                result.0,
                result.1
            );
            if result.1 == 0 && action == "shutdown" {
                state.shutdown_requested = true;
            }
            write_http_response(
                stream,
                if result.1 == 0 { 200 } else { 409 },
                if result.1 == 0 { "OK" } else { "Conflict" },
                &payload,
                Some(origin),
            );
        }
        _ => write_http_response(
            stream,
            404,
            "Not Found",
            r#"{"ok":false,"error":"Endpoint not found."}"#,
            Some(origin),
        ),
    }
}

fn start_api(shared: SharedState) -> Result<(), String> {
    let port = shared
        .lock()
        .map_err(|_| "State lock poisoned")?
        .settings
        .port;
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|error| format!("Could not bind local API on 127.0.0.1:{port}: {error}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    thread::spawn(move || loop {
        let stop = shared.lock().map(|s| s.shutdown_requested).unwrap_or(true);
        if stop {
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let clone = Arc::clone(&shared);
                thread::spawn(move || handle_http_request(stream, clone));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20))
            }
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    });
    Ok(())
}

#[cfg(windows)]
fn start_background_copy() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let base = exe
        .parent()
        .ok_or("Could not determine installation directory")?;
    let log_dir = base.join("logs");
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

fn request_status(port: u16, token: &str) -> Result<(), String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("Could not connect to the local API on port {port}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(700)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_millis(700)))
        .ok();
    let request = format!(
        "GET /api/status HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: https://xpui.app.spotify.com\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Could not send local API health check: {e}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("Could not read local API health check: {e}"))?;
    if response.starts_with("HTTP/1.1 200") {
        return Ok(());
    }
    let status = response.lines().next().unwrap_or("No HTTP response");
    let detail = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.trim())
        .unwrap_or("");
    Err(if detail.is_empty() {
        format!("Local API health check returned {status}")
    } else {
        format!("Local API health check returned {status}: {detail}")
    })
}

fn append_startup_log(path: &Path, message: &str) -> Result<(), String> {
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
    let base = install_dir()?;
    let startup_path = base.join("logs").join("startup.log");

    // Create and write the launch log before reading settings. This records failures
    // such as a missing/corrupt settings file that occur before the detached monitor runs.
    append_startup_log(&startup_path, "Launcher invoked.")?;

    let settings_path = base.join("settings.json");
    let settings = match load_settings(&settings_path) {
        Ok(settings) => settings,
        Err(error) => {
            let detail = format!(
                "Could not load server settings at {}: {error}",
                settings_path.display()
            );
            let _ = append_startup_log(&startup_path, &detail);
            return Err(format!(
                "{detail}. Diagnostic log: {}",
                startup_path.display()
            ));
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreground_child_process_is_recognized_as_spotify() {
        let spotify_pids = [42, 84, 126];
        assert!(spotify_process_is_foreground(Some(84), spotify_pids));
        assert!(!spotify_process_is_foreground(Some(7), spotify_pids));
        assert!(!spotify_process_is_foreground(None, spotify_pids));
    }

    #[test]
    fn updating_caps_preserves_the_first_captured_original_limits() {
        let originals = platform::OriginalLimits {
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
        assert!(!background_delay_elapsed(0, 10_000, 2));
        assert!(!background_delay_elapsed(10_000, 11_999, 2));
        assert!(background_delay_elapsed(10_000, 12_000, 2));
        assert!(background_delay_elapsed(10_000, 15_000, 2));
    }

    #[test]
    fn managed_limits_are_active_only_outside_the_foreground() {
        let settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: true,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_caps: BTreeMap::new(),
            manual_role_caps: BTreeMap::new(),
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
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
            trim_to_cap: true,
            optimize_when_background: false,
            optimize_when_minimized: false,
            optimize_when_hidden: true,
            manual_caps: BTreeMap::new(),
            manual_role_caps: BTreeMap::new(),
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
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_caps: BTreeMap::new(),
            manual_role_caps: BTreeMap::new(),
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
    fn manual_caps_are_persisted_by_pid_and_full_creation_identity() {
        let identity = 116_444_736_000_000_000u64;
        let serialized = format!(r#"{{"manualCaps":["42:{identity}:36"]}}"#);
        let caps = parse_manual_caps(&serialized);
        assert_eq!(caps.get(&(42, identity)), Some(&36));
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
    fn role_override_is_reused_when_a_new_pid_replaces_an_old_process() {
        let mut settings = Settings {
            port: DEFAULT_PORT,
            token: "a".repeat(64),
            enabled: true,
            auto_optimize: false,
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_caps: BTreeMap::from([((42, 100), 40)]),
            manual_role_caps: BTreeMap::from([("Renderer".to_string(), 52)]),
        };
        let restarted_process = platform::ProcessRow {
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
    fn manual_cap_parser_discards_invalid_and_out_of_range_values() {
        let serialized =
            r#"{"manualCaps":["42:100:7","43:101:513","44:102:36","0:103:24","45:0:24"]}"#;
        let caps = parse_manual_caps(serialized);
        assert_eq!(parse_json_number(r#"{"capMB":36}"#, "capMB"), Some(36));
        assert_eq!(parse_json_number(r#"{"capMB":36.5}"#, "capMB"), None);
        assert_eq!(caps.len(), 1);
        assert_eq!(caps.get(&(44, 102)), Some(&36));
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
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_caps: BTreeMap::new(),
            manual_role_caps: BTreeMap::new(),
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
        let identity = 116_444_736_000_000_000u64;
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
            hidden_delay_seconds: DEFAULT_HIDDEN_DELAY_SECONDS,
            trim_to_cap: true,
            optimize_when_background: true,
            optimize_when_minimized: true,
            optimize_when_hidden: true,
            manual_caps: BTreeMap::new(),
            manual_role_caps: BTreeMap::new(),
        };
        settings.manual_caps.insert((42, identity), 36);
        settings.manual_role_caps.insert("Renderer".to_string(), 52);
        write_settings(&path, &settings).expect("settings should be written");
        let loaded = load_settings(&path).expect("settings should load");
        assert_eq!(loaded.manual_caps.get(&(42, identity)), Some(&36));
        assert_eq!(loaded.manual_role_caps.get("Renderer"), Some(&52));
        assert!(!loaded.enabled);
        assert!(!loaded.auto_optimize);
        let _ = fs::remove_file(path);
    }
}
