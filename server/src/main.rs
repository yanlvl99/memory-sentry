//! Command-line entry point for the Memory Sentry server.
//!
//! Windows 10 and Windows 11 desktop editions only. The platform-independent
//! policy remains testable cross-platform, but Win32 operations fail closed off Windows.

#[cfg(windows)]
#[path = "platform/windows.rs"]
mod platform;
#[cfg(windows)]
mod runtime;

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
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_dir.join("setup.log")) else { return; };
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
        println!("Memory Sentry core self-test passed. This command does not change process memory.");
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
            [command] if command == "--check-os" => runtime::check_supported_os(),
            [command] if command == "--check-install" => runtime::check_install_prerequisites(),
            [command] if command == "--check-spicetify" => runtime::check_spicetify_cli(),
            [command] if command == "--prepare-upgrade" => runtime::prepare_upgrade(),
            [command] if command == "--remove-previous" => runtime::remove_previous_version(),
            [] => runtime::run_monitor(),
            [command] if command == "--monitor" => runtime::run_monitor(),
            [command] if command == "--launch" => runtime::launch_monitor(),
            [command] if command == "--pre-uninstall" => runtime::pre_uninstall(),
            [command] if command == "--restore-limits" => runtime::restore_limits_command(),
            [command, app_dir] if command == "--configure" => runtime::configure_install(app_dir),
            [command, app_dir] if command == "--msi-install" => runtime::msi_install(app_dir),
            [command, new_exe] if command == "--swap" => runtime::swap_server(new_exe),
            _ => Err("Unknown arguments. Supported commands: --version, --self-test, --check-os, --check-install, --check-spicetify, --monitor, --launch, --prepare-upgrade, --remove-previous, --swap <new-server-exe>, --configure <custom-app-folder>, --pre-uninstall, --restore-limits.".to_string()),
        };
        if let Err(error) = result {
            let first = args.first().map(String::as_str);
            if first == Some("--configure") || first == Some("--msi-install") || first == Some("--remove-previous") {
                log_setup_failure(&error);
            }
            eprintln!("Memory Sentry: {error}");
            std::process::exit(1);
        }
    }
}
