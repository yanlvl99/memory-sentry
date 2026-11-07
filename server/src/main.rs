//! Command-line entry point for the Memory Sentry server.
//!
//! Windows 10 and Windows 11 desktop editions only. The platform-independent
//! policy remains testable cross-platform, but Win32 operations fail closed off Windows.

#[cfg(windows)]
mod Runtime;

use memory_sentry_core::{
    ADJUSTMENT_COOLDOWN_MS, MemoryTier, decide_adjustment, faults_since, memory_tier,
    DemandHistogram, MemoryPressure, PolicySample, ProcessRole,
};

#[cfg(windows)]
fn log_setup_failure(error: &str) {
    use std::fs::OpenOptions;
    use std::io::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    let Some(local) = std::env::var_os("LOCALAPPDATA") else { return; };
    let log_dir = std::path::PathBuf::from(local)
        .join("MemorySentryForSpicetify")
        .join("logs");
    if std::fs::create_dir_all(&log_dir).is_err() { return; }
    let log_path = log_dir.join("setup.log");
    Runtime::rotate_log_if_large(&log_path);
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) else { return; };
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let _ = writeln!(file, "[{timestamp}] [SETUP] {error}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().map(String::as_str) == Some("--version") {
        println!("Memory Sentry server {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.first().map(String::as_str) == Some("--self-test") {
        match run_self_test() {
            Ok(()) => println!("Memory Sentry core self-test passed. This command does not change process memory."),
            Err(error) => {
                eprintln!("Memory Sentry core self-test failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    #[cfg(not(windows))]
    {
        let _ = args;
        eprintln!("Memory Sentry supports Windows 10 and Windows 11 desktop editions only.");
        std::process::exit(2);
    }

    #[cfg(windows)]
    {
        let result = match args.as_slice() {
            [command] if command == "--check-os" => Runtime::check_supported_os(),
            [] => Runtime::run_monitor(),
            [command] if command == "--monitor" => Runtime::run_monitor(),
            [command] if command == "--launch" => Runtime::launch_monitor(),
            [command] if command == "--pre-uninstall" => Runtime::pre_uninstall(),
            [command] if command == "--restore-limits" => Runtime::restore_limits_command(),
            [command, app_dir] if command == "--configure" => Runtime::configure_install(app_dir),
            [command, new_exe] if command == "--swap" => Runtime::swap_server(new_exe),
            _ => Err("Unknown arguments. Supported commands: --version, --self-test, --check-os, --monitor, --launch, --swap <new-server-exe>, --configure <custom-app-folder>, --pre-uninstall, --restore-limits.".to_string()),
        };
        if let Err(error) = result {
            let first = args.first().map(String::as_str);
            if first == Some("--configure") {
                log_setup_failure(&error);
            }
            eprintln!("Memory Sentry: {error}");
            std::process::exit(1);
        }
    }
}

fn run_self_test() -> Result<(), String> {
    // --- DemandHistogram: observe + percentile_mb ---
    let mut hist = DemandHistogram::default();
    for mb in [10.0, 20.0, 30.0, 40.0, 50.0] {
        hist.observe(mb);
    }
    let p50 = hist.percentile_mb(50.0).ok_or("percentile_mb returned None")?;
    if !(30.0..=40.0).contains(&p50) {
        return Err(format!("DemandHistogram p50 = {p50}, expected 30-40"));
    }

    // --- faults_since: normal and 32-bit wraparound ---
    if faults_since(100, 150) != 50 {
        return Err("faults_since normal delta wrong".to_string());
    }
    // 32-bit counter wrapped: previous near max, current small
    if faults_since(u32::MAX as u64 - 100, 100) != 201 {
        return Err("faults_since wraparound delta wrong".to_string());
    }

    // --- decide_adjustment: valid input returns Some ---
    let valid = PolicySample {
        role: ProcessRole::Utility,
        current_cap_mb: 24,
        working_set_mb: 20.0,
        cpu_percent: 5.0,
        busy_percent: 5.0,
        cpu_ewma: 5.0,
        faults_per_sec: 0.0,
        demand_p95_mb: 30.0,
        pressure: MemoryPressure::Normal,
        minimum_cap_mb: 4,
        system_total_mb: 16384,
        last_adjustment_ms: 0,
        now_ms: ADJUSTMENT_COOLDOWN_MS,
    };
    let adj = decide_adjustment(valid).ok_or("decide_adjustment returned None for valid input")?;
    if adj.next_cap_mb == 0 {
        return Err("decide_adjustment returned zero cap".to_string());
    }

    // --- decide_adjustment: invalid input returns None ---
    let mut invalid = valid.clone();
    invalid.working_set_mb = f64::NAN;
    if decide_adjustment(invalid).is_some() {
        return Err("decide_adjustment should reject NaN working set".to_string());
    }
    let mut invalid = valid.clone();
    invalid.current_cap_mb = 0; // below role minimum
    if decide_adjustment(invalid).is_some() {
        return Err("decide_adjustment should reject cap below minimum".to_string());
    }
    let mut invalid = valid.clone();
    invalid.now_ms = 0; // inside cooldown
    if decide_adjustment(invalid).is_some() {
        return Err("decide_adjustment should reject cooldown".to_string());
    }

    // --- memory_tier: known RAM sizes ---
    if memory_tier(4096) != MemoryTier::Small {
        return Err("memory_tier 4GB should be Small".to_string());
    }
    if memory_tier(16384) != MemoryTier::Medium {
        return Err("memory_tier 16GB should be Medium".to_string());
    }
    if memory_tier(65536) != MemoryTier::Large {
        return Err("memory_tier 64GB should be Large".to_string());
    }

    Ok(())
}
