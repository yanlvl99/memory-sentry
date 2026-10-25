'use strict';

// Spicetify Memory Sentry 0.2.0 — adaptive Windows companion.
// Uses a per-process working-set policy while Spotify is minimized or hidden.
// The audio service is always excluded; original native limits are backed up/restored.

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const http = require('node:http');
const crypto = require('node:crypto');
const { execFileSync, execFile } = require('node:child_process');
const { performance } = require('node:perf_hooks');
const AdaptivePolicy = require('./adaptive-policy');

const VERSION = '0.2.0';
if (process.platform !== 'win32') {
  console.error('[Memory Sentry] This companion only runs on Windows.');
  process.exit(2);
}
const koffi = require('koffi');

const ROOT = __dirname;
const SETTINGS_PATH = path.join(ROOT, 'settings.json');
const LOG_DIR = path.join(ROOT, 'logs');
const LOG_PATH = path.join(LOG_DIR, 'helper.log');
const BACKUP_PATH = path.join(ROOT, 'working-set-backup.json');
const BACKUP_TMP_PATH = `${BACKUP_PATH}.tmp`;
fs.mkdirSync(LOG_DIR, { recursive: true });

function clamp(value, min, max, fallback) {
  const n = Number(value);
  return Number.isFinite(n) ? Math.max(min, Math.min(max, Math.round(n))) : fallback;
}

function loadSettings() {
  const parsed = JSON.parse(fs.readFileSync(SETTINGS_PATH, 'utf8').replace(/^\uFEFF/, ''));
  if (!parsed || typeof parsed.token !== 'string' || parsed.token.length < 32) {
    throw new Error('settings.json is missing a valid per-install token.');
  }
  return {
    port: Number(parsed.port) || 38472,
    token: parsed.token,
    autoOptimize: parsed.autoOptimize !== false,
    hiddenDelaySeconds: clamp(parsed.hiddenDelaySeconds, 1, 30, 2)
  };
}

let settings = loadSettings();

const PROCESS_QUERY_INFORMATION = 0x0400;
const PROCESS_SET_QUOTA = 0x0100;
const QUOTA_LIMITS_HARDWS_MIN_DISABLE = 0x00000002;
const QUOTA_LIMITS_HARDWS_MAX_ENABLE = 0x00000004;
const HARD_CAP_FLAGS = QUOTA_LIMITS_HARDWS_MIN_DISABLE | QUOTA_LIMITS_HARDWS_MAX_ENABLE;
// 80 KiB is at least 20 x 4-KiB pages; Windows rounds a smaller minimum up.
const MIN_WORKING_SET_BYTES = 80 * 1024;

const user32 = koffi.load('user32.dll');
const kernel32 = koffi.load('kernel32.dll');
const psapi = koffi.load('psapi.dll');

const IsWindowVisible = user32.func('int32_t __stdcall IsWindowVisible(void *hWnd)');
const IsIconic = user32.func('int32_t __stdcall IsIconic(void *hWnd)');
const GetForegroundWindow = user32.func('void * __stdcall GetForegroundWindow(void)');
const OpenProcess = kernel32.func('void * __stdcall OpenProcess(uint32_t dwDesiredAccess, int32_t bInheritHandle, uint32_t dwProcessId)');
const CloseHandle = kernel32.func('int32_t __stdcall CloseHandle(void *hObject)');
const GetLastError = kernel32.func('uint32_t __stdcall GetLastError(void)');
const MSS_FILETIME = koffi.struct('MSS_FILETIME', {
  dwLowDateTime: 'uint32_t',
  dwHighDateTime: 'uint32_t'
});
const GetProcessTimes = kernel32.func(
  'int32_t __stdcall GetProcessTimes(void *hProcess, _Out_ MSS_FILETIME *lpCreationTime, _Out_ MSS_FILETIME *lpExitTime, _Out_ MSS_FILETIME *lpKernelTime, _Out_ MSS_FILETIME *lpUserTime)'
);
const GetProcessWorkingSetSizeEx = kernel32.func(
  'int32_t __stdcall GetProcessWorkingSetSizeEx(void *hProcess, _Out_ size_t *lpMinimumWorkingSetSize, _Out_ size_t *lpMaximumWorkingSetSize, _Out_ uint32_t *Flags)'
);
const SetProcessWorkingSetSizeEx = kernel32.func(
  'int32_t __stdcall SetProcessWorkingSetSizeEx(void *hProcess, size_t dwMinimumWorkingSetSize, size_t dwMaximumWorkingSetSize, uint32_t Flags)'
);
const EmptyWorkingSet = psapi.func('int32_t __stdcall EmptyWorkingSet(void *hProcess)');

const PROCESS_MEMORY_COUNTERS = koffi.struct('MSS_PROCESS_MEMORY_COUNTERS', {
  cb: 'uint32_t',
  PageFaultCount: 'uint32_t',
  PeakWorkingSetSize: 'size_t',
  WorkingSetSize: 'size_t',
  QuotaPeakPagedPoolUsage: 'size_t',
  QuotaPagedPoolUsage: 'size_t',
  QuotaPeakNonPagedPoolUsage: 'size_t',
  QuotaNonPagedPoolUsage: 'size_t',
  PagefileUsage: 'size_t',
  PeakPagefileUsage: 'size_t'
});
const GetProcessMemoryInfo = psapi.func(
  'int32_t __stdcall GetProcessMemoryInfo(void *Process, _Inout_ MSS_PROCESS_MEMORY_COUNTERS *ppsmemCounters, uint32_t cb)'
);

const state = {
  startedAt: new Date().toISOString(),
  lastSnapshotAt: 0,
  snapshotAtMs: 0,
  snapshotOk: false,
  snapshot: { processes: [], windows: [] },
  rootPid: null,
  hwnd: 0n,
  windowTitle: '',
  windowState: 'not-detected',
  hiddenSince: 0,
  lastRestoreAttemptAt: 0,
  lastAction: null,
  memory: { spotifyTotalMB: 0, targetTotalMB: 0, processCount: 0, targetCount: 0, cappedCount: 0, targets: [] },
  lastError: '',
  recentEvents: [],
  telemetry: {
    lastRestoreDurationMs: null, lastRestoreAt: null, lastRestoreRestored: 0, lastRestoreFailures: 0,
    pageFaultDelta: null, pageFaultsPerSecond: null, sampleIntervalMs: null,
    adaptiveAdjustments: 0, lastAdaptiveAdjustmentAt: null
  },
  busy: false
};

