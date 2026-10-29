//! Win32 process, memory, window-state and installation primitives.
//! All operations validate the process creation timestamp immediately before mutation.

use std::ffi::c_void;
use std::path::Path;
use std::process::Command;

pub const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
pub const PROCESS_SET_QUOTA: u32 = 0x0100;
pub const QUOTA_LIMITS_HARDWS_MIN_DISABLE: u32 = 0x0000_0002;
pub const QUOTA_LIMITS_HARDWS_MAX_ENABLE: u32 = 0x0000_0004;
pub const HARD_CAP_FLAGS: u32 = QUOTA_LIMITS_HARDWS_MIN_DISABLE | QUOTA_LIMITS_HARDWS_MAX_ENABLE;
pub const MIN_WORKING_SET_BYTES: usize = 80 * 1024;
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const PROCESS_TERMINATE: u32 = 0x0001;
const ERROR_NO_MORE_FILES: u32 = 18;

type Handle = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn value(self) -> u64 {
        ((self.high as u64) << 32) | self.low as u64
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProcessMemoryCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProcessMemoryCountersEx2 {
    base: ProcessMemoryCounters,
    private_usage: usize,
    private_working_set_size: usize,
    shared_commit_usage: u64,
}

#[repr(C)]
struct RtlOsVersionInfoExW {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform_id: u32,
    service_pack: [u16; 128],
    service_pack_major: u16,
    service_pack_minor: u16,
    suite_mask: u16,
    product_type: u8,
    reserved: u8,
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
    fn SetLastError(error: u32);
    fn GetProcessTimes(
        process: Handle,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn GetProcessWorkingSetSizeEx(
        process: Handle,
        minimum: *mut usize,
        maximum: *mut usize,
        flags: *mut u32,
    ) -> i32;
    fn SetProcessWorkingSetSizeEx(process: Handle, minimum: usize, maximum: usize, flags: u32) -> i32;
    fn GetCurrentProcessId() -> u32;
    fn CreateMutexW(attributes: *mut c_void, initial_owner: i32, name: *const u16) -> Handle;
    fn ReleaseMutex(handle: Handle) -> i32;
    fn QueryFullProcessImageNameW(process: Handle, flags: u32, exe_name: *mut u16, size: *mut u32) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
    fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn GetStdHandle(which: i32) -> Handle;
    fn SetHandleInformation(handle: Handle, mask: u32, flags: u32) -> i32;
}

#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(process: Handle, counters: *mut c_void, size: u32) -> i32;
    fn EmptyWorkingSet(process: Handle) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: Handle, text: *const u16, caption: *const u16, utype: u32) -> i32;
    fn IsWindow(hwnd: Handle) -> i32;
    fn IsWindowVisible(hwnd: Handle) -> i32;
    fn IsIconic(hwnd: Handle) -> i32;
    fn GetForegroundWindow() -> Handle;
    fn GetWindowThreadProcessId(hwnd: Handle, process_id: *mut u32) -> u32;
    fn EnumWindows(callback: Option<unsafe extern "system" fn(Handle, isize) -> i32>, param: isize) -> i32;
    fn GetWindow(hwnd: Handle, relationship: i32) -> Handle;
    fn GetWindowLongW(hwnd: Handle, index: i32) -> i32;
    fn GetWindowTextLengthW(hwnd: Handle) -> i32;
    fn GetWindowTextW(hwnd: Handle, buffer: *mut u16, max_count: i32) -> i32;
    fn GetWindowRect(hwnd: Handle, rect: *mut WindowRect) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: Handle, attribute: u32, value: *mut c_void, size: u32) -> i32;
}

#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(algorithm: Handle, buffer: *mut u8, length: u32, flags: u32) -> i32;
}

#[link(name = "ntdll")]
extern "system" {
    fn RtlGetVersion(info: *mut RtlOsVersionInfoExW) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        rclsid: *const Guid,
        outer: *mut c_void,
        class_context: u32,
        iid: *const Guid,
        out: *mut *mut c_void,
    ) -> i32;
}

#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: i32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
}

// PROCESSENTRY32W is 568 bytes on 64-bit Windows. Process32First rejects a mismatched dwSize,
// so an incorrect layout silently enumerates zero processes and the forced stop becomes a no-op.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<ProcessEntry32W>() == 568);

#[derive(Clone, Debug)]
pub struct ProcessRow {
    pub pid: u32,
    pub ppid: u32,
    pub cmd: String,
    pub creation: u64,
    pub cpu_time_ms: f64,
    pub cpu_percent: Option<f64>,
    pub working_set_mb: Option<f64>,
    pub private_working_set_mb: Option<f64>,
    pub private_mb: Option<f64>,
    pub page_fault_count: Option<u64>,
    pub page_faults_delta: Option<u64>,
    pub error_code: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub processes: Vec<ProcessRow>,
}

