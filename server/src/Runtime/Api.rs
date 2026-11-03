//! Loopback-only HTTP/1.1 API for the dashboard: request framing, trusted-origin and bearer-token checks,
//! the status/config payload serializers and the per-connection handler. It reads and mutates the
//! shared state and delegates to the persistence and monitor helpers that remain in the parent.

use memory_sentry_windows as windows;
use std::fmt::Write as FmtWrite;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use super::*;

pub(crate) fn public_config(settings: &Settings) -> String {
    format!(
        "{{\"enabled\":{},\"autoOptimize\":{},\"customMode\":{},\"hiddenDelaySeconds\":{},\"refreshIntervalMs\":{},\"trimToCap\":{},\"optimizeWhenBackground\":{},\"optimizeWhenMinimized\":{},\"optimizeWhenHidden\":{},\"policy\":{{\"minMB\":{},\"maxMB\":{}}}}}",
        settings.enabled, settings.auto_optimize, settings.custom_mode, settings.hidden_delay_seconds, settings.refresh_interval_ms, settings.trim_to_cap, settings.optimize_when_background, settings.optimize_when_minimized, settings.optimize_when_hidden, MIN_CAP_MB, MAX_CAP_MB
    )
}

pub(crate) fn status_payload(state: &AppState) -> String {
    let targets = descendants(state);
    let process_count = state.snapshot.processes.len();
    let private_working_set_available = process_count > 0
        && state
            .snapshot
            .processes
            .iter()
            .all(|p| p.private_working_set_mb.is_some());
    let displayed_memory_mb = |process: &windows::ProcessRow| {
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

    // Assemble the per-process JSON straight into one buffer (comma-separated) rather than building
    // a String per process into a Vec and then joining: this path runs on every dashboard poll and
    // scales with the Spotify process count, so avoiding the O(n) intermediate allocations matters.
    let mut targets_buf = String::new();
    for (p, role) in targets.iter() {
        let record = state.active_caps.get(&p.pid).filter(|r| r.creation == p.creation);
        let profile = state.profiles.get(&p.pid).filter(|profile| profile.creation == p.creation);
        let capped = record.is_some();
        let current_cap = record
            .and_then(|r| r.applied_maximum_mb)
            .or_else(|| profile.map(|p| p.current_cap_mb));
        let default_cap = role.manual_default_cap_mb();
        let adaptive_status = profile.map(|p| p.last_reason.as_str()).unwrap_or("baseline");
        // Parse the command line once per process per poll: the sub-label feeds both the reported
        // field and the identity key, so identity is rebuilt from the sub-label we already have rather
        // than re-parsing it, and the manual-cap lookup is keyed on that identity directly.
        let sub_label = process_sub_label(&p.cmd, *role);
        let identity = identity_from_parts(*role, sub_label.as_deref());
        let manual_cap = manual_cap_for_identity(&state.settings, &identity, *role);
        // Learned cross-session usage profile for this identity (local-only telemetry): the EWMA
        // score and the App-Standby-style bucket it falls in, so the dashboard can show "Perfil
        // aprendido". Null until this identity has been sampled long enough to persist a score.
        let learned = state.settings.usage_scores.get(&identity).copied();
        let bucket = learned.map(|score| memory_sentry_core::usage_bucket(score as f64).label());
        if !targets_buf.is_empty() {
            targets_buf.push(',');
        }
        let _ = write!(
            targets_buf,
            "{{\"pid\":{},\"creation\":{},\"role\":{},\"subLabel\":{},\"identity\":{},\"workingSetMB\":{},\"privateWorkingSetMB\":{},\"privateMB\":{},\"cpuPercent\":{},\"pageFaultCount\":{},\"pageFaultsDelta\":{},\"pageFaultsPerSec\":{},\"errorCode\":{},\"capped\":{},\"manual\":{},\"included\":true,\"limitMB\":{},\"manualCapMB\":{},\"defaultLimitMB\":{},\"adaptiveStatus\":{},\"usageScore\":{},\"usageBucket\":{}}}",
            p.pid, json_quote(&p.creation.to_string()), json_quote(role_name(*role)),
            json_optional_string(sub_label.as_deref()),
            json_quote(&identity),
            json_optional_number(p.working_set_mb),
            json_optional_number(p.private_working_set_mb),
            json_optional_number(p.private_mb),
            json_optional_number(p.cpu_percent),
            json_optional_u64(p.page_fault_count),
            json_optional_u64(p.page_faults_delta),
            json_optional_number(p.page_faults_per_sec),
            p.error_code,
            capped,
            record.map(|r| r.manual).unwrap_or(false),
            current_cap.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            manual_cap.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            default_cap,
            json_quote(adaptive_status),
            learned.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            json_optional_string(bucket)
        );
    }

    let mut events_buf = String::new();
    for e in state.recent_events.iter().rev() {
        if !events_buf.is_empty() {
            events_buf.push(',');
        }
        let _ = write!(
            events_buf,
            "{{\"time\":{},\"level\":{},\"message\":{}}}",
            json_quote(&e.time),
            json_quote(&e.level),
            json_quote(&e.message)
        );
    }

    let last_action = state
        .last_action
        .as_ref()
        .map(|action| {
            let mut failures_buf = String::new();
            for failure in action.failures.iter() {
                if !failures_buf.is_empty() {
                    failures_buf.push(',');
                }
                let _ = write!(failures_buf, "{}", json_quote(failure));
            }
            format!(
                "{{\"time\":{},\"reason\":{},\"applied\":{},\"processes\":[],\"failures\":[{}]}}",
                json_quote(&action.time),
                json_quote(&action.reason),
                action.applied,
                failures_buf
            )
        })
        .unwrap_or_else(|| "null".to_string());

    let hidden_seconds = if state.hidden_since_ms > 0 {
        now_ms().saturating_sub(state.hidden_since_ms) / 1000
    } else {
        0
    };
    let (sys_total_mb, sys_avail_mb, mem_load) = windows::system_memory();
    let memory = format!(
        "{{\"spotifyTotalMB\":{},\"targetTotalMB\":{},\"processCount\":{},\"targetCount\":{},\"cappedCount\":{},\"privateWorkingSet\":{},\"systemTotalMB\":{},\"systemAvailableMB\":{},\"memoryLoadPercent\":{},\"pressureLevel\":{},\"targets\":[{}],\"sampledAt\":{}}}",
        json_num(spotify_total_mb), json_num(target_total_mb), process_count, targets.len(), capped_count,
        private_working_set_available, json_num(sys_total_mb as f64), json_num(sys_avail_mb as f64), json_num(mem_load as f64), json_quote(pressure_level(mem_load, sys_avail_mb, sys_total_mb)), targets_buf, json_quote(&iso_now())
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
        "{{\"ok\":true,\"helper\":{{\"running\":true,\"version\":{},\"node\":\"Server\",\"runtime\":\"Server\",\"startedAt\":{}}},\"spotify\":{{\"found\":{},\"rootPid\":{},\"windowTitle\":{},\"windowState\":{},\"windowStateReason\":{},\"hiddenSeconds\":{}}},\"config\":{},\"memory\":{},\"lastAction\":{},\"telemetry\":{},\"lastError\":{},\"recentEvents\":[{}],\"cap\":{{\"active\":{},\"cappedProcesses\":{},\"adaptive\":true,\"minMB\":{},\"maxMB\":{}}}}}",
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
        events_buf,
        !state.active_caps.is_empty(),
        capped_count,
        MIN_CAP_MB,
        MAX_CAP_MB
    )
}

pub(crate) fn json_quote(value: &str) -> String {
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

pub(crate) fn json_num(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.3}")
    } else {
        "0".to_string()
    }
}
pub(crate) fn json_optional_number(value: Option<f64>) -> String {
    value
        .filter(|v| v.is_finite())
        .map(json_num)
        .unwrap_or_else(|| "null".to_string())
}
pub(crate) fn json_optional_u64(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
}
pub(crate) fn json_optional_string(value: Option<&str>) -> String {
    value.map(json_quote).unwrap_or_else(|| "null".to_string())
}