// pid -> the original Win32 working-set settings plus a creation-time identity.
// Values are persisted before changing the process so a future helper startup can recover them.
const cappedProcesses = new Map();
let snapshotInFlight = false;
let lastSnapshotAttemptAt = 0;
const pageFaultBaselines = new Map();
const adaptiveProfiles = new Map();
const targetRetryAt = new Map();
const TARGET_RETRY_DELAY_MS = 2500;

function log(message, level = 'info') {
  const line = `[${new Date().toISOString()}] [${level.toUpperCase()}] ${message}`;
  try {
    if (fs.existsSync(LOG_PATH) && fs.statSync(LOG_PATH).size > 512 * 1024) {
      const previous = path.join(LOG_DIR, `helper-${Date.now()}.previous.log`);
      fs.renameSync(LOG_PATH, previous);
    }
    fs.appendFileSync(LOG_PATH, `${line}${os.EOL}`, 'utf8');
  } catch (_) { /* logging must never stop the monitor */ }
  state.recentEvents.unshift({ time: new Date().toISOString(), level, message });
  state.recentEvents = state.recentEvents.slice(0, 14);
  if (level === 'error') state.lastError = message;
  console.log(line);
}

const PROCESS_SNAPSHOT_SCRIPT = `
$ErrorActionPreference = 'Stop'
$procs = @(Get-CimInstance Win32_Process -Filter "Name='Spotify.exe'" | ForEach-Object {
  $created = ''
  try { if ($_.CreationDate) { $created = [string](([datetime]$_.CreationDate).ToFileTimeUtc()) } } catch { }
  $cpuMs = 0.0
  try { $cpuMs = ([double]$_.KernelModeTime + [double]$_.UserModeTime) / 10000.0 } catch { }
  [pscustomobject]@{
    pid=[int]$_.ProcessId; ppid=[int]$_.ParentProcessId; name=[string]$_.Name
    cmd=[string]$_.CommandLine; path=[string]$_.ExecutablePath; creation=$created
    cpuTimeMs=$cpuMs; workingSetBytes=[double]$_.WorkingSetSize
    privateBytes=[double]$_.PrivatePageCount; pageFaults=[double]$_.PageFaults
  }
})
$wins = @(Get-Process -Name Spotify -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ pid=[int]$_.Id; hwnd=[string]$_.MainWindowHandle.ToInt64(); title=[string]$_.MainWindowTitle }
})
[pscustomobject]@{ processes=$procs; windows=$wins } | ConvertTo-Json -Depth 4 -Compress
`;

const POWERSHELL_ARGS = [
  '-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', PROCESS_SNAPSHOT_SCRIPT
];
const POWERSHELL_OPTIONS = { encoding: 'utf8', windowsHide: true, timeout: 8000, maxBuffer: 1024 * 1024 };

function parseSnapshotOutput(stdout) {
  const json = JSON.parse(String(stdout || '{}').trim() || '{}');
  const toArray = value => value == null ? [] : (Array.isArray(value) ? value : [value]);
  return {
    processes: toArray(json.processes).map(p => ({
      pid: Number(p.pid), ppid: Number(p.ppid), name: String(p.name || 'Spotify.exe'),
      cmd: String(p.cmd || ''), path: String(p.path || ''), creation: String(p.creation || ''),
      cpuTimeMs: Number(p.cpuTimeMs), cpuPercent: null,
      workingSetMB: Number.isFinite(Number(p.workingSetBytes)) ? Number(p.workingSetBytes) / (1024 * 1024) : null,
      privateMB: Number.isFinite(Number(p.privateBytes)) ? Number(p.privateBytes) / (1024 * 1024) : null,
      pageFaultCount: Number.isFinite(Number(p.pageFaults)) ? Number(p.pageFaults) : null
    })).filter(p => Number.isInteger(p.pid) && p.pid > 0),
    windows: toArray(json.windows).map(w => ({
      pid: Number(w.pid), hwnd: String(w.hwnd || '0'), title: String(w.title || '')
    })).filter(w => Number.isInteger(w.pid) && w.pid > 0)
  };
}

function powershellSnapshot() {
  const stdout = execFileSync('powershell.exe', POWERSHELL_ARGS, { ...POWERSHELL_OPTIONS });
  return parseSnapshotOutput(stdout);
}

function powershellSnapshotAsync() {
  // The recurring monitor must never synchronously wait for PowerShell. On slower PCs,
  // the old sync query could block the Node event loop while the user was restoring Spotify.
  return new Promise((resolve, reject) => {
    execFile('powershell.exe', POWERSHELL_ARGS, POWERSHELL_OPTIONS, (error, stdout, stderr) => {
      if (error) {
        if (stderr && !error.message.includes(stderr.trim())) error.message += ` — ${stderr.trim().slice(0, 400)}`;
        reject(error);
        return;
      }
      try { resolve(parseSnapshotOutput(stdout)); }
      catch (parseError) { reject(parseError); }
    });
  });
}

function resolveRootAndWindow(snapshot) {
  const byPid = new Map(snapshot.processes.map(p => [p.pid, p]));
  const candidates = snapshot.windows
    .filter(w => w.hwnd && w.hwnd !== '0' && byPid.has(w.pid))
    .map(w => ({ window: w, process: byPid.get(w.pid) }))
    .sort((a, b) => {
      const aChild = /--type\s*=\s*\w+/i.test(a.process.cmd) ? 1 : 0;
      const bChild = /--type\s*=\s*\w+/i.test(b.process.cmd) ? 1 : 0;
      return aChild - bChild;
    });
  const chosen = candidates.find(x => !/--type\s*=\s*\w+/i.test(x.process.cmd)) || candidates[0];
  if (!chosen) return { rootPid: null, hwnd: 0n, title: '' };
  let hwnd = 0n;
  try { hwnd = BigInt(chosen.window.hwnd); } catch (_) { hwnd = 0n; }
  return { rootPid: chosen.window.pid, hwnd, title: chosen.window.title };
}

function acceptSnapshot(snapshot) {
  const sampledAt = Date.now();
  const previousByKey = new Map(state.snapshot.processes.map(p => [`${p.pid}:${p.creation}`, p]));
  const elapsedMs = state.snapshotAtMs ? sampledAt - state.snapshotAtMs : 0;
  snapshot.processes = snapshot.processes.map(p => {
    const previous = previousByKey.get(`${p.pid}:${p.creation}`);
    let cpuPercent = null;
    if (previous) cpuPercent = AdaptivePolicy.estimateCpuPercent(previous.cpuTimeMs, p.cpuTimeMs, elapsedMs);
    return { ...p, cpuPercent: cpuPercent == null ? null : Math.round(cpuPercent * 10) / 10 };
  });
  state.snapshot = snapshot;
  state.lastSnapshotAt = sampledAt;
  state.snapshotAtMs = sampledAt;
  state.snapshotOk = true;
  const root = resolveRootAndWindow(snapshot);
  state.rootPid = root.rootPid;
  state.hwnd = root.hwnd;
  state.windowTitle = root.title;
  state.lastError = '';
  state.windowState = getWindowState();
  return true;
}

