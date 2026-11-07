//! Persistence and event-log layer: on-disk settings, the legacy-layout migration, the JSON/TOML-ish
//! parsers, the recovery-backup file, and the rolling event log. These operations read and write
//! the data directory and reuse the shared state, types and constants from the parent module.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use super::*;

/// Copy the current rc single-directory data into the split data directory. Removal of the old
/// folder is owned by the MSI's RemoveLegacyFolder custom action (which also clears the stale
/// Inno uninstall registry entry), so this only moves data and never deletes. Already-migrated
/// installs and unsupported 0.* Node recovery are skipped.
pub(crate) fn migrate_legacy_layout() -> Result<(), String> {
    let old = install_dir()?;
    let data = data_dir()?;
    if old == data || !old.is_dir() || data.join("settings.json").is_file() {
        return Ok(());
    }
    fs::create_dir_all(&data).map_err(|e| format!("Could not create the data directory: {e}"))?;
    for name in ["settings.json", "working-set-backup.tsv", "helper.pid"] {
        let from = old.join(name);
        if from.is_file() {
            fs::copy(&from, data.join(name)).map_err(|e| format!("Could not migrate {name}: {e}"))?;
        }
    }
    let old_logs = old.join("logs");
    if old_logs.is_dir() {
        let new_logs = data.join("logs");
        fs::create_dir_all(&new_logs).map_err(|e| e.to_string())?;
        if let Ok(entries) = fs::read_dir(&old_logs) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name() {
                        let _ = fs::copy(&path, new_logs.join(name));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_json_string(text: &str, key: &str) -> Option<String> {
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

pub(crate) fn parse_json_number(text: &str, key: &str) -> Option<u64> {
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

pub(crate) fn parse_json_number_f64(text: &str, key: &str) -> Option<f64> {
    let marker = format!("\"{key}\"");
    let pos = text.find(&marker)? + marker.len();
    let tail = text[pos..].trim_start().strip_prefix(':')?.trim_start();
    let end = tail
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(tail.len());
    if end == 0 {
        return None;
    }
    tail[..end].parse().ok()
}

pub(crate) fn parse_json_bool(text: &str, key: &str) -> Option<bool> {
    let marker = format!("\"{key}\"");
    let pos = text.find(&marker)? + marker.len();
    let tail = text[pos..].trim_start().strip_prefix(':')?.trim_start();
    let (literal, value) = if tail.starts_with("true") {
        ("true", true)
    } else if tail.starts_with("false") {
        ("false", false)
    } else {
        return None;
    };
    // End the literal at a JSON boundary so a hand typo (tru x, truex) is rejected instead of
    // partially parsed; quoted values were always a parse miss, as with the number helper.
    match tail.as_bytes().get(literal.len()) {
        None | Some(b' ' | b'\t' | b'\r' | b'\n' | b',' | b'}' | b']') => Some(value),
        Some(_) => None,
    }
}

pub(crate) fn parse_json_string_array(text: &str, key: &str) -> Option<Vec<String>> {
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

pub(crate) fn parse_manual_role_caps(text: &str) -> BTreeMap<String, u32> {
    parse_json_string_array(text, "manualRoleCaps")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            // Keys are per-process identities that may contain ':' (e.g. `Renderer:#7`), so the
            // cap is the LAST segment; the leading role prefix must still be a known role.
            let (identity, cap) = entry.rsplit_once(':')?;
            let cap = cap.parse::<u32>().ok()?;
            if !identity_role_prefix_ok(identity) || !(MIN_MANUAL_CAP_MB..=MAX_MANUAL_CAP_MB).contains(&cap) {
                return None;
            }
            Some((identity.to_string(), cap))
        })
        .collect()
}

pub(crate) fn parse_auto_role_caps(text: &str) -> BTreeMap<String, u32> {
    parse_json_string_array(text, "autoRoleCaps")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let (identity, cap) = entry.rsplit_once(':')?;
            let cap = cap.parse::<u32>().ok()?;
            if !identity_role_prefix_ok(identity) || !(MIN_CAP_MB..=MAX_CAP_MB).contains(&cap) {
                return None;
            }
            Some((identity.to_string(), cap))
        })
        .collect()
}

/// Cross-session usage telemetry: a per-identity busy-fraction score (0..=100). Serialized as a flat
/// array of "identity:score" strings (mirrors manualRoleCaps - the identity may contain ':', so the
/// score is the LAST segment). The role prefix must be a known role and the score must be 0..=100,
/// so a hand-edited file cannot poison the map; anything else is dropped.
pub(crate) fn parse_usage_scores(text: &str) -> BTreeMap<String, u32> {
    parse_json_string_array(text, "usageScores")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let (identity, score) = entry.rsplit_once(':')?;
            let score = score.parse::<u32>().ok()?;
            if !identity_role_prefix_ok(identity) || score > 100 {
                return None;
            }
            Some((identity.to_string(), score))
        })
        .collect()
}