#[derive(Clone, Copy, Debug)]
pub struct OriginalLimits {
    pub minimum: usize,
    pub maximum: usize,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessStats {
    pub creation: u64,
    pub cpu_time_ms: f64,
    pub working_set_mb: f64,
    pub private_working_set_mb: Option<f64>,
    pub page_fault_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreOutcome {
    Restored,
    ProcessGone,
    PidReused,
}

fn last_error() -> u32 {
    unsafe { GetLastError() }
}

fn open_process(pid: u32, access: u32) -> Result<Handle, String> {
    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        return Err(format!("OpenProcess({pid}) failed with Win32 error {}", last_error()));
    }
    Ok(handle)
}

fn process_times(handle: Handle) -> Result<(u64, f64), String> {
    let mut creation = FileTime::default();
    let mut exit = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    let ok = unsafe {
        GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user)
    };
    if ok == 0 {
        return Err(format!("GetProcessTimes failed with Win32 error {}", last_error()));
    }
    let cpu_ticks = kernel.value().saturating_add(user.value());
    Ok((creation.value(), cpu_ticks as f64 / 10_000.0))
}

fn process_stats_with_handle(handle: Handle) -> Result<(f64, Option<f64>, u64), String> {
    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        ..ProcessMemoryCounters::default()
    };
    let ok = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters as *mut _ as *mut c_void,
            std::mem::size_of::<ProcessMemoryCounters>() as u32,
        )
    };
    if ok == 0 {
        return Err(format!("GetProcessMemoryInfo failed with Win32 error {}", last_error()));
    }

    let mut extended = ProcessMemoryCountersEx2 {
        base: counters,
        ..ProcessMemoryCountersEx2::default()
    };
    extended.base.cb = std::mem::size_of::<ProcessMemoryCountersEx2>() as u32;
    let extended_ok = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut extended as *mut _ as *mut c_void,
            std::mem::size_of::<ProcessMemoryCountersEx2>() as u32,
        )
    } != 0;
    let working_set_size = if extended_ok {
        extended.base.working_set_size
    } else {
        counters.working_set_size
    };
    let private_working_set_mb = if extended_ok && extended.private_working_set_size > 0 {
        Some(extended.private_working_set_size as f64 / (1024.0 * 1024.0))
    } else {
        None
    };
    Ok((
        working_set_size as f64 / (1024.0 * 1024.0),
        private_working_set_mb,
        counters.page_fault_count as u64,
    ))
}

pub fn inspect_process(pid: u32) -> Result<ProcessStats, String> {
    let handle = open_process(pid, PROCESS_QUERY_INFORMATION)?;
    let result = (|| {
        let (creation, cpu_time_ms) = process_times(handle)?;
        let (working_set_mb, private_working_set_mb, page_fault_count) = process_stats_with_handle(handle)?;
        Ok(ProcessStats { creation, cpu_time_ms, working_set_mb, private_working_set_mb, page_fault_count })
    })();
    unsafe { CloseHandle(handle); }
    result
}

pub fn read_original_limits(pid: u32, expected_creation: u64) -> Result<OriginalLimits, String> {
    let handle = open_process(pid, PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA)?;
    let result = (|| {
        let (creation, _) = process_times(handle)?;
        if creation != expected_creation {
            return Err("Process identity changed before reading working-set limits.".to_string());
        }
        let mut minimum = 0usize;
        let mut maximum = 0usize;
        let mut flags = 0u32;
        let ok = unsafe {
            GetProcessWorkingSetSizeEx(handle, &mut minimum, &mut maximum, &mut flags)
        };
        if ok == 0 {
            return Err(format!("GetProcessWorkingSetSizeEx failed with Win32 error {}", last_error()));
        }
        Ok(OriginalLimits { minimum, maximum, flags })
    })();
    unsafe { CloseHandle(handle); }
    result
}

pub fn set_working_set_limits(
    pid: u32,
    expected_creation: u64,
    minimum: usize,
    maximum: usize,
    flags: u32,
) -> Result<(), String> {
    let handle = open_process(pid, PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA)?;
    let result = (|| {
        let (creation, _) = process_times(handle)?;
        if creation != expected_creation {
            return Err("Process identity changed before setting working-set limits; skipped.".to_string());
        }
        let ok = unsafe { SetProcessWorkingSetSizeEx(handle, minimum, maximum, flags) };
        if ok == 0 {
            return Err(format!("SetProcessWorkingSetSizeEx failed with Win32 error {}", last_error()));
        }
        Ok(())
    })();
    unsafe { CloseHandle(handle); }
    result
}

pub fn empty_working_set(pid: u32, expected_creation: u64) -> Result<(), String> {
    let handle = open_process(pid, PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA)?;
    let result = (|| {
        let (creation, _) = process_times(handle)?;
        if creation != expected_creation {
            return Err("Process identity changed before trimming; skipped.".to_string());
        }
        let ok = unsafe { EmptyWorkingSet(handle) };
        if ok == 0 {
            return Err(format!("EmptyWorkingSet failed with Win32 error {}", last_error()));
        }
        Ok(())
    })();
    unsafe { CloseHandle(handle); }
    result
}