function recordSnapshotFailure(error) {
  state.snapshotOk = false;
  const message = `Could not query Spotify processes: ${error.message}`;
  if (state.lastError !== message) log(message, 'error');
  else state.lastError = message;
}

function refreshSnapshot() {
  try { return acceptSnapshot(powershellSnapshot()); }
  catch (error) { recordSnapshotFailure(error); return false; }
}

function refreshSnapshotAsync() {
  if (snapshotInFlight) return false;
  snapshotInFlight = true;
  lastSnapshotAttemptAt = Date.now();
  powershellSnapshotAsync()
    .then(snapshot => { acceptSnapshot(snapshot); })
    .catch(error => { recordSnapshotFailure(error); })
    .finally(() => { snapshotInFlight = false; });
  return true;
}

function getWindowState() {
  if (!state.rootPid || !state.hwnd) return state.snapshot.processes.length ? 'window-not-found' : 'not-running';
  try {
    const visible = Boolean(IsWindowVisible(state.hwnd));
    const minimized = Boolean(IsIconic(state.hwnd));
    if (minimized) return 'minimized';
    if (!visible) return 'hidden';
    const foreground = GetForegroundWindow();
    if (foreground && String(BigInt(foreground)) === String(state.hwnd)) return 'foreground';
    return 'background-visible';
  } catch (error) {
    state.lastError = `Could not inspect the Spotify window: ${error.message}`;
    return 'window-check-error';
  }
}

function descendantPids(rootPid, processes) {
  const ids = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const p of processes) {
      if (!ids.has(p.pid) && ids.has(p.ppid)) {
        ids.add(p.pid);
        changed = true;
      }
    }
  }
  return ids;
}

function processRole(p) {
  if (/audio\.mojom\.AudioService|--utility-sub-type\s*=\s*[^\s]*audio|audio-service/i.test(p.cmd)) return 'Audio';
  if (p.pid === state.rootPid) return 'Main';
  if (/--type\s*=\s*renderer\b/i.test(p.cmd)) return 'Renderer';
  if (/--type\s*=\s*gpu-process\b/i.test(p.cmd)) return 'GPU';
  if (/--type\s*=\s*utility\b/i.test(p.cmd)) return 'Utility';
  if (/crashpad/i.test(p.cmd)) return 'Crashpad';
  return 'Helper';
}

function allSpotifyDescendants() {
  if (!state.rootPid) return [];
  const descendants = descendantPids(state.rootPid, state.snapshot.processes);
  return state.snapshot.processes
    .filter(p => descendants.has(p.pid))
    .map(p => ({ ...p, role: processRole(p) }));
}

function targetProcesses() {
  return allSpotifyDescendants().filter(p => {
    // Dedicated audio processes are never capped, regardless of configuration.
    if (p.role === 'Audio') return false;
    // A process without a creation identity cannot be safely backed up/restored.
    return Boolean(p.creation);
  }).map(p => ({ pid: p.pid, role: p.role, creation: p.creation }));
}

function openProcess(pid, access) {
  const handle = OpenProcess(access, 0, pid);
  if (!handle) return { handle: null, errorCode: Number(GetLastError()) };
  return { handle, errorCode: 0 };
}

function readWorkingSetWithHandle(handle) {
  const counters = { cb: koffi.sizeof(PROCESS_MEMORY_COUNTERS) };
  const ok = GetProcessMemoryInfo(handle, counters, koffi.sizeof(PROCESS_MEMORY_COUNTERS));
  if (!ok) return { mb: null, pageFaultCount: null, errorCode: Number(GetLastError()) };
  return {
    mb: Number(counters.WorkingSetSize) / (1024 * 1024),
    pageFaultCount: Number(counters.PageFaultCount),
    errorCode: 0
  };
}

function readWorkingSetMB(pid) {
  const opened = openProcess(pid, PROCESS_QUERY_INFORMATION);
  if (!opened.handle) return { mb: null, errorCode: opened.errorCode };
  try { return readWorkingSetWithHandle(opened.handle); }
  finally { try { CloseHandle(opened.handle); } catch (_) { } }
}

function readOriginalLimits(handle) {
  const minimum = [0];
  const maximum = [0];
  const flags = [0];
  const ok = GetProcessWorkingSetSizeEx(handle, minimum, maximum, flags);
  if (!ok) return { ok: false, errorCode: Number(GetLastError()) };
  const minValue = Number(minimum[0]);
  const maxValue = Number(maximum[0]);
  const flagValue = Number(flags[0]);
  if (!Number.isFinite(minValue) || !Number.isFinite(maxValue) || !Number.isFinite(flagValue)) {
    return { ok: false, errorCode: -1 };
  }
  return { ok: true, minimum: minValue, maximum: maxValue, flags: flagValue };
}

function processCreationTimeFromHandle(handle) {
  const creation = {};
  const exitTime = {};
  const kernelTime = {};
  const userTime = {};
  const ok = GetProcessTimes(handle, creation, exitTime, kernelTime, userTime);
  if (!ok) return { ok: false, errorCode: Number(GetLastError()) };
  try {
    const high = BigInt(Number(creation.dwHighDateTime) >>> 0);
    const low = BigInt(Number(creation.dwLowDateTime) >>> 0);
    return { ok: true, ticks: ((high << 32n) | low).toString() };
  } catch (_) {
    return { ok: false, errorCode: -1 };
  }
}

function sameCreationIdentity(expected, actual) {
  // Win32 FILETIME has 100 ns units; CIM timestamps may be rounded to microseconds.
  // A 10 ms tolerance accommodates that serialization without confusing PID reuse.
  try {
    let delta = BigInt(expected) - BigInt(actual);
    if (delta < 0n) delta = -delta;
    return delta <= 100000n;
  } catch (_) { return false; }
}

function verifyProcessIdentity(handle, expectedCreation) {
  const current = processCreationTimeFromHandle(handle);
  if (!current.ok) return { ok: false, errorCode: current.errorCode };
  return { ok: sameCreationIdentity(expectedCreation, current.ticks), actual: current.ticks };
}

function roundMB(value) {
  return Number.isFinite(value) ? Math.round(value * 10) / 10 : null;
}

function recordKey(record) { return `${record.pid}:${record.creation}`; }
function currentProcessForRecord(record) {
  return state.snapshot.processes.find(p => p.pid === record.pid && p.creation === record.creation) || null;
}
function samePidDifferentInstance(record) {
  return state.snapshot.processes.some(p => p.pid === record.pid && p.creation !== record.creation);
}