/// Learned safe edge per identity: the lowest ceiling that did not make the process thrash. Serialized
/// like autoRoleCaps (flat "identity:cap" strings; identity may contain ':', so the cap is the LAST
/// segment). Values outside the automatic cap range are dropped so a hand-edited file cannot freeze a
/// process above its role maximum or push it below the absolute floor.
pub(crate) fn parse_safe_floors(text: &str) -> BTreeMap<String, u32> {
    parse_json_string_array(text, "safeFloors")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let (identity, cap) = entry.rsplit_once(':')?;
            let cap = cap.parse::<u32>().ok()?;
            if !identity_role_prefix_ok(identity) || !(MIN_CAP_MB..=MAX_CAP_MB).contains(&cap) {
                return None;
            }
            Some((identity.to_string(), cap))
        })
        .collect()
}

pub(crate) fn load_settings(path: &Path) -> Result<Settings, String> {
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
    let custom_mode = parse_json_bool(&raw, "customMode").unwrap_or(false);
    // Custom is now the only active mode: whenever the plugin is enabled, run the adaptive engine
    // with per-process identity pins available. Coerce legacy Automatic/Manual settings on load.
    let (auto_optimize, custom_mode) = if enabled { (true, true) } else { (auto_optimize, custom_mode) };
    let hidden_delay_seconds = parse_json_number_f64(&raw, "hiddenDelaySeconds")
        .unwrap_or(DEFAULT_HIDDEN_DELAY_SECONDS)
        .clamp(0.5, 60.0);
    let refresh_interval_ms = parse_json_number(&raw, "refreshIntervalMs")
        .unwrap_or(DEFAULT_REFRESH_INTERVAL_MS)
        .clamp(250, 10_000);
    let trim_to_cap = parse_json_bool(&raw, "trimToCap").unwrap_or(true);
    let optimize_when_background = parse_json_bool(&raw, "optimizeWhenBackground").unwrap_or(true);
    let optimize_when_minimized = parse_json_bool(&raw, "optimizeWhenMinimized").unwrap_or(true);
    let optimize_when_hidden = parse_json_bool(&raw, "optimizeWhenHidden").unwrap_or(true);
    let manual_role_caps = parse_manual_role_caps(&raw);
    let auto_role_caps = parse_auto_role_caps(&raw);
    let usage_scores = parse_usage_scores(&raw);
    let safe_floors = parse_safe_floors(&raw);
    Ok(Settings {
        port,
        token,
        enabled,
        auto_optimize,
        custom_mode,
        hidden_delay_seconds,
        refresh_interval_ms,
        trim_to_cap,
        optimize_when_background,
        optimize_when_minimized,
        optimize_when_hidden,
        manual_role_caps,
        auto_role_caps,
        usage_scores,
        safe_floors,
        schema_version: CURRENT_SCHEMA_VERSION,
    })
}