pub fn restore_working_set_limits(
    pid: u32,
    expected_creation: u64,
    minimum: usize,
    maximum: usize,
    flags: u32,
) -> Result<RestoreOutcome, String> {
    let handle = match open_process(pid, PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA) {
        Ok(handle) => handle,
        Err(error) if error.contains("error 87") || error.contains("error 1168") => {
            return Ok(RestoreOutcome::ProcessGone);
        }
        Err(error) => return Err(error),
    };
    let result = (|| {
        let (creation, _) = process_times(handle)?;
        if creation != expected_creation {
            return Ok(RestoreOutcome::PidReused);
        }
        let ok = unsafe { SetProcessWorkingSetSizeEx(handle, minimum, maximum, flags) };
        if ok == 0 {
            return Err(format!("Could not restore PID {pid}: Win32 error {}", last_error()));
        }
        Ok(RestoreOutcome::Restored)
    })();
    unsafe { CloseHandle(handle); }
    result
}

pub fn foreground_process_id() -> Option<u32> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_null() {
        return None;
    }
    let mut process_id = 0;
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, &mut process_id) };
    if thread_id == 0 || process_id == 0 {
        None
    } else {
        Some(process_id)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WindowRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

const GW_OWNER: i32 = 4;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: i32 = 0x0000_0080;
const WS_EX_NOACTIVATE: i32 = 0x0800_0000;
const DWMWA_CLOAKED: u32 = 14;

/// One titled top-level window owned by one of the scanned processes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowCandidate {
    pub pid: u32,
    pub hwnd: usize,
    pub title: String,
    pub on_screen: bool,
    pub iconic: bool,
    pub area: i64,
}

/// The shell can keep WS_VISIBLE on a window it stopped presenting (another virtual
/// desktop, a suppressed UWP or shell surface), so visibility alone is not proof that
/// the user can see the window.
fn is_window_cloaked(window: Handle) -> bool {
    let mut cloaked: i32 = 0;
    unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            &mut cloaked as *mut i32 as *mut c_void,
            std::mem::size_of::<i32>() as u32,
        ) == 0
            && cloaked != 0
    }
}

struct WindowScan {
    pids: Vec<u32>,
    found: Vec<WindowCandidate>,
}

unsafe extern "system" fn scan_top_level_window(window: Handle, param: isize) -> i32 {
    let scan = &mut *(param as *mut WindowScan);
    if IsWindow(window) == 0 {
        return 1;
    }
    let mut pid = 0;
    if GetWindowThreadProcessId(window, &mut pid) == 0 || !scan.pids.contains(&pid) {
        return 1;
    }
    // Owned windows are dialogs, and tool or no-activate windows are notifications,
    // tray icons and invisible Electron support surfaces. None of them is the main
    // Spotify window we are looking for.
    if !GetWindow(window, GW_OWNER).is_null() {
        return 1;
    }
    if GetWindowLongW(window, GWL_EXSTYLE) & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0 {
        return 1;
    }
    let title_length = GetWindowTextLengthW(window);
    if title_length <= 0 {
        return 1;
    }
    let mut buffer = vec![0u16; title_length as usize + 1];
    let written = GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32);
    let title = String::from_utf16_lossy(&buffer[..written.max(0) as usize]);
    let mut rect = WindowRect::default();
    let _ = GetWindowRect(window, &mut rect);
    let area = (rect.right - rect.left).max(0) as i64 * (rect.bottom - rect.top).max(0) as i64;
    // A window without a usable area is a support surface (Spotify keeps a 1x1
    // "GDI+ Window", plus IME and 0x0 helper windows). They can carry a title while
    // never being the app window, and a real window hidden to the tray keeps its last
    // size, so requiring a plausible area separates "hidden" from "leftover garbage".
    const MINIMUM_WINDOW_AREA: i64 = 256;
    if area < MINIMUM_WINDOW_AREA {
        return 1;
    }
    let on_screen = IsWindowVisible(window) != 0 && !is_window_cloaked(window);
    scan.found.push(WindowCandidate {
        pid,
        hwnd: window as usize,
        title,
        on_screen,
        iconic: IsIconic(window) != 0,
        area,
    });
    1
}

/// Every titled top-level window of the given processes, read straight from the window
/// manager. This replaces the PowerShell MainWindowHandle probe: the .NET property only
/// accepts unowned *visible* windows, so it always reports 0 once Spotify hides itself
/// in the tray, and the handle it does return can be destroyed at any moment.
pub fn window_candidates(pids: &[u32]) -> Vec<WindowCandidate> {
    if pids.is_empty() {
        return Vec::new();
    }
    let mut scan = WindowScan {
        pids: pids.to_vec(),
        found: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(scan_top_level_window), &mut scan as *mut WindowScan as isize);
    }
    scan.found.sort_by_key(|window| (window.pid, window.hwnd));
    scan.found
}

