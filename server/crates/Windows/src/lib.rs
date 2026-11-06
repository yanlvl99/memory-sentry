//! Win32 process, memory, window-state and installation primitives.
//! All operations validate the process creation timestamp immediately before mutation.

// The entire crate is Windows-only (raw kernel32/psapi/user32 FFI plus std::os::windows). Off
// Windows it compiles to an empty crate, mirroring the previous `#[cfg(windows)] mod platform`.
#![cfg(windows)]

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
// QUERY_LIMITED_INFORMATION is enough to open a foreign process for metadata + memory reads and,
// unlike QUERY_INFORMATION, succeeds even on many protected processes without elevation.
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
// ReadProcessMemory additionally requires VM_READ on the process handle.
const PROCESS_VM_READ: u32 = 0x0010;
// NtQueryInformationProcess information class 0 == ProcessBasicInformation.
const PROCESS_BASIC_INFORMATION_CLASS: u32 = 0;
// x64 PEB offsets for reading a remote command line. There is no public API for a foreign process's
// full command line, so we walk PEB -> ProcessParameters -> CommandLine. These are the well-known
// x64 layout offsets; the size asserts below pin the local struct layouts so Process32/PEB reads
// cannot silently mis-enumerate on a mismatched ABI.
const PEB_PROCESS_PARAMETERS_OFFSET: usize = 0x20;
const RTL_USER_PROCESS_PARAMETERS_COMMAND_LINE_OFFSET: usize = 0x70;

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

// MEMORYSTATUSEX for GlobalMemoryStatusEx; the adaptive engine reads this as its memory-pressure
// signal (Android lmkd uses Pressure Stall Information; Windows has no PSI, so dwMemoryLoad plus the
// available/total physical ratio is the closest built-in proxy).
#[repr(C)]
struct MemoryStatusEx {
    dw_length: u32,
    dw_memory_load: u32,
    ull_total_phys: u64,
    ull_avail_phys: u64,
    ull_total_page_file: u64,
    ull_avail_page_file: u64,
    ull_total_virtual: u64,
    ull_avail_virtual: u64,
    ull_avail_extended_virtual: u64,
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

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SystemInfo {
    processor_architecture: u16,
    reserved0: u16,
    page_size: u32,
    minimum_application_address: usize,
    maximum_application_address: usize,
    active_processor_mask: usize,
    number_of_processors: u32,
    processor_type: u32,
    allocation_granularity: u32,
    processor_level: u16,
    processor_revision: u16,
}

// GetNativeSystemInfo fills wProcessorArchitecture with the PROCESSOR_ARCHITECTURE_* values
// (not the IMAGE_FILE_MACHINE_* PE-header constants): AMD64 is 9 and ARM64 is 12.
const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;
const PROCESSOR_ARCHITECTURE_ARM64: u16 = 12;

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
    fn SetLastError(error: u32);
    fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    fn CreateMemoryResourceNotification(notification_type: i32) -> Handle;
    fn QueryMemoryResourceNotification(handle: Handle, state: *mut i32) -> i32;
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
    fn GetNativeSystemInfo(info: *mut SystemInfo);
    fn ReadProcessMemory(
        process: Handle,
        base_address: *const c_void,
        buffer: *mut c_void,
        size: usize,
        bytes_read: *mut usize,
    ) -> i32;
}

#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(process: Handle, counters: *mut c_void, size: u32) -> i32;
    fn EmptyWorkingSet(process: Handle) -> i32;
}