function readBackupFile() {
  try {
    const parsed = JSON.parse(fs.readFileSync(BACKUP_PATH, 'utf8').replace(/^\uFEFF/, ''));
    if (!parsed || !Array.isArray(parsed.processes)) return [];
    return parsed.processes.filter(r => Number.isInteger(Number(r.pid)) && Number(r.pid) > 0 &&
      typeof r.creation === 'string' && r.creation.length > 0 &&
      Number.isFinite(Number(r.minimum)) && Number.isFinite(Number(r.maximum)) && Number.isFinite(Number(r.flags))
    ).map(r => ({
      pid: Number(r.pid), creation: String(r.creation), role: String(r.role || 'Unknown'),
      minimum: Number(r.minimum), maximum: Number(r.maximum), flags: Number(r.flags),
      appliedMaximumMB: Number.isFinite(Number(r.appliedMaximumMB)) ? Number(r.appliedMaximumMB) : null,
      appliedAt: String(r.appliedAt || '')
    }));
  } catch (_) { return []; }
}

function persistBackups() {
  try {
    if (!cappedProcesses.size) {
      fs.rmSync(BACKUP_PATH, { force: true });
      fs.rmSync(BACKUP_TMP_PATH, { force: true });
      return true;
    }
    const content = JSON.stringify({ version: VERSION, processes: [...cappedProcesses.values()] }, null, 2) + os.EOL;
    fs.writeFileSync(BACKUP_TMP_PATH, content, 'utf8');
    try { fs.renameSync(BACKUP_TMP_PATH, BACKUP_PATH); }
    catch (_) {
      fs.copyFileSync(BACKUP_TMP_PATH, BACKUP_PATH);
      fs.rmSync(BACKUP_TMP_PATH, { force: true });
    }
    return true;
  } catch (error) {
    log(`Could not persist the original working-set backup: ${error.message}`, 'error');
    return false;
  }
}

function makeBackupRecord(target, limits) {
  return {
    pid: target.pid,
    creation: target.creation,
    role: target.role,
    minimum: limits.minimum,
    maximum: limits.maximum,
    flags: limits.flags,
    appliedMaximumMB: null,
    appliedAt: new Date().toISOString()
  };
}

function setOriginalLimits(record, handle) {
  return Boolean(SetProcessWorkingSetSizeEx(handle, record.minimum, record.maximum, record.flags));
}

function applyHardCap(target, reason, limitMB = AdaptivePolicy.baseCapMB(target.role)) {
  const existing = cappedProcesses.get(target.pid);
  if (existing && existing.creation === target.creation) return { ok: true, skipped: true };
  if (existing && existing.creation !== target.creation) {
    // PID reuse: only discard the stale backup after observing this new creation identity.
    cappedProcesses.delete(target.pid);
    persistBackups();
  }

  const opened = openProcess(target.pid, PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA);
  if (!opened.handle) {
    return { ok: false, error: `PID ${target.pid}: OpenProcess denied (error ${opened.errorCode})` };
  }

  let record = null;
  let capApplied = false;
  try {
    const identity = verifyProcessIdentity(opened.handle, target.creation);
    if (!identity.ok) {
      return { ok: false, error: identity.errorCode ? `PID ${target.pid}: could not verify process identity (error ${identity.errorCode}); cap was not applied.` : `PID ${target.pid}: identity changed since the last snapshot; cap skipped.` };
    }
    const original = readOriginalLimits(opened.handle);
    if (!original.ok) return { ok: false, error: `PID ${target.pid}: GetProcessWorkingSetSizeEx failed (error ${original.errorCode})` };
    const before = readWorkingSetWithHandle(opened.handle);
    record = makeBackupRecord(target, original);
    cappedProcesses.set(target.pid, record);
    if (!persistBackups()) {
      cappedProcesses.delete(target.pid);
      return { ok: false, error: `PID ${target.pid}: could not save the working-set backup; no cap was applied.` };
    }

    const capBytes = limitMB * 1024 * 1024;
    const minBytes = Math.min(MIN_WORKING_SET_BYTES, capBytes);
    const changed = SetProcessWorkingSetSizeEx(opened.handle, minBytes, capBytes, HARD_CAP_FLAGS);
    if (!changed) {
      cappedProcesses.delete(target.pid);
      persistBackups();
      return { ok: false, error: `PID ${target.pid}: SetProcessWorkingSetSizeEx failed (error ${Number(GetLastError())})` };
    }
    capApplied = true;
    record.appliedMaximumMB = limitMB;
    persistBackups();

    let initialTrimOk = true;
    // Trim once on transition into hidden mode, only if the process is currently over the cap.
    // The hard max handles subsequent growth; we do not call EmptyWorkingSet in a loop.
    if (before.mb != null && before.mb > limitMB) {
      initialTrimOk = Boolean(EmptyWorkingSet(opened.handle));
    }
    const after = readWorkingSetWithHandle(opened.handle);
    log(`Adaptive working-set cap applied to ${target.role} PID ${target.pid}: ${limitMB} MB; ${roundMB(before.mb)} → ${roundMB(after.mb)} MB (reason=${reason}, initial trim ${initialTrimOk ? 'ok/skipped' : 'failed'}).`);
    return { ok: true, beforeMB: before.mb, afterMB: after.mb, trimOk: initialTrimOk };
  } catch (error) {
    if (record) {
      if (capApplied) {
        // If telemetry/trim fails after the limit was installed, roll it back before
        // discarding the backup. If rollback fails, keep the backup for later recovery.
        let restored = false;
        try { restored = Boolean(setOriginalLimits(record, opened.handle)); } catch (_) { }
        if (restored) cappedProcesses.delete(target.pid);
      } else {
        cappedProcesses.delete(target.pid);
      }
      persistBackups();
    }
    return { ok: false, error: `PID ${target.pid}: ${error.message}` };
  } finally {
    try { CloseHandle(opened.handle); } catch (_) { }
  }
}