/// The window that best represents the process: the largest one the user can see,
/// then the largest minimized one, then the largest hidden one.
fn largest_window(list: &[WindowCandidate]) -> Option<&WindowCandidate> {
    list.iter()
        .filter(|window| window.on_screen && !window.iconic)
        .max_by_key(|window| window.area)
        .or_else(|| {
            list.iter()
                .filter(|window| window.iconic)
                .max_by_key(|window| window.area)
        })
        .or_else(|| list.iter().max_by_key(|window| window.area))
}

/// Window state plus the window that represents it. A minimized window is still
/// visible and carries IsIconic, a tray-hidden window is neither visible nor cloaked
/// but still has a size. "Foreground" additionally requires something of Spotify to be
/// on screen: after a tray hide the shell can hand focus to a leftover window of the
/// same process, and trusting the process id alone would keep reporting foreground.
pub fn classify_windows(
    candidates: &[WindowCandidate],
    is_foreground: bool,
) -> (&'static str, Option<&WindowCandidate>) {
    if candidates.is_empty() {
        return ("window-not-found", None);
    }
    if is_foreground && candidates.iter().any(|window| window.on_screen) {
        return ("foreground", largest_window(candidates));
    }
    if candidates.iter().any(|window| window.on_screen && !window.iconic) {
        return ("background-visible", largest_window(candidates));
    }
    if candidates.iter().any(|window| window.iconic) {
        return ("minimized", largest_window(candidates));
    }
    ("hidden", largest_window(candidates))
}

/// Clear the inherit flag on our stdout/stderr so a detached child we spawn (the monitor) does not
/// inherit the installer custom action's output pipe. Without this, the child keeps the pipe's write
/// end open after the parent exits, so the waiting custom action never sees EOF and the setup hangs.
pub fn detach_stdio_from_children() {
    const STD_OUTPUT_HANDLE: i32 = -11;
    const STD_ERROR_HANDLE: i32 = -12;
    const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
    unsafe {
        for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let handle = GetStdHandle(which);
            if !handle.is_null() && handle as isize != -1 {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

/// Show a modal error dialog with the exact reason. Used by setup-time verbs so a failure surfaces
/// to the user, not only to the installer log. MB_OK | MB_ICONERROR.
pub fn fatal_setup_message(message: &str) {
    let encode = |value: &str| -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    };
    let text = encode(message);
    let caption = encode("Memory Sentry — Setup stopped");
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x0000_0010);
    }
}