#[link(name = "user32")]
extern "system" {
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
    // Driven only by the feature-gated debug console (see control_window). PostMessage/SysCommand
    // work cross-process, so a window can be minimized or restored without touching its thread.
    fn PostMessageW(hwnd: Handle, message: u32, w_param: usize, l_param: isize) -> i32;
    fn SetWindowPos(
        hwnd: Handle,
        insert_after: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn SetForegroundWindow(hwnd: Handle) -> i32;
    fn BringWindowToTop(hwnd: Handle) -> i32;
    fn GetShellWindow() -> Handle;
    fn keybd_event(virtual_key: u8, scan_code: u8, flags: u32, extra_info: usize);
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
    fn NtQueryInformationProcess(
        process: Handle,
        process_information_class: u32,
        process_information: *mut c_void,
        process_information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}

/// Reads system physical memory for the adaptive pressure signal: returns
/// `(total_mb, available_mb, memory_load_percent)`. On failure returns zeros so callers treat the
/// pressure as unknown rather than trusting stale values.
pub fn system_memory() -> (u64, u64, u32) {
    let mut status = MemoryStatusEx {
        dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
        dw_memory_load: 0,
        ull_total_phys: 0,
        ull_avail_phys: 0,
        ull_total_page_file: 0,
        ull_avail_page_file: 0,
        ull_total_virtual: 0,
        ull_avail_virtual: 0,
        ull_avail_extended_virtual: 0,
    };
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        return (0, 0, 0);
    }
    const MIB: u64 = 1024 * 1024;
    (
        status.ull_total_phys / MIB,
        status.ull_avail_phys / MIB,
        status.dw_memory_load,
    )
}

/// Coarse system memory pressure. The primary signal is the OS memory resource notification, which is
/// the same Low/High condition Windows uses to ask applications to shrink or grow; the GlobalMemoryStatusEx
/// ratio is only a fallback when the notification object cannot be created. This is the trigger the
/// policy uses instead of a timer, mirroring the documented Windows behaviour of keeping memory use
/// constant while neither the Low nor the High condition is signalled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPressure {
    Low,
    Normal,
    High,
}

pub fn memory_pressure() -> MemoryPressure {
    use std::sync::OnceLock;
    static LOW: OnceLock<usize> = OnceLock::new();
    static HIGH: OnceLock<usize> = OnceLock::new();
    // The handles live for the process; the OS closes them at termination. Re-creating them per poll
    // would leak handles at the monitor cadence.
    let low = *LOW.get_or_init(|| unsafe { CreateMemoryResourceNotification(0) as usize });
    let high = *HIGH.get_or_init(|| unsafe { CreateMemoryResourceNotification(1) as usize });
    let mut state: i32 = 0;
    unsafe {
        if low != 0
            && QueryMemoryResourceNotification(low as Handle, &mut state) != 0
            && state != 0
        {
            return MemoryPressure::Low;
        }
        state = 0;
        if high != 0
            && QueryMemoryResourceNotification(high as Handle, &mut state) != 0
            && state != 0
        {
            return MemoryPressure::High;
        }
    }
    let (total, available, load) = system_memory();
    let available_pct = if total > 0 {
        available.saturating_mul(100).checked_div(total).unwrap_or(100)
    } else {
        100
    };
    if load >= 90 || available_pct <= 10 {
        MemoryPressure::Low
    } else if available_pct >= 40 {
        MemoryPressure::High
    } else {
        MemoryPressure::Normal
    }
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

// PROCESS_BASIC_INFORMATION (NTSTATUS exit status is not exposed here; the fields we rely on are the
// PEB pointer and the pid lineage). x64 layout is six pointer-sized slots = 48 bytes.
#[repr(C)]
struct ProcessBasicInformation {
    reserved1: *mut c_void,
    peb_base_address: *mut c_void,
    reserved2_0: *mut c_void,
    reserved2_1: *mut c_void,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
}

// UNICODE_STRING as laid out in a remote PEB: two u16 lengths then, after alignment padding, the
// pointer to a UTF-16 buffer. repr(C) places `buffer` at offset 8 on x64.
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut c_void,
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<ProcessBasicInformation>() == 48);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<UnicodeString>() == 16);

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
    // Faults per second for the exact window this row's delta was taken over, stamped by whichever
    // sampler advanced the baseline. Keeping the rate next to the delta stops a later consumer from
    // dividing it by a different (wider) window.
    pub page_faults_per_sec: Option<f64>,
    pub error_code: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub processes: Vec<ProcessRow>,
    // Wall-clock ms when the producer captured these counters. CPU percentages must divide by the age
    // of the instant the values were OBSERVED, not the moment the main loop drained the channel, so the
    // capture thread stamps this once per snapshot before sending it.
    pub captured_at_ms: u64,
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
    pub private_mb: Option<f64>,
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