function restoreOne(record, reason) {
  const opened = openProcess(record.pid, PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA);
  if (!opened.handle) {
    // ERROR_INVALID_PARAMETER (87) normally means this PID no longer exists. Other errors
    // may be access/transient failures, so preserve the recovery record and retry.
    if (opened.errorCode === 87 || opened.errorCode === 1168) {
      cappedProcesses.delete(record.pid);
      adaptiveProfiles.delete(record.pid);
      log(`Discarded recovery record for PID ${record.pid}: process no longer exists (${reason}).`, 'warn');
      return { ok: true, gone: true };
    }
    return { ok: false, retry: true, error: `PID ${record.pid}: could not open process for restoration (error ${opened.errorCode}); recovery record preserved.` };
  }
  try {
    // Validate the actual process handle, not a 2.5-second-old PowerShell process list.
    // This both avoids stale PID reuse and lets focus restoration happen without another query.
    const identity = verifyProcessIdentity(opened.handle, record.creation);
    if (!identity.ok) {
      if (identity.errorCode) return { ok: false, retry: true, error: `PID ${record.pid}: could not verify process identity during restoration (error ${identity.errorCode}); recovery record preserved.` };
      cappedProcesses.delete(record.pid);
      adaptiveProfiles.delete(record.pid);
      log(`Discarded recovery record for PID ${record.pid}: PID was reused; no limits were applied to the new process.`, 'warn');
      return { ok: true, gone: true };
    }
    if (!setOriginalLimits(record, opened.handle)) {
      return { ok: false, retry: true, error: `PID ${record.pid}: failed to restore original working-set limits (error ${Number(GetLastError())}).` };
    }
    const measured = readWorkingSetWithHandle(opened.handle);
    cappedProcesses.delete(record.pid);
    adaptiveProfiles.delete(record.pid);
    log(`Restored original limits for ${record.role} PID ${record.pid}: ${roundMB(measured.mb)} MB resident (${reason}).`);
    return { ok: true, workingSetMB: measured.mb };
  } finally {
    try { CloseHandle(opened.handle); } catch (_) { }
  }
}

function restoreRecords(records, reason) {
  const outcomes = [];
  for (const record of records) {
    try { outcomes.push(restoreOne(record, reason)); }
    catch (error) { outcomes.push({ ok: false, retry: true, error: `PID ${record.pid}: ${error.message}` }); }
  }
  persistBackups();
  const failed = outcomes.filter(outcome => !outcome.ok);
  for (const failure of failed) log(failure.error, 'warn');
  return { restored: outcomes.filter(outcome => outcome.ok && !outcome.gone).length, gone: outcomes.filter(outcome => outcome.gone).length, failed: failed.length };
}

function restoreAllCappedProcesses(reason) {
  if (!cappedProcesses.size) return { restored: 0, gone: 0, failed: 0 };
  // restoreOne verifies PID identity through GetProcessTimes; do not block focus recovery
  // on a fresh PowerShell/CIM snapshot.
  return restoreRecords([...cappedProcesses.values()], reason);
}

function restoreAllMeasured(reason) {
  const started = performance.now();
  const result = restoreAllCappedProcesses(reason);
  state.telemetry.lastRestoreDurationMs = Math.round((performance.now() - started) * 10) / 10;
  state.telemetry.lastRestoreAt = new Date().toISOString();
  state.telemetry.lastRestoreRestored = result.restored;
  state.telemetry.lastRestoreFailures = result.failed;
  return result;
}

function restorePersistedCapsAtStartup() {
  const records = readBackupFile();
  if (!records.length) return { restored: 0, gone: 0, failed: 0 };
  if (!refreshSnapshot()) {
    log(`There are ${records.length} recovery records, but process discovery failed; records were preserved.`, 'warn');
    records.forEach(record => cappedProcesses.set(record.pid, record));
    return { restored: 0, gone: 0, failed: records.length };
  }
  for (const record of records) cappedProcesses.set(record.pid, record);
  const result = restoreAllCappedProcesses('recovery from a previous run');
  log(`Recovery: ${result.restored} restored, ${result.gone} processes exited, ${result.failed} pending.`);
  return result;
}

function refreshMemory() {
  const sampledAtMs = Date.now();
  const previousSampledAtMs = state.memory.sampledAt ? Date.parse(state.memory.sampledAt) : null;
  const sampleIntervalMs = Number.isFinite(previousSampledAtMs) ? Math.max(0, sampledAtMs - previousSampledAtMs) : null;
  const descendants = allSpotifyDescendants();
  const targetsByPid = new Map(targetProcesses().map(p => [p.pid, p]));
  const all = [];
  const currentKeys = new Set();
  let pageFaultDelta = 0;
  let pageFaultMeasuredProcesses = 0;

  for (const p of state.snapshot.processes) {
    const stats = readWorkingSetMB(p.pid);
    const identityKey = `${p.pid}:${p.creation}`;
    currentKeys.add(identityKey);
    let faultsDelta = null;
    if (stats.pageFaultCount != null) {
      const previous = pageFaultBaselines.get(identityKey);
      if (previous != null) {
        faultsDelta = stats.pageFaultCount >= previous
          ? stats.pageFaultCount - previous
          : (stats.pageFaultCount + 0x100000000 - previous);
        pageFaultDelta += faultsDelta;
        pageFaultMeasuredProcesses++;
      }
      pageFaultBaselines.set(identityKey, stats.pageFaultCount);
    }
    all.push({
      pid: p.pid, creation: p.creation, workingSetMB: stats.mb,
      privateMB: p.privateMB, cpuPercent: p.cpuPercent,
      pageFaultCount: stats.pageFaultCount, pageFaultsDelta: faultsDelta, errorCode: stats.errorCode
    });
  }
  for (const key of pageFaultBaselines.keys()) {
    if (!currentKeys.has(key)) pageFaultBaselines.delete(key);
  }

  const byPid = new Map(all.map(p => [p.pid, p]));
  const targets = descendants.map(p => {
    const memory = byPid.get(p.pid) || { workingSetMB: null, pageFaultCount: null, pageFaultsDelta: null, errorCode: -1 };
    const target = targetsByPid.get(p.pid);
    const record = cappedProcesses.get(p.pid);
    const isCapped = Boolean(target && record && record.creation === p.creation);
    return {
      pid: p.pid,
      role: p.role,
      workingSetMB: memory.workingSetMB == null ? null : roundMB(memory.workingSetMB),
      pageFaultsDelta: memory.pageFaultsDelta,
      cpuPercent: p.cpuPercent,
      privateMB: memory.privateMB == null ? null : roundMB(memory.privateMB),
      errorCode: memory.errorCode,
      included: Boolean(target),
      protectedAudio: p.role === 'Audio',
      capped: isCapped,
      limitMB: isCapped ? (Number(record.appliedMaximumMB) || AdaptivePolicy.baseCapMB(p.role)) : null,
      adaptiveStatus: isCapped ? adaptiveProfiles.get(p.pid)?.lastReason || 'holding' : null
    };
  });
  const sum = list => list.reduce((total, p) => total + (p.workingSetMB || 0), 0);
  const targetRows = targets.filter(p => p.included);
  const seconds = sampleIntervalMs != null && sampleIntervalMs > 0 ? sampleIntervalMs / 1000 : null;
  state.telemetry.pageFaultDelta = pageFaultMeasuredProcesses ? pageFaultDelta : null;
  state.telemetry.pageFaultsPerSecond = seconds && pageFaultMeasuredProcesses
    ? Math.round((pageFaultDelta / seconds) * 10) / 10 : null;
  state.telemetry.sampleIntervalMs = sampleIntervalMs;
  state.memory = {
    spotifyTotalMB: roundMB(sum(all)),
    targetTotalMB: roundMB(sum(targetRows)),
    processCount: state.snapshot.processes.length,
    targetCount: targetRows.length,
    cappedCount: cappedProcesses.size,
    targets,
    sampledAt: new Date(sampledAtMs).toISOString(),
    pageFaultDelta: state.telemetry.pageFaultDelta,
    pageFaultsPerSecond: state.telemetry.pageFaultsPerSecond,
    sampleIntervalMs,
    pageFaultMeasuredProcesses
  };
}