pub fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    let status = unsafe {
        BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), bytes.len() as u32, 0x0000_0002)
    };
    if status != 0 {
        return Err(format!("BCryptGenRandom failed with NTSTATUS 0x{:08X}", status as u32));
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn detect_supported_client_os() -> Result<(&'static str, u32), String> {
    let mut info = RtlOsVersionInfoExW {
        size: std::mem::size_of::<RtlOsVersionInfoExW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform_id: 0,
        service_pack: [0; 128],
        service_pack_major: 0,
        service_pack_minor: 0,
        suite_mask: 0,
        product_type: 0,
        reserved: 0,
    };
    let status = unsafe { RtlGetVersion(&mut info) };
    if status != 0 {
        return Err(format!("RtlGetVersion returned NTSTATUS 0x{:08X}", status as u32));
    }
    if info.product_type != 1 {
        return Err("Windows Server editions are not supported".to_string());
    }
    if info.major != 10 || info.minor != 0 || info.build < 10_240 {
        return Err("unsupported Windows version".to_string());
    }
    if info.build >= 22_000 {
        Ok(("Windows 11", info.build))
    } else {
        Ok(("Windows 10", info.build))
    }
}

pub fn powershell_snapshot() -> Result<Snapshot, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = r#"
$ErrorActionPreference = 'Stop'
try {
  [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
  $utf8 = New-Object System.Text.UTF8Encoding($false)
  $culture = [Globalization.CultureInfo]::InvariantCulture
  foreach ($p in @(Get-CimInstance Win32_Process -Filter "Name='Spotify.exe'")) {
    $created = '0'
    try { if ($p.CreationDate) { $created = [string](([datetime]$p.CreationDate).ToFileTimeUtc()) } } catch {}
    $cpuMs = 0.0
    try { $cpuMs = ([double]$p.KernelModeTime + [double]$p.UserModeTime) / 10000.0 } catch {}
    $cmd64 = [Convert]::ToBase64String($utf8.GetBytes([string]$p.CommandLine))
    $ws = 0.0; $private = 0.0; $faults = 0.0
    try { $ws = [double]$p.WorkingSetSize } catch {}
    try { $private = [double]$p.PrivatePageCount } catch {}
    try { $faults = [double]$p.PageFaults } catch {}
    [Console]::Out.WriteLine(('P|{0}|{1}|{2}|{3}|{4}|{5}|{6}|{7}' -f
      $p.ProcessId, $p.ParentProcessId, $created,
      $cpuMs.ToString($culture), $ws.ToString($culture),
      $private.ToString($culture), $faults.ToString($culture), $cmd64))
  }
} catch {
  [Console]::Error.WriteLine($_.Exception.Message)
  exit 2
}
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Could not launch Windows PowerShell: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Process discovery failed: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut snapshot = Snapshot::default();
    for raw in stdout.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        let parts: Vec<&str> = line.split('|').collect();
        match parts.first().copied() {
            Some("P") if parts.len() == 9 => {
                let pid = parts[1].parse::<u32>().unwrap_or(0);
                let ppid = parts[2].parse::<u32>().unwrap_or(0);
                if pid == 0 { continue; }
                let creation_from_wmi = parts[3].parse::<u64>().unwrap_or(0);
                let cpu_from_wmi = parts[4].parse::<f64>().unwrap_or(0.0);
                let ws = parts[5].parse::<f64>().unwrap_or(0.0);
                let private = parts[6].parse::<f64>().unwrap_or(0.0);
                let faults = parts[7].parse::<f64>().unwrap_or(0.0);
                let cmd = String::from_utf8_lossy(&decode_base64(parts[8])).into_owned();
                let mut creation = creation_from_wmi;
                let mut cpu_time_ms = cpu_from_wmi;
                let mut working_set_mb = (ws > 0.0).then_some(ws / (1024.0 * 1024.0));
                let mut private_working_set_mb = None;
                let mut page_fault_count = (parts[7].parse::<u64>().is_ok()).then_some(faults as u64);
                let mut error_code = 0;
                if let Ok(stats) = inspect_process(pid) {
                    // The creation timestamp is the identity later revalidated before every mutation.
                    creation = stats.creation;
                    cpu_time_ms = stats.cpu_time_ms;
                    working_set_mb = Some(stats.working_set_mb);
                    private_working_set_mb = stats.private_working_set_mb;
                    page_fault_count = Some(stats.page_fault_count);
                } else {
                    error_code = -1;
                }
                snapshot.processes.push(ProcessRow {
                    pid, ppid, cmd, creation, cpu_time_ms, cpu_percent: None,
                    working_set_mb, private_working_set_mb,
                    private_mb: (private > 0.0).then_some(private / (1024.0 * 1024.0)),
                    page_fault_count, page_faults_delta: None, error_code,
                });
            }
            _ => {}
        }
    }
    Ok(snapshot)
}

fn decode_base64(input: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => (byte - b'A') as u32,
            b'a'..=b'z' => (byte - b'a' + 26) as u32,
            b'0'..=b'9' => (byte - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => continue,
        };
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((accumulator >> bits) & 0xff) as u8);
        }
    }
    output
}

pub fn process_matches_executable(pid: u32, expected_path: &Path) -> bool {
    let Ok(handle) = open_process(pid, 0x1000) else { return false; };
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length)
    } != 0;
    unsafe { CloseHandle(handle); }
    if !queried || length == 0 || length as usize > buffer.len() { return false; }

    let actual = String::from_utf16_lossy(&buffer[..length as usize]);
    let expected = expected_path.canonicalize().unwrap_or_else(|_| expected_path.to_path_buf());
    let normalize = |value: &str| value.replace('/', "\\").trim_start_matches(r"\\?\").to_ascii_lowercase();
    normalize(&actual) == normalize(&expected.to_string_lossy())
}

/// PIDs of running processes whose image path matches `installed_exe`, excluding this process.
/// A failed snapshot or enumeration is reported as `Err` so it can never masquerade as "nothing
/// to stop".
pub fn installed_process_pids(installed_exe: &Path) -> Result<Vec<u32>, String> {
    let mut pids = Vec::new();
    let self_pid = current_process_id();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() || snapshot as isize == -1 {
        return Err(format!(
            "Could not snapshot the process list (Win32 error {}).",
            last_error()
        ));
    }
    let mut entry = ProcessEntry32W {
        dw_size: std::mem::size_of::<ProcessEntry32W>() as u32,
        cnt_usage: 0,
        th32_process_id: 0,
        th32_default_heap_id: 0,
        th32_module_id: 0,
        cnt_threads: 0,
        th32_parent_process_id: 0,
        pc_pri_class_base: 0,
        dw_flags: 0,
        sz_exe_file: [0; 260],
    };
    if unsafe { Process32FirstW(snapshot, &mut entry) } == 0 {
        let error = last_error();
        unsafe { CloseHandle(snapshot); }
        if error != ERROR_NO_MORE_FILES {
            return Err(format!("Could not enumerate processes (Win32 error {error})."));
        }
        return Ok(pids);
    }
    loop {
        let pid = entry.th32_process_id;
        if pid != self_pid && process_matches_executable(pid, installed_exe) {
            pids.push(pid);
        }
        if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
            break;
        }
    }
    unsafe { CloseHandle(snapshot); }
    Ok(pids)
}