pub(crate) struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: String,
}

pub(crate) fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
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

pub(crate) fn trusted_origin(origin: &str) -> bool {
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

pub(crate) fn constant_time_equals(left: &str, right: &str) -> bool {
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

pub(crate) fn parse_bearer(value: &str) -> Option<&str> {
    let (scheme, token) = value.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
}

pub(crate) fn write_http_response(
    mut stream: TcpStream,
    status: u16,
    reason: &str,
    body: &str,
    origin: Option<&str>,
) {
    let cors = origin.filter(|value| trusted_origin(value)).map(|value| format!(
        "Access-Control-Allow-Origin: {value}\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Authorization, Content-Type\r\nAccess-Control-Allow-Private-Network: true\r\nAccess-Control-Max-Age: 300\r\nVary: Origin\r\n"
    )).unwrap_or_default();
    // The response head and body go out as two writes instead of one formatted copy of the whole
    // exchange: the payload is rebuilt per poll and copying ~15 KB into a second String only to
    // write it once is pure transient allocation. The bytes on the wire are identical.
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nContent-Length: {}\r\n{}\r\n",
        body.len(), cors
    );
    let _ = stream.write_all(head.as_bytes());
    if !body.is_empty() {
        let _ = stream.write_all(body.as_bytes());
    }
    let _ = stream.flush();
}

pub(crate) fn live_target_for_identity(
    state: &AppState,
    identity: &str,
) -> Option<(windows::ProcessRow, ProcessRole)> {
    if !state.snapshot_ok
        || state.snapshot_at_ms == 0
        || now_ms().saturating_sub(state.snapshot_at_ms) > SNAPSHOT_STALE_MS
    {
        return None;
    }
    descendants(state)
        .into_iter()
        .find(|(process, role)| process_identity(&process.cmd, *role) == identity)
}

pub(crate) fn set_manual_cap(state: &mut AppState, body: &str) -> Result<(), String> {
    if !state.settings.enabled {
        return Err("Enable Memory Sentry before setting a manual process limit.".to_string());
    }
    let identity = parse_json_string(body, "identity")
        .filter(|value| !value.is_empty())
        .ok_or("A valid process identity is required.")?;
    let cap_mb = parse_json_number(body, "capMB")
        .and_then(|value| u32::try_from(value).ok())
        .ok_or("A whole-number limit is required.")?;
    if !(MIN_MANUAL_CAP_MB..=MAX_MANUAL_CAP_MB).contains(&cap_mb) {
        return Err(format!(
            "Manual limits must be between {MIN_MANUAL_CAP_MB} and {MAX_MANUAL_CAP_MB} MB."
        ));
    }
    // Persist the pin by IDENTITY first: it is the source of truth and survives process restarts.
    let previous = state.settings.manual_role_caps.insert(identity.clone(), cap_mb);
    if let Err(error) = write_settings(&state.settings_path, &state.settings) {
        if let Some(previous) = previous {
            state.settings.manual_role_caps.insert(identity, previous);
        } else {
            state.settings.manual_role_caps.remove(&identity);
        }
        return Err(error);
    }
    // Best-effort live apply for instant feedback. The monitor loop re-applies every identity pin
    // each poll, so a transient failure (e.g. a protected process that cannot be opened) is NOT an
    // error here - the limit simply takes effect on a later pass instead of a red error toast.
    if let Some((process, role)) = live_target_for_identity(state, &identity) {
        if let Err(error) = apply_manual_cap(state, &process, role, cap_mb) {
            log_event(
                state,
                format!("Limit for {identity} saved; live apply deferred: {error}"),
                "info",
            );
        }
    }
    Ok(())
}

pub(crate) fn clear_manual_cap(state: &mut AppState, body: &str) -> Result<(), String> {
    if !state.settings.enabled {
        return Err("Enable Memory Sentry before clearing a manual process limit.".to_string());
    }
    let identity = parse_json_string(body, "identity")
        .filter(|value| !value.is_empty())
        .ok_or("A valid process identity is required.")?;
    let previous_identity_cap = state.settings.manual_role_caps.remove(&identity);
    if previous_identity_cap.is_none() {
        return Ok(());
    }
    if let Err(error) = write_settings(&state.settings_path, &state.settings) {
        if let Some(previous) = previous_identity_cap {
            state.settings.manual_role_caps.insert(identity, previous);
        }
        return Err(error);
    }
    // Hand the live process back to the adaptive engine now (best-effort); the loop reconciles too.
    if let Some((process, _role)) = live_target_for_identity(state, &identity) {
        let record = state
            .active_caps
            .get(&process.pid)
            .filter(|r| r.creation == process.creation && r.manual)
            .cloned();
        if let Some(record) = record {
            if let Err(error) = restore_one_record(state, &record, "custom pin cleared") {
                log_event(
                    state,
                    format!("Limit for {identity} cleared; live restore deferred: {error}"),
                    "info",
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn update_config(state: &mut AppState, body: &str) -> Result<(), String> {
    let mut next = state.settings.clone();
    if let Some(value) = parse_json_bool(body, "enabled") {
        next.enabled = value;
    }
    if let Some(value) = parse_json_bool(body, "autoOptimize") {
        next.auto_optimize = value;
    }
    if let Some(value) = parse_json_bool(body, "customMode") {
        next.custom_mode = value && next.enabled;
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
    next.hidden_delay_seconds = parse_json_number_f64(body, "hiddenDelaySeconds")
        .unwrap_or(next.hidden_delay_seconds)
        .clamp(0.5, 60.0);
    next.refresh_interval_ms = parse_json_number(body, "refreshIntervalMs")
        .unwrap_or(next.refresh_interval_ms)
        .clamp(250, 10_000);

    let policy_changed = next.auto_optimize != state.settings.auto_optimize
        || next.custom_mode != state.settings.custom_mode;
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

pub(crate) fn handle_http_request(stream: TcpStream, shared: SharedState) {
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
            // Stamp the watch signal so the monitor loop boosts sampling while the dashboard is open.
            state.last_status_request_ms = now_ms();
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

pub(crate) fn start_api(shared: SharedState) -> Result<(), String> {
    let port = shared
        .lock()
        .map_err(|_| "State lock poisoned")?
        .settings
        .port;
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|error| format!("Could not bind local API on 127.0.0.1:{port}: {error}"))?;
    // Blocking accept instead of a 20 ms nonblocking poll: the old loop woke ~50 times a second
    // forever (and took the state lock on every wake) just to look for a connection and re-check
    // the shutdown flag. A parked accept() costs nothing until one arrives, and the shutdown path
    // never depended on this thread: the handler sets shutdown_requested, the monitor loop breaks
    // within its 100 ms cadence, and process exit reaps the parked accept (which also unbinds the
    // port, as the old break did, a few milliseconds later at exit).
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let clone = Arc::clone(&shared);
                    // Modest explicit stack: the handler is shallow (no recursion) and every payload
                    // lives on the heap, so each transient connection thread reserves less memory.
                    let _ = thread::Builder::new()
                        .stack_size(512 * 1024)
                        .spawn(move || handle_http_request(stream, clone));
                }
                // Transient accept failures should not busy-spin the loop in place of the old poll.
                Err(_) => thread::sleep(Duration::from_millis(50)),
            }
        }
    });
    Ok(())
}

#[cfg(windows)]
pub(crate) fn request_status(port: u16, token: &str) -> Result<(), String> {
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