function publicConfig() {
  return {
    autoOptimize: settings.autoOptimize,
    hiddenDelaySeconds: settings.hiddenDelaySeconds,
    policy: { minMB: AdaptivePolicy.MIN_CAP_MB, maxMB: AdaptivePolicy.MAX_CAP_MB, cpuHighPercent: 10 }
  };
}

function persistSettings() {
  const serialized = JSON.stringify({ ...settings, version: VERSION }, null, 2) + os.EOL;
  const tmp = `${SETTINGS_PATH}.tmp`;
  fs.writeFileSync(tmp, serialized, 'utf8');
  try { fs.renameSync(tmp, SETTINGS_PATH); }
  catch (_) { fs.copyFileSync(tmp, SETTINGS_PATH); fs.rmSync(tmp, { force: true }); }
}

function updateConfig(incoming) {
  if (!incoming || typeof incoming !== 'object' || Array.isArray(incoming)) throw new Error('Invalid configuration payload.');
  if (typeof incoming.autoOptimize === 'boolean') settings.autoOptimize = incoming.autoOptimize;
  settings.hiddenDelaySeconds = clamp(incoming.hiddenDelaySeconds, 1, 30, settings.hiddenDelaySeconds);
  persistSettings();
  log(`Settings updated: automatic optimization=${settings.autoOptimize}, hide delay=${settings.hiddenDelaySeconds}s.`);
  if (!settings.autoOptimize && cappedProcesses.size) restoreAllMeasured('automatic optimization disabled');
}

function ensureAdaptiveProfile(target, now) {
  let profile = adaptiveProfiles.get(target.pid);
  if (!profile || profile.creation !== target.creation) {
    profile = {
      creation: target.creation,
      currentCapMB: AdaptivePolicy.baseCapMB(target.role),
      highCpuMs: 0,
      lowCpuMs: 0,
      lastEvaluatedSnapshotAt: state.snapshotAtMs,
      lastAdjustmentAt: 0,
      lastReason: 'baseline',
      lastCpuPercent: null,
      lastWorkingSetMB: null,
      lastTrimAt: 0
    };
    adaptiveProfiles.set(target.pid, profile);
  }
  return profile;
}

function adjustWorkingSetCap(target, profile, nextCapMB, reason, now) {
  const record = cappedProcesses.get(target.pid);
  if (!record || record.creation !== target.creation) return false;
  const opened = openProcess(target.pid, PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA);
  if (!opened.handle) {
    log(`Adaptive cap update skipped for PID ${target.pid}: OpenProcess failed (${opened.errorCode}).`, 'warn');
    return false;
  }
  try {
    const identity = verifyProcessIdentity(opened.handle, target.creation);
    if (!identity.ok) {
      log(`Adaptive cap update skipped for PID ${target.pid}: process identity could not be verified.`, 'warn');
      return false;
    }
    const previousCapMB = profile.currentCapMB;
    const before = readWorkingSetWithHandle(opened.handle);
    const capBytes = nextCapMB * 1024 * 1024;
    const minBytes = Math.min(MIN_WORKING_SET_BYTES, capBytes);
    if (!SetProcessWorkingSetSizeEx(opened.handle, minBytes, capBytes, HARD_CAP_FLAGS)) {
      log(`Adaptive cap update failed for PID ${target.pid} (${Number(GetLastError())}).`, 'warn');
      return false;
    }
    record.appliedMaximumMB = nextCapMB;
    persistBackups();
    let trimmed = false;
    if (nextCapMB < profile.currentCapMB && before.mb != null && before.mb > nextCapMB + 4 && now - profile.lastTrimAt >= 15000) {
      // Trim only on an actual downward policy change, never in a polling loop.
      trimmed = Boolean(EmptyWorkingSet(opened.handle));
      profile.lastTrimAt = now;
    }
    profile.currentCapMB = nextCapMB;
    profile.lastAdjustmentAt = now;
    profile.lastReason = reason;
    profile.highCpuMs = 0;
    profile.lowCpuMs = 0;
    state.telemetry.adaptiveAdjustments += 1;
    state.telemetry.lastAdaptiveAdjustmentAt = new Date(now).toISOString();
    log(`Adaptive policy ${nextCapMB > previousCapMB ? 'raised' : 'lowered'} ${target.role} PID ${target.pid} to ${nextCapMB} MB (reason=${reason}, working set=${roundMB(before.mb)} MB${trimmed ? ', one-time trim requested' : ''}).`);
    return true;
  } finally {
    try { CloseHandle(opened.handle); } catch (_) { }
  }
}