/// Terminate every running instance of the installed server executable, waiting briefly for
/// each to exit. Never touches this process. Returns Err if enumeration fails or a process still
/// survives, so callers can refuse to replace a locked executable.
pub fn force_terminate_installed_process(installed_exe: &Path) -> Result<(), String> {
    for pid in installed_process_pids(installed_exe)? {
        let handle = open_process(pid, PROCESS_TERMINATE)?;
        if unsafe { TerminateProcess(handle, 1) } == 0 {
            let error = last_error();
            unsafe { CloseHandle(handle); }
            return Err(format!("TerminateProcess({pid}) failed with Win32 error {error}"));
        }
        unsafe { WaitForSingleObject(handle, 3000); }
        unsafe { CloseHandle(handle); }
    }
    let survivors = installed_process_pids(installed_exe)?;
    if survivors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Memory Sentry process(es) {:?} survived forced termination.",
            survivors
        ))
    }
}

pub struct SingleInstanceGuard {
    handle: Handle,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

pub fn acquire_single_instance() -> Result<SingleInstanceGuard, String> {
    // The mutex name is a stable cross-version identifier. Do not rename it: an older
    // build that is still running must keep new instances from starting a second monitor.
    let name: Vec<u16> = "Local\\MemorySentry-Companion".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { SetLastError(0); }
    let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 1, name.as_ptr()) };
    if handle.is_null() {
        return Err(format!("Could not create single-instance mutex (Win32 error {}).", last_error()));
    }
    if last_error() == 183 {
        unsafe { CloseHandle(handle); }
        return Err("Memory Sentry is already running for this Windows session.".to_string());
    }
    Ok(SingleInstanceGuard { handle })
}

pub fn current_process_id() -> u32 {
    unsafe { GetCurrentProcessId() }
}

const COINIT_MULTITHREADED: u32 = 0x0000_0000;
const CLSCTX_ALL: u32 = 0x17;
const E_RENDER: i32 = 0;
const DEVICE_STATE_ACTIVE: u32 = 0x0000_0001;
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;
const AUDIO_SESSION_STATE_EXPIRED: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const CLSID_MM_DEVICE_ENUMERATOR: Guid = Guid {
    data1: 0xBCDE_0395,
    data2: 0xE52F,
    data3: 0x467C,
    data4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E],
};
const IID_IMM_DEVICE_ENUMERATOR: Guid = Guid {
    data1: 0xA956_64D2,
    data2: 0x9614,
    data3: 0x4F35,
    data4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6],
};
const IID_IAUDIO_SESSION_MANAGER2: Guid = Guid {
    data1: 0x77AA_99A0,
    data2: 0x1BD6,
    data3: 0x484F,
    data4: [0x8B, 0xC7, 0x2C, 0x65, 0x4C, 0x9A, 0x9B, 0x6F],
};
const IID_IAUDIO_SESSION_CONTROL2: Guid = Guid {
    data1: 0xBFB7_FF88,
    data2: 0x7239,
    data3: 0x4FC9,
    data4: [0x8F, 0xA2, 0x07, 0xC9, 0x50, 0xBE, 0x9C, 0x6D],
};

type QueryInterfaceFn = unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32;
type AddRefFn = unsafe extern "system" fn(*mut c_void) -> u32;
type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;

#[repr(C)]
#[allow(dead_code)]
struct VtableUnknown {
    query_interface: QueryInterfaceFn,
    add_ref: AddRefFn,
    release: ReleaseFn,
}

#[repr(C)]
struct UnknownObject {
    vtable: *const VtableUnknown,
}

#[repr(C)]
#[allow(dead_code)]
struct VtableImmDeviceEnumerator {
    query_interface: QueryInterfaceFn,
    add_ref: AddRefFn,
    release: ReleaseFn,
    enum_audio_endpoints: unsafe extern "system" fn(
        *mut c_void,
        i32,
        u32,
        *mut *mut ImmDeviceCollection,
    ) -> i32,
}

#[repr(C)]
struct ImmDeviceEnumerator {
    vtable: *const VtableImmDeviceEnumerator,
}

#[repr(C)]
#[allow(dead_code)]
struct VtableImmDeviceCollection {
    query_interface: QueryInterfaceFn,
    add_ref: AddRefFn,
    release: ReleaseFn,
    get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    item: unsafe extern "system" fn(*mut c_void, u32, *mut *mut ImmDevice) -> i32,
}

#[repr(C)]
struct ImmDeviceCollection {
    vtable: *const VtableImmDeviceCollection,
}

#[repr(C)]
#[allow(dead_code)]
struct VtableImmDevice {
    query_interface: QueryInterfaceFn,
    add_ref: AddRefFn,
    release: ReleaseFn,
    activate: unsafe extern "system" fn(
        *mut c_void,
        *const Guid,
        u32,
        *mut c_void,
        *mut *mut c_void,
    ) -> i32,
}