pub(crate) fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("json.tmp");
    let manual_role_caps = settings
        .manual_role_caps
        .iter()
        .map(|(role, cap)| json_quote(&format!("{role}:{cap}")))
        .collect::<Vec<_>>()
        .join(", ");
    let auto_role_caps = settings
        .auto_role_caps
        .iter()
        .map(|(role, cap)| json_quote(&format!("{role}:{cap}")))
        .collect::<Vec<_>>()
        .join(", ");
    let usage_scores = settings
        .usage_scores
        .iter()
        .map(|(identity, score)| json_quote(&format!("{identity}:{score}")))
        .collect::<Vec<_>>()
        .join(", ");
    let safe_floors = settings
        .safe_floors
        .iter()
        .map(|(identity, cap)| json_quote(&format!("{identity}:{cap}")))
        .collect::<Vec<_>>()
        .join(", ");
    let contents = format!(
        "{{\n  \"port\": {},\n  \"token\": {},\n  \"enabled\": {},\n  \"autoOptimize\": {},\n  \"customMode\": {},\n  \"hiddenDelaySeconds\": {},\n  \"refreshIntervalMs\": {},\n  \"trimToCap\": {},\n  \"optimizeWhenBackground\": {},\n  \"optimizeWhenMinimized\": {},\n  \"optimizeWhenHidden\": {},\n  \"manualRoleCaps\": [{}],\n  \"autoRoleCaps\": [{}],\n  \"usageScores\": [{}],\n  \"safeFloors\": [{}],\n  \"schemaVersion\": {},\n  \"version\": \"{}\"\n}}\n",
        settings.port, json_quote(&settings.token), settings.enabled, settings.auto_optimize, settings.custom_mode, settings.hidden_delay_seconds, settings.refresh_interval_ms, settings.trim_to_cap, settings.optimize_when_background, settings.optimize_when_minimized, settings.optimize_when_hidden, manual_role_caps, auto_role_caps, usage_scores, safe_floors, settings.schema_version, VERSION
    );
    let mut file = File::create(&temp).map_err(|e| format!("Could not stage settings: {e}"))?;
    file.write_all(contents.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    replace_file(&temp, path)?;
    Ok(())
}

/// Replace an unusable settings file with fresh defaults so boot can continue. The bad
/// file is preserved as `settings.json.bad`; the only fatal failure is the OS RNG itself.
pub(crate) fn bootstrap_default_settings(path: &Path) -> Result<Settings, String> {
    if path.exists() {
        let bad = path.with_extension("json.bad");
        let _ = fs::remove_file(&bad);
        let _ = fs::rename(path, &bad);
    }
    let settings = Settings { token: windows::random_token()?, ..default_settings() };
    write_settings(path, &settings)?;
    Ok(settings)
}

pub(crate) fn replace_file(temp: &Path, target: &Path) -> Result<(), String> {
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

pub(crate) fn log_event(state: &mut AppState, message: impl Into<String>, level: &str) {
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
    rotate_log_if_large(&state.log_path);
    if let Some(parent) = state.log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Every app log grows one line at a time and never gets read back for correctness, so all logs
/// share the same cap as helper.log: when a file passes 512 KB it is renamed beside itself as
/// `<stem>-<ms>.previous.log` and the next append starts a fresh file. Best effort: rotation
/// failures are ignored because logging must never break the engine.
pub(crate) fn rotate_log_if_large(path: &Path) {
    let limit = 512 * 1024;
    if fs::metadata(path)
        .map(|m| m.len() > limit)
        .unwrap_or(false)
    {
        if let (Some(parent), Some(stem)) = (path.parent(), path.file_stem()) {
            let shifted = parent.join(format!(
                "{}-{}.previous.log",
                stem.to_string_lossy(),
                now_ms()
            ));
            let _ = fs::rename(path, &shifted);
        }
    }
}

pub(crate) fn load_backup_file(path: &Path) -> Vec<BackupRecord> {
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

pub(crate) fn persist_backups(state: &AppState) -> Result<(), String> {
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
                .replace(['\t', '\r', '\n'], " "),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("mss-rotate-{}-{}-{}", name, std::process::id(), now_ms()));
        fs::create_dir_all(&dir).expect("test dir");
        dir
    }

    #[test]
    fn large_logs_rotate_beside_themselves_and_small_ones_stay() {
        let dir = unique_dir(&format!("test-{}", now_ms()));
        let big = dir.join("mss-rotate-big.log");
        fs::write(&big, "x".repeat(600 * 1024)).expect("test log");
        rotate_log_if_large(&big);
        let shifted = fs::read_dir(&dir)
            .expect("dir")
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("mss-rotate-big-")
                && e.file_name().to_string_lossy().ends_with(".previous.log"));
        assert!(shifted, "a rotated previous log must exist");
        assert!(fs::metadata(&big).is_err(), "original must be renamed away");

        let small = dir.join("mss-rotate-small.log");
        fs::write(&small, "small").expect("test log");
        rotate_log_if_large(&small);
        assert!(fs::metadata(&small).is_ok(), "small files keep their name");

        let _ = fs::remove_dir_all(dir);
    }
}