function updateAdaptiveCaps(now) {
  const snapshotAt = state.snapshotAtMs;
  const snapshotFresh = snapshotAt > 0 && now - snapshotAt <= 6500;
  for (const target of targetProcesses()) {
    const record = cappedProcesses.get(target.pid);
    if (!record || record.creation !== target.creation) continue;
    const process = state.snapshot.processes.find(p => p.pid === target.pid && p.creation === target.creation);
    if (!process) continue;
    const profile = ensureAdaptiveProfile(target, now);
    const appliedCap = Number(record.appliedMaximumMB) || profile.currentCapMB;
    profile.currentCapMB = appliedCap;

    // Only count each actual process snapshot once. The 100 ms window poll must not
    // turn one stale CPU sample into six seconds of alleged sustained activity.
    if (!snapshotAt || profile.lastEvaluatedSnapshotAt === snapshotAt) continue;
    const sampleIntervalMs = profile.lastEvaluatedSnapshotAt
      ? Math.max(0, snapshotAt - profile.lastEvaluatedSnapshotAt)
      : 0;
    profile.lastEvaluatedSnapshotAt = snapshotAt;
    const cpu = snapshotFresh && Number.isFinite(process.cpuPercent) ? process.cpuPercent : null;
    profile.lastCpuPercent = cpu;
    profile.lastWorkingSetMB = process.workingSetMB;

    if (sampleIntervalMs <= 0 || sampleIntervalMs > 6500 || cpu == null) {
      profile.highCpuMs = 0;
      profile.lowCpuMs = 0;
      continue;
    }
    const atWorkingSetCeiling = Number.isFinite(process.workingSetMB) && process.workingSetMB >= appliedCap;
    if (cpu >= 10 && atWorkingSetCeiling) profile.highCpuMs += sampleIntervalMs;
    else profile.highCpuMs = 0;
    if (cpu <= 3 && Number.isFinite(process.workingSetMB) && process.workingSetMB <= appliedCap - 6) {
      profile.lowCpuMs += sampleIntervalMs;
    } else {
      profile.lowCpuMs = 0;
    }

    const decision = AdaptivePolicy.decideAdjustment({
      role: target.role,
      currentCapMB: appliedCap,
      workingSetMB: process.workingSetMB,
      cpuPercent: cpu,
      highCpuMs: profile.highCpuMs,
      lowCpuMs: profile.lowCpuMs,
      lastAdjustmentAt: profile.lastAdjustmentAt,
      now
    });
    if (decision) adjustWorkingSetCap(target, profile, decision.nextCapMB, decision.reason, now);
  }
}

function syncActiveCapsWhileHidden() {
  const now = Date.now();
  // Do not apply new native limits using a stale process tree after discovery fails.
  if (!state.snapshotOk || !state.snapshotAtMs || now - state.snapshotAtMs > 6500) return;
  const targets = targetProcesses();
  const wanted = new Set(targets.map(target => recordKey(target)));

  const noLongerTargets = [...cappedProcesses.values()].filter(record => !wanted.has(recordKey(record)));
  if (noLongerTargets.length) restoreRecords(noLongerTargets, 'process is no longer eligible for optimization');
  for (const key of targetRetryAt.keys()) if (!wanted.has(key)) targetRetryAt.delete(key);

  const applied = [];
  const failures = [];
  for (const target of targets) {
    const key = recordKey(target);
    const existing = cappedProcesses.get(target.pid);
    if (existing && existing.creation === target.creation) {
      targetRetryAt.delete(key);
      ensureAdaptiveProfile(target, now);
      continue;
    }
    if ((targetRetryAt.get(key) || 0) > now) continue;
    targetRetryAt.set(key, now + TARGET_RETRY_DELAY_MS);
    const initialCap = AdaptivePolicy.baseCapMB(target.role);
    const result = applyHardCap(target, 'Spotify hidden', initialCap);
    if (result.ok && !result.skipped) {
      targetRetryAt.delete(key);
      ensureAdaptiveProfile(target, now);
      applied.push({ pid: target.pid, role: target.role, beforeMB: result.beforeMB, afterMB: result.afterMB, limitMB: initialCap });
    } else if (!result.ok && result.error) failures.push(result.error);
  }

  updateAdaptiveCaps(Date.now());
  if (applied.length || failures.length) {
    state.lastAction = {
      time: new Date().toISOString(), reason: 'automatic', mode: 'adaptive',
      applied: applied.length, processes: applied, failures: failures.slice(0, 8)
    };
    if (applied.length) log(`Adaptive optimization started for ${applied.length} process(es); dedicated audio processes remain excluded.`);
    failures.slice(0, 6).forEach(message => log(message, 'warn'));
  }
}

function maybeAutoOptimize(now) {
  const hidden = state.windowState === 'minimized' || state.windowState === 'hidden';
  if (!hidden) {
    state.hiddenSince = 0;
    if (cappedProcesses.size && now - state.lastRestoreAttemptAt >= 1500) {
      state.lastRestoreAttemptAt = now;
      restoreAllMeasured(`window ${state.windowState}`);
    }
    return;
  }
  if (!settings.autoOptimize) {
    if (cappedProcesses.size && now - state.lastRestoreAttemptAt >= 1500) {
      state.lastRestoreAttemptAt = now;
      restoreAllMeasured('automatic optimization disabled');
    }
    state.hiddenSince = 0;
    return;
  }
  if (!state.hiddenSince) {
    state.hiddenSince = now;
    log(`Spotify entered ${state.windowState}; adaptive limits will begin after ${settings.hiddenDelaySeconds}s.`);
  }
  if (now - state.hiddenSince < settings.hiddenDelaySeconds * 1000 || state.busy) return;
  syncActiveCapsWhileHidden();
}

function constantTimeEquals(a, b) {
  const left = Buffer.from(String(a || ''), 'utf8');
  const right = Buffer.from(String(b || ''), 'utf8');
  return left.length === right.length && crypto.timingSafeEqual(left, right);
}

function trustedOrigin(origin) {
  if (typeof origin !== 'string' || !origin) return false;
  try {
    const url = new URL(origin);
    return url.protocol === 'https:' && (url.hostname === 'spotify.com' || url.hostname.endsWith('.spotify.com'));
  } catch (_) { return false; }
}

function authorized(req) {
  const header = req.headers.authorization || '';
  const match = /^Bearer\s+(.+)$/i.exec(header);
  return Boolean(match && constantTimeEquals(match[1], settings.token));
}

function writeJson(res, statusCode, payload, origin) {
  const headers = {
    'Content-Type': 'application/json; charset=utf-8',
    'Cache-Control': 'no-store',
    'X-Content-Type-Options': 'nosniff',
    'Connection': 'close'
  };
  if (origin && trustedOrigin(origin)) {
    headers['Access-Control-Allow-Origin'] = origin;
    headers['Access-Control-Allow-Methods'] = 'GET, POST, OPTIONS';
    headers['Access-Control-Allow-Headers'] = 'Authorization, Content-Type';
    headers['Access-Control-Allow-Private-Network'] = 'true';
    headers['Access-Control-Max-Age'] = '300';
    headers['Vary'] = 'Origin';
  }
  res.writeHead(statusCode, headers);
  res.end(JSON.stringify(payload));
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let content = '';
    req.on('data', chunk => {
      content += chunk;
      if (content.length > 32768) {
        reject(new Error('Request body is too large.'));
        req.destroy();
      }
    });
    req.on('end', () => {
      if (!content) return resolve({});
      try { resolve(JSON.parse(content)); } catch (_) { reject(new Error('Invalid JSON.')); }
    });
    req.on('error', reject);
  });
}