#[repr(C)]
struct ImmDevice {
    vtable: *const VtableImmDevice,
}

#[repr(C)]
#[allow(dead_code)]
struct VtableAudioSessionManager2 {
    query_interface: QueryInterfaceFn,
    add_ref: AddRefFn,
    release: ReleaseFn,
    get_audio_session_control: *const c_void,
    get_simple_audio_volume: *const c_void,
    get_session_enumerator:
        unsafe extern "system" fn(*mut c_void, *mut *mut AudioSessionEnumerator) -> i32,
}

#[repr(C)]
struct AudioSessionManager2 {
    vtable: *const VtableAudioSessionManager2,
}

#[repr(C)]
#[allow(dead_code)]
struct VtableAudioSessionEnumerator {
    query_interface: QueryInterfaceFn,
    add_ref: AddRefFn,
    release: ReleaseFn,
    get_count: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
    get_session: unsafe extern "system" fn(*mut c_void, i32, *mut *mut AudioSessionControl) -> i32,
}

#[repr(C)]
struct AudioSessionEnumerator {
    vtable: *const VtableAudioSessionEnumerator,
}

#[repr(C)]
#[allow(dead_code)]
struct VtableAudioSessionControl2 {
    query_interface: QueryInterfaceFn,
    add_ref: AddRefFn,
    release: ReleaseFn,
    get_state: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
    get_display_name: *const c_void,
    set_display_name: *const c_void,
    get_icon_path: *const c_void,
    set_icon_path: *const c_void,
    get_grouping_param: *const c_void,
    set_grouping_param: *const c_void,
    register_audio_session_notification: *const c_void,
    unregister_audio_session_notification: *const c_void,
    get_session_identifier: *const c_void,
    get_session_instance_identifier: *const c_void,
    get_process_id: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
}

#[repr(C)]
struct AudioSessionControl {
    vtable: *const VtableAudioSessionControl2,
}

struct ComPtr<T> {
    ptr: *mut T,
}

impl<T> ComPtr<T> {
    unsafe fn claim(ptr: *mut T) -> Self {
        debug_assert!(!ptr.is_null());
        Self { ptr }
    }

    fn raw(&self) -> *mut c_void {
        self.ptr.cast()
    }
}

impl<T> Drop for ComPtr<T> {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe {
            let object = self.ptr.cast::<UnknownObject>();
            let vtable = (*object).vtable;
            ((*vtable).release)(self.ptr.cast());
        }
    }
}

fn hresult_error(context: &str, hr: i32) -> String {
    format!("{context} failed with HRESULT 0x{:08X}", hr as u32)
}

fn check_hresult(hr: i32, context: &str) -> Result<(), String> {
    if hr < 0 {
        Err(hresult_error(context, hr))
    } else {
        Ok(())
    }
}

unsafe fn query_interface(object: *mut c_void, iid: &Guid) -> Result<*mut c_void, i32> {
    let base = object.cast::<UnknownObject>();
    let vtable = (*base).vtable;
    let mut out: *mut c_void = std::ptr::null_mut();
    let hr = ((*vtable).query_interface)(object, iid, &mut out);
    if hr < 0 {
        return Err(hr);
    }
    Ok(out)
}

unsafe fn collect_device_session_pids(
    device: &ComPtr<ImmDevice>,
    pids: &mut Vec<u32>,
) -> Result<(), String> {
    let mut manager: *mut c_void = std::ptr::null_mut();
    check_hresult(
        ((*(*device.ptr).vtable).activate)(
            device.raw(),
            &IID_IAUDIO_SESSION_MANAGER2,
            CLSCTX_ALL,
            std::ptr::null_mut(),
            &mut manager,
        ),
        "IMMDevice::Activate(IAudioSessionManager2)",
    )?;
    let manager = ComPtr::claim(manager.cast::<AudioSessionManager2>());

    let mut sessions: *mut AudioSessionEnumerator = std::ptr::null_mut();
    check_hresult(
        ((*(*manager.ptr).vtable).get_session_enumerator)(manager.raw(), &mut sessions),
        "IAudioSessionManager2::GetSessionEnumerator",
    )?;
    let sessions = ComPtr::claim(sessions);

    let mut count = 0i32;
    check_hresult(
        ((*(*sessions.ptr).vtable).get_count)(sessions.raw(), &mut count),
        "IAudioSessionEnumerator::GetCount",
    )?;

    for index in 0..count {
        let mut control: *mut AudioSessionControl = std::ptr::null_mut();
        let listed =
            ((*(*sessions.ptr).vtable).get_session)(sessions.raw(), index, &mut control) >= 0;
        if !listed || control.is_null() {
            continue;
        }
        let control = ComPtr::claim(control);
        let control2 = match query_interface(control.raw(), &IID_IAUDIO_SESSION_CONTROL2) {
            Ok(ptr) => ComPtr::claim(ptr.cast::<AudioSessionControl>()),
            Err(_) => continue,
        };
        let mut state = 0i32;
        let queried = ((*(*control2.ptr).vtable).get_state)(control2.raw(), &mut state) >= 0;
        if !queried || state == AUDIO_SESSION_STATE_EXPIRED {
            continue;
        }
        let mut pid = 0u32;
        if ((*(*control2.ptr).vtable).get_process_id)(control2.raw(), &mut pid) < 0 {
            continue;
        }
        if pid != 0 {
            pids.push(pid);
        }
    }
    Ok(())
}