fn process_stats_with_handle(handle: Handle) -> Result<(f64, Option<f64>, Option<f64>, u64), String> {
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
    // PrivateUsage (PROCESS_MEMORY_COUNTERS_EX) is the private commit charge, the same notion the
    // WMI PrivatePageCount fed the PowerShell path, so both discovery paths report a consistent value.
    let private_mb = if extended_ok && extended.private_usage > 0 {
        Some(extended.private_usage as f64 / (1024.0 * 1024.0))
    } else {
        None
    };
    Ok((
        working_set_size as f64 / (1024.0 * 1024.0),
        private_working_set_mb,
        private_mb,
        counters.page_fault_count as u64,
    ))
}

pub fn inspect_process(pid: u32) -> Result<ProcessStats, String> {
    let handle = open_process(pid, PROCESS_QUERY_INFORMATION)?;
    let result = (|| {
        let (creation, cpu_time_ms) = process_times(handle)?;
        let (working_set_mb, private_working_set_mb, private_mb, page_fault_count) = process_stats_with_handle(handle)?;
        Ok(ProcessStats { creation, cpu_time_ms, working_set_mb, private_working_set_mb, private_mb, page_fault_count })
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

/// The foreground window as a raw handle. The debug console records this before it steals focus so a
/// scenario can hand focus back to exactly whatever the user had up.
pub fn foreground_window_hwnd() -> Option<usize> {
    let foreground = unsafe { GetForegroundWindow() };
    (!foreground.is_null()).then_some(foreground as usize)
}

// Window-control verbs for the feature-gated debug console. They let a test drive a real window
// (minimize/restore/maximize/z-order/focus/blur) so the shipped focus/restore path can be exercised
// without a human. These are the only Win32 *mutations* the server exposes; the monitor never calls
// them, so in a release binary they are unreachable dead code (no route reaches control_window
// without --features debug-api), which keeps the whole debug surface off end users.
const WM_SYSCOMMAND: u32 = 0x0112;
const SC_MINIMIZE: usize = 0xF020;
const SC_MAXIMIZE: usize = 0xF030;
const SC_RESTORE: usize = 0xF120;
const HWND_TOP: usize = 0;
const HWND_BOTTOM: usize = 1;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOACTIVATE: u32 = 0x0010;
const VK_MENU: u8 = 0x12;
const KEYEVENTF_KEYUP: u32 = 0x0002;

/// Move one real window for the debug console. Returns a short label describing the path that worked
/// (e.g. which focus fallback was needed) so a scenario can quote how the transition happened. The
/// unsafe FFI lives here, in one place, so DebugApi stays pure delegation.
pub fn control_window(hwnd: usize, action: &str) -> Result<String, String> {
    // Blur needs no target handle - it hands focus to the shell's desktop window, which is how a
    // test takes foreground away from the app to reach a real background-visible transition.
    if action == "blur" {
        let shell = unsafe { GetShellWindow() };
        if shell.is_null() {
            return Err("No shell window to move focus to.".to_string());
        }
        return if unsafe { SetForegroundWindow(shell) } != 0 {
            Ok("blurred-to-shell".to_string())
        } else {
            Err(format!("SetForegroundWindow(shell) failed ({})", last_error()))
        };
    }
    if hwnd == 0 {
        return Err("A window handle is required.".to_string());
    }
    let window = hwnd as Handle;
    if unsafe { IsWindow(window) } == 0 {
        return Err(format!("Handle {hwnd:#x} is not a live window."));
    }
    match action {
        "minimize" => post_sys_command(window, SC_MINIMIZE, "minimize"),
        "restore" => post_sys_command(window, SC_RESTORE, "restore"),
        "maximize" => post_sys_command(window, SC_MAXIMIZE, "maximize"),
        "bottom" => set_zorder(window, HWND_BOTTOM as Handle, "bottom"),
        "top" => set_zorder(window, HWND_TOP as Handle, "top"),
        "focus" => focus_window(window),
        other => Err(format!("Unknown window action \"{other}\".")),
    }
}

fn post_sys_command(window: Handle, command: usize, label: &str) -> Result<String, String> {
    if unsafe { PostMessageW(window, WM_SYSCOMMAND, command, 0) } != 0 {
        Ok(format!("posted-{label}"))
    } else {
        Err(format!("PostMessageW({label}) failed ({})", last_error()))
    }
}

fn set_zorder(window: Handle, insert_after: Handle, label: &str) -> Result<String, String> {
    let ok = unsafe {
        SetWindowPos(
            window,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
    if ok != 0 {
        Ok(format!("zorder-{label}"))
    } else {
        Err(format!("SetWindowPos({label}) failed ({})", last_error()))
    }
}

/// Mirror the fallback the harness used before this moved server-side: direct call, then clear the
/// foreground lock with an Alt tap, then BringWindowToTop. Whichever works is reported as the path.
fn focus_window(window: Handle) -> Result<String, String> {
    if unsafe { SetForegroundWindow(window) } != 0 {
        return Ok("direct".to_string());
    }
    unsafe {
        keybd_event(VK_MENU, 0, 0, 0);
        std::thread::sleep(std::time::Duration::from_millis(10));
        keybd_event(VK_MENU, 0, KEYEVENTF_KEYUP, 0);
    }
    if unsafe { SetForegroundWindow(window) } != 0 {
        return Ok("alt-tap".to_string());
    }
    if unsafe { BringWindowToTop(window) } != 0 {
        return Ok("bring-to-top".to_string());
    }
    Err(format!("focus failed ({})", last_error()))
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

/// Returns the machine's native CPU architecture as "x64" or "arm64" (or a labeled unsupported
/// value). GetNativeSystemInfo reports the real hardware architecture even for an emulated process,
/// so this is the correct basis for rejecting a mismatched installer.
pub fn native_arch() -> Result<String, String> {
    let mut info = SystemInfo::default();
    unsafe { GetNativeSystemInfo(&mut info) };
    Ok(match info.processor_architecture {
        PROCESSOR_ARCHITECTURE_AMD64 => "x64".to_string(),
        PROCESSOR_ARCHITECTURE_ARM64 => "arm64".to_string(),
        other => format!("unsupported (0x{other:04X})"),
    })
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

// Read `len` raw bytes from a remote process. Bounded so a corrupt length can never drive a huge
// allocation, and any partial read is treated as failure rather than trusting a short buffer.
fn read_memory_bytes(handle: Handle, address: usize, len: usize) -> Option<Vec<u8>> {
    const MAX_READ: usize = 32_768;
    if address == 0 || len == 0 || len > MAX_READ {
        return None;
    }
    let mut buffer = vec![0u8; len];
    let mut bytes_read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            handle,
            address as *const c_void,
            buffer.as_mut_ptr() as *mut c_void,
            len,
            &mut bytes_read,
        )
    };
    if ok == 0 || bytes_read != len {
        return None;
    }
    Some(buffer)
}

fn read_usize(handle: Handle, address: usize) -> Option<usize> {
    let bytes = read_memory_bytes(handle, address, std::mem::size_of::<usize>())?;
    let mut arr = [0u8; 8];
    arr[..bytes.len()].copy_from_slice(&bytes);
    Some(usize::from_le_bytes(arr))
}

/// Decode a remote UNICODE_STRING buffer (UTF-16LE bytes) into a String. Lossy so a stray surrogate
/// never aborts discovery; the even-length assumption matches Windows' UTF-16 command lines.
fn parse_command_line_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

// The full command line of a remote process, obtained by walking PEB -> ProcessParameters ->
// CommandLine. There is no public Win32 API for this, so any failure (access denied, protected
// process, a vanished handle) returns None and the caller degrades to an empty command line - the
// same as the PowerShell path when WMI returned no CommandLine - rather than dropping the process.
fn native_command_line(pid: u32) -> Option<String> {
    let handle = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ).ok()?;
    let result = (|| {
        let mut basic = ProcessBasicInformation {
            reserved1: std::ptr::null_mut(),
            peb_base_address: std::ptr::null_mut(),
            reserved2_0: std::ptr::null_mut(),
            reserved2_1: std::ptr::null_mut(),
            unique_process_id: 0,
            inherited_from_unique_process_id: 0,
        };
        let mut returned = 0u32;
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                PROCESS_BASIC_INFORMATION_CLASS,
                &mut basic as *mut _ as *mut c_void,
                std::mem::size_of::<ProcessBasicInformation>() as u32,
                &mut returned,
            )
        };
        if status != 0 || basic.peb_base_address.is_null() {
            return None;
        }
        let peb = basic.peb_base_address as usize;
        let parameters = read_usize(handle, peb + PEB_PROCESS_PARAMETERS_OFFSET)?;
        let string_addr = parameters + RTL_USER_PROCESS_PARAMETERS_COMMAND_LINE_OFFSET;
        let raw = read_memory_bytes(handle, string_addr, std::mem::size_of::<UnicodeString>())?;
        let length = u16::from_le_bytes([raw[0], raw[1]]) as usize;
        let mut ptr_arr = [0u8; 8];
        ptr_arr.copy_from_slice(&raw[8..16]);
        let buffer = usize::from_le_bytes(ptr_arr);
        if length == 0 {
            return Some(String::new());
        }
        let chars = read_memory_bytes(handle, buffer, length)?;
        Some(parse_command_line_utf16(&chars))
    })();
    unsafe { CloseHandle(handle); }
    result
}

// PROCESSENTRY32W.sz_exe_file is the executable base name only (e.g. "Spotify.exe"), which is exactly
// the Name='Spotify.exe' filter the WMI query applied, so comparing it here keeps the same process set.
fn entry_exe_matches(name: &[u16; 260], expected_lower: &str) -> bool {
    let end = name.iter().position(|&unit| unit == 0).unwrap_or(name.len());
    let text: String = String::from_utf16_lossy(&name[..end]).to_ascii_lowercase();
    text == expected_lower
}

/// Pure Win32 process discovery, app-agnostic: enumerate every process whose executable base name is
/// in `targets` (lowercased) with a Toolhelp32 snapshot, read each command line from the remote PEB,
/// then reuse inspect_process for creation/CPU/working-set/faults. This replaces the per-tick
/// PowerShell + Get-CimInstance subprocess (tens of MB transient + a WMI query every snapshot) with
/// in-process calls. The Snapshot/ProcessRow shape is identical so the monitor, API and dashboard are
/// unchanged. Callers keep a PowerShell fallback for the rare pass where native enumeration or the PEB
/// reads fail broadly, so discovery is never worse than before. Which executable to target is supplied
/// by the caller (the application profile) and never hardcoded here.
pub fn native_process_snapshot(targets: &[&str]) -> Result<Snapshot, String> {
    let mut snapshot = Snapshot::default();
    let toolhelp = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if toolhelp.is_null() || toolhelp as isize == -1 {
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
    if unsafe { Process32FirstW(toolhelp, &mut entry) } == 0 {
        let error = last_error();
        unsafe { CloseHandle(toolhelp); }
        if error == ERROR_NO_MORE_FILES {
            return Ok(snapshot);
        }
        return Err(format!("Could not enumerate processes (Win32 error {error})."));
    }
    loop {
        let pid = entry.th32_process_id;
        let ppid = entry.th32_parent_process_id.max(0) as u32;
        if pid != 0 && targets.iter().any(|target| entry_exe_matches(&entry.sz_exe_file, target)) {
            let cmd = native_command_line(pid).unwrap_or_default();
            let mut creation = 0u64;
            let mut cpu_time_ms = 0.0f64;
            let mut working_set_mb = None;
            let mut private_working_set_mb = None;
            let mut private_mb = None;
            let mut page_fault_count = None;
            let mut error_code = 0i32;
            match inspect_process(pid) {
                Ok(stats) => {
                    // The creation timestamp is the identity revalidated before every mutation.
                    creation = stats.creation;
                    cpu_time_ms = stats.cpu_time_ms;
                    working_set_mb = Some(stats.working_set_mb);
                    private_working_set_mb = stats.private_working_set_mb;
                    private_mb = stats.private_mb;
                    page_fault_count = Some(stats.page_fault_count);
                }
                Err(_) => error_code = -1,
            }
            snapshot.processes.push(ProcessRow {
                pid, ppid, cmd, creation, cpu_time_ms, cpu_percent: None,
                working_set_mb, private_working_set_mb, private_mb,
                page_fault_count, page_faults_delta: None, page_faults_per_sec: None, error_code,
            });
        }
        if unsafe { Process32NextW(toolhelp, &mut entry) } == 0 {
            break;
        }
    }
    unsafe { CloseHandle(toolhelp); }
    Ok(snapshot)
}

pub fn powershell_snapshot(image_name: &str) -> Result<Snapshot, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // The image name is a fixed value from the application profile, never free-form user input, so
    // substituting it into the WMI filter is safe. A placeholder token (rather than format!) keeps the
    // many PowerShell `-f` braces (`{0}`, `{1}`, ...) intact.
    let script = r#"
$ErrorActionPreference = 'Stop'
try {
  [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
  $utf8 = New-Object System.Text.UTF8Encoding($false)
  $culture = [Globalization.CultureInfo]::InvariantCulture
  foreach ($p in @(Get-CimInstance Win32_Process -Filter "Name='__MSS_IMAGE__'")) {
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
"#
    .replace("__MSS_IMAGE__", image_name);
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script.as_str()])
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
                    page_fault_count, page_faults_delta: None, page_faults_per_sec: None, error_code,
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
    fn memory_pressure_reports_a_level_without_panicking() {
        let pressure = memory_pressure();
        println!("memory pressure: {pressure:?}");
        let (total, available, _) = system_memory();
        if total > 0 {
            assert!(available <= total);
        }
    }

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
    fn parse_command_line_utf16_decodes_a_known_command_line() {
        // The remote PEB stores the command line as a UTF-16LE UNICODE_STRING; a wrong byte order or
        // an odd-length truncation would corrupt the identity sub-labels, so pin the decode exactly.
        let cmdline = "\"C:\\Users\\me\\AppData\\Spotify.exe\" --type=renderer --renderer-client-id=7";
        let bytes: Vec<u8> = cmdline.encode_utf16().flat_map(|unit| unit.to_le_bytes()).collect();
        assert_eq!(parse_command_line_utf16(&bytes), cmdline);
    }

    #[test]
    fn entry_exe_matches_filters_by_case_insensitive_base_name() {
        let mut name = [0u16; 260];
        for (index, unit) in "Spotify.exe".encode_utf16().enumerate() {
            name[index] = unit;
        }
        assert!(entry_exe_matches(&name, "spotify.exe"));
        let mut other = [0u16; 260];
        for (index, unit) in "SpotifyWebHelper.exe".encode_utf16().enumerate() {
            other[index] = unit;
        }
        assert!(!entry_exe_matches(&other, "spotify.exe"));
    }

    #[test]
    fn native_process_snapshot_enumerates_without_panicking() {
        // Must never panic on a real machine even with no Spotify running or with processes we cannot
        // open; any process present carries a non-zero pid, and command lines may be empty when the
        // PEB read is not permitted (the caller's fallback then covers a broad failure).
        let snapshot = native_process_snapshot(&["spotify.exe"]).expect("toolhelp enumeration should succeed on Windows");
        for process in &snapshot.processes {
            assert_ne!(process.pid, 0);
        }
    }

    #[test]
    fn native_command_line_reads_our_own_live_process() {
        // The pure parser proves the decode, but only a read against a real, accessible process proves
        // the whole FFI chain works on this machine: OpenProcess(QUERY_LIMITED|VM_READ) ->
        // NtQueryInformationProcess -> the x64 PEB offsets -> ReadProcessMemory. Our own test binary
        // always has a command line containing its executable name, so this asserts a non-empty read.
        let pid = current_process_id();
        let cmd = native_command_line(pid).expect("a process can always PEB-read its own command line");
        let exe_stem = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        assert!(!cmd.is_empty(), "self command line should not be empty for pid {pid}");
        if !exe_stem.is_empty() {
            assert!(
                cmd.to_ascii_lowercase().contains(&exe_stem.to_ascii_lowercase()),
                "self command line should name its own executable; got: {cmd}"
            );
        }
    }

    // Proves native discovery is behavior-equivalent to the retired PowerShell/WMI path on REAL
    // processes: same Spotify.exe set, same parent lineage, and command lines carrying the same
    // role-determining tokens that process_role()/process_identity() parse. #[ignore]d so it neither
    // runs in CI nor spawns PowerShell on every suite run; invoke it on a machine with Spotify open
    // (cargo test -p memory-sentry-windows -- --ignored) to validate the STEP A swap end-to-end.
    #[test]
    #[ignore = "compares against powershell_snapshot live; run manually with Spotify running"]
    fn native_discovery_matches_powershell_on_live_processes() {
        let native = native_process_snapshot(&["spotify.exe"]).expect("native enumeration");
        let shell = powershell_snapshot("Spotify.exe").expect("powershell enumeration");
        let role_markers = ["--type=renderer", "--type=gpu-process", "--type=utility", "crashpad", "audio.mojom.audioservice", "--utility-sub-type=audio"];
        let role_of = |cmd: &str| -> Vec<&'static str> {
            let lower = cmd.to_ascii_lowercase();
            role_markers.iter().copied().filter(|m| lower.contains(m)).collect()
        };
        let mut native_pids: Vec<u32> = native.processes.iter().map(|p| p.pid).collect();
        let mut shell_pids: Vec<u32> = shell.processes.iter().map(|p| p.pid).collect();
        native_pids.sort_unstable();
        shell_pids.sort_unstable();
        println!("native found {} procs, powershell found {} procs", native_pids.len(), shell_pids.len());
        assert_eq!(native_pids, shell_pids, "native and WMI enumerate a different Spotify.exe process set");
        for n in &native.processes {
            let Some(s) = shell.processes.iter().find(|p| p.pid == n.pid) else { continue };
            assert_eq!(n.ppid, s.ppid, "parent pid differs for pid {}", n.pid);
            // Only the live, readable command lines can be diffed; a protected PID yields empty cmd.
            if !n.cmd.is_empty() && !s.cmd.is_empty() {
                assert_eq!(role_of(&n.cmd), role_of(&s.cmd), "role tokens differ for pid {}\n native: {}\n shell:  {}", n.pid, n.cmd, s.cmd);
            }
        }
    }

    // Quantifies the STEP A win directly: the old path spawns powershell.exe + a WMI query every tick,
    // the new one is an in-process Toolhelp walk + PEB reads. #[ignore]d so the suite never pays the
    // multi-second PowerShell cost; run manually to show the recurring per-tick cost collapse.
    #[test]
    #[ignore = "times powershell_snapshot (multi-second); run manually for the perf figure"]
    fn native_discovery_is_far_cheaper_than_powershell() {
        use std::time::Instant;
        // Warm-up once so the first-call JIT/paths do not skew the median.
        let _ = native_process_snapshot(&["spotify.exe"]);
        let mut native_samples = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let _ = native_process_snapshot(&["spotify.exe"]).expect("native enumeration");
            native_samples.push(start.elapsed().as_millis());
        }
        let start = Instant::now();
        let _ = powershell_snapshot("Spotify.exe").expect("powershell enumeration");
        let shell_ms = start.elapsed().as_millis();
        native_samples.sort_unstable();
        let native_median = native_samples[native_samples.len() / 2];
        println!("native median {native_median} ms over 5 calls vs one powershell call {shell_ms} ms");
        assert!(
            native_median < shell_ms,
            "native discovery (median {native_median} ms) should be cheaper than a PowerShell+WMI tick ({shell_ms} ms)"
        );
    }

    #[test]
    fn native_arch_resolves_to_a_supported_64_bit_family() {
        // Guards the GetNativeSystemInfo mapping: comparing wProcessorArchitecture against the
        // wrong constant family (IMAGE_FILE_MACHINE_* instead of PROCESSOR_ARCHITECTURE_*) would
        // make every real machine look "unsupported" and wrongly block valid installs.
        let arch = native_arch().expect("native architecture is readable");
        assert!(
            arch == "x64" || arch == "arm64",
            "unexpected native architecture: {arch}"
        );
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