function statusPayload() {
  return {
    ok: true,
    helper: { running: true, version: VERSION, node: process.version, startedAt: state.startedAt },
    spotify: {
      found: Boolean(state.rootPid), rootPid: state.rootPid, windowTitle: state.windowTitle,
      windowState: state.windowState, hiddenSeconds: state.hiddenSince ? Math.floor((Date.now() - state.hiddenSince) / 1000) : 0
    },
    config: publicConfig(), memory: state.memory, lastAction: state.lastAction,
    telemetry: { ...state.telemetry },
    lastError: state.lastError, recentEvents: state.recentEvents,
    cap: { active: cappedProcesses.size > 0, cappedProcesses: cappedProcesses.size, adaptive: true, minMB: AdaptivePolicy.MIN_CAP_MB, maxMB: AdaptivePolicy.MAX_CAP_MB, audioProtected: true }
  };
}

const server = http.createServer(async (req, res) => {
  const origin = req.headers.origin || '';
  const host = String(req.headers.host || '').toLowerCase();
  if (host !== `127.0.0.1:${settings.port}` && host !== `localhost:${settings.port}`) {
    writeJson(res, 403, { ok: false, error: 'Host is not allowed.' }, null);
    return;
  }
  if (!trustedOrigin(origin)) {
    writeJson(res, 403, { ok: false, error: 'Origin is not allowed.' }, null);
    return;
  }
  if (req.method === 'OPTIONS') {
    const preflightHeaders = {
      'Access-Control-Allow-Origin': origin,
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Authorization, Content-Type',
      'Access-Control-Allow-Private-Network': 'true',
      'Access-Control-Max-Age': '300', 'Vary': 'Origin', 'Content-Length': '0'
    };
    res.writeHead(204, preflightHeaders);
    res.end();
    return;
  }
  if (!authorized(req)) {
    writeJson(res, 401, { ok: false, error: 'Invalid local token.' }, origin);
    return;
  }

  const url = new URL(req.url, `http://127.0.0.1:${settings.port}`);
  try {
    if (req.method === 'GET' && url.pathname === '/api/status') {
      writeJson(res, 200, statusPayload(), origin);
      return;
    }
    if (req.method === 'POST' && url.pathname === '/api/config') {
      updateConfig(await readBody(req));
      writeJson(res, 200, { ok: true, config: publicConfig() }, origin);
      return;
    }
    if (req.method === 'POST' && url.pathname === '/api/action') {
      const body = await readBody(req);
      if (body.action !== 'restoreNow') {
        writeJson(res, 400, { ok: false, error: 'Unknown action.' }, origin);
        return;
      }
      state.hiddenSince = Date.now();
      const result = restoreAllMeasured('manual request');
      writeJson(res, result.failed ? 409 : 200, { ok: result.failed === 0, ...result }, origin);
      return;
    }
    writeJson(res, 404, { ok: false, error: 'Endpoint not found.' }, origin);
  } catch (error) {
    writeJson(res, 400, { ok: false, error: error.message }, origin);
  }
});

function tick() {
  if (state.busy) return;
  const now = Date.now();
  // Snapshot discovery is async so PowerShell cannot stall focus detection/restoration.
  if ((!state.lastSnapshotAt || now - state.lastSnapshotAt >= 2500) &&
      (!lastSnapshotAttemptAt || now - lastSnapshotAttemptAt >= 2500)) refreshSnapshotAsync();
  state.windowState = getWindowState();
  if (now - (state.memory.sampledAt ? Date.parse(state.memory.sampledAt) : 0) >= 2000) refreshMemory();
  maybeAutoOptimize(now);
}

function tickSafe() {
  try { tick(); } catch (error) { log(`Unexpected monitor error: ${error.message}`, 'error'); }
}

function shutdown() {
  try {
    refreshSnapshot();
    restoreAllCappedProcesses('companion shutdown');
  } catch (_) { }
  try { server.close(); } catch (_) { }
  process.exit(0);
}

server.on('error', error => {
  log(`Local API could not start on port ${settings.port}: ${error.message}. Another helper instance may already be running.`, 'error');
  process.exitCode = 1;
  process.exit(1);
});

if (process.argv.includes('--restore-limits')) {
  // Installer/uninstaller recovery mode: restore any persisted limits and exit without a server.
  try {
    const result = restorePersistedCapsAtStartup();
    console.log(JSON.stringify({ mode: 'restore-limits', ...result, remaining: cappedProcesses.size }, null, 2));
    process.exitCode = result.failed ? 1 : 0;
  } catch (error) {
    console.error(`[Memory Sentry] Restore failed: ${error.message}`);
    process.exitCode = 1;
  }
} else if (process.argv.includes('--self-test')) {
  try {
    const snapshotOk = refreshSnapshot();
    state.windowState = getWindowState();
    refreshMemory();
    console.log(JSON.stringify({
      platform: process.platform, node: process.version, snapshotOk,
      spotifyFound: Boolean(state.rootPid), windowState: state.windowState,
      processCount: state.memory.processCount, targetCount: state.memory.targetCount,
      targetMB: state.memory.targetTotalMB, cappedCount: cappedProcesses.size,
      version: VERSION,
      note: 'Read-only self-test. It does not apply caps or clean working sets.'
    }, null, 2));
    process.exitCode = snapshotOk ? 0 : 1;
  } catch (error) {
    console.error(`[Memory Sentry] Self-test failed: ${error.message}`);
    process.exitCode = 1;
  }
} else {
  // Restore leftovers from a previously interrupted version before starting a new cap session.
  try { restorePersistedCapsAtStartup(); }
  catch (error) { log(`Failed to recover previous working-set limits: ${error.message}`, 'warn'); }

  server.listen(settings.port, '127.0.0.1', () => {
    log(`Memory Sentry v${VERSION} listening on 127.0.0.1:${settings.port}; focus polling=100 ms; adaptive working-set policy=${AdaptivePolicy.MIN_CAP_MB}-${AdaptivePolicy.MAX_CAP_MB} MB; dedicated audio is always excluded.`);
    refreshSnapshot();
    state.windowState = getWindowState();
    refreshMemory();
    // Poll window visibility/focus at 100 ms; the old 1 s cadence added needless latency.
    setInterval(tickSafe, 100);
  });
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
process.on('exit', () => {
  // Synchronous best-effort recovery for normal process exits. The JSON backup file
  // remains as a second recovery path if Node is force-killed or Windows restarts.
  try {
    if (cappedProcesses.size && state.snapshotOk) {
      for (const record of [...cappedProcesses.values()]) {
        const opened = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA, 0, record.pid);
        if (!opened) continue;
        try {
          const identity = verifyProcessIdentity(opened, record.creation);
          if (identity.ok) SetProcessWorkingSetSizeEx(opened, record.minimum, record.maximum, record.flags);
        } catch (_) { }
        try { CloseHandle(opened); } catch (_) { }
      }
    }
  } catch (_) { }
});