unsafe fn collect_audio_session_pids() -> Result<Vec<u32>, String> {
    let mut enumerator: *mut c_void = std::ptr::null_mut();
    check_hresult(
        CoCreateInstance(
            &CLSID_MM_DEVICE_ENUMERATOR,
            std::ptr::null_mut(),
            CLSCTX_ALL,
            &IID_IMM_DEVICE_ENUMERATOR,
            &mut enumerator,
        ),
        "CoCreateInstance(MMDeviceEnumerator)",
    )?;
    let enumerator = ComPtr::claim(enumerator.cast::<ImmDeviceEnumerator>());

    let mut collection: *mut ImmDeviceCollection = std::ptr::null_mut();
    check_hresult(
        ((*(*enumerator.ptr).vtable).enum_audio_endpoints)(
            enumerator.raw(),
            E_RENDER,
            DEVICE_STATE_ACTIVE,
            &mut collection,
        ),
        "IMMDeviceEnumerator::EnumAudioEndpoints",
    )?;
    let collection = ComPtr::claim(collection);

    let mut device_count = 0u32;
    check_hresult(
        ((*(*collection.ptr).vtable).get_count)(collection.raw(), &mut device_count),
        "IMMDeviceCollection::GetCount",
    )?;

    let mut pids = Vec::new();
    for index in 0..device_count {
        let mut device: *mut ImmDevice = std::ptr::null_mut();
        check_hresult(
            ((*(*collection.ptr).vtable).item)(collection.raw(), index, &mut device),
            "IMMDeviceCollection::Item",
        )?;
        let device = ComPtr::claim(device);
        collect_device_session_pids(&device, &mut pids)?;
    }
    pids.sort_unstable();
    pids.dedup();
    Ok(pids)
}

pub fn audio_session_pids() -> Result<Vec<u32>, String> {
    let hr = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) };
    let owns_initialization = if hr >= 0 {
        true
    } else if hr == RPC_E_CHANGED_MODE {
        false
    } else {
        return Err(hresult_error("CoInitializeEx", hr));
    };
    let result = unsafe { collect_audio_session_pids() };
    if owns_initialization {
        unsafe { CoUninitialize(); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_session_pids_enumerates_without_panicking() {
        let result = audio_session_pids();
        match &result {
            Ok(pids) => println!("audio session pids: {pids:?}"),
            Err(error) => println!("audio session discovery unavailable: {error}"),
        }
        if let Ok(pids) = result {
            assert!(pids.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }

    #[test]
    fn classify_windows_separates_tray_hidden_from_minimized_and_visible() {
        let window = |on_screen: bool, iconic: bool, area: i64| WindowCandidate {
            pid: 10,
            hwnd: 1,
            title: "Spotify".to_string(),
            on_screen,
            iconic,
            area,
        };
        assert_eq!(
            classify_windows(&[window(false, false, 1_920_000)], false).0,
            "hidden"
        );
        assert_eq!(
            classify_windows(&[window(true, true, 1_920_000)], false).0,
            "minimized"
        );
        assert_eq!(
            classify_windows(&[window(true, false, 1_920_000)], false).0,
            "background-visible"
        );
        assert_eq!(
            classify_windows(&[window(true, false, 1_920_000)], true).0,
            "foreground"
        );
        assert_eq!(classify_windows(&[], false).0, "window-not-found");
        // A leftover window of the same process holding focus must not pretend the
        // app is still in the foreground when nothing of it is on screen.
        assert_eq!(
            classify_windows(&[window(false, false, 1_920_000)], true).0,
            "hidden"
        );
        // Any window that is really on screen wins over a hidden main window.
        assert_eq!(
            classify_windows(
                &[window(false, false, 1_920_000), window(true, false, 100)],
                false
            )
            .0,
            "background-visible"
        );
        assert_eq!(
            classify_windows(&[window(false, false, 100), window(false, false, 9_000)], false)
                .1
                .map(|found| found.area),
            Some(9_000)
        );
    }

    #[test]
    fn window_candidate_scan_accepts_the_current_process_without_panicking() {
        assert!(window_candidates(&[]).is_empty());
        let own = std::process::id();
        let found = window_candidates(&[own]);
        assert!(found.iter().all(|window| window.pid == own));
    }
}
