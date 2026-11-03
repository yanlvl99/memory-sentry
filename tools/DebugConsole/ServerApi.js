'use strict';

// Talks to the local Memory Sentry server the same way the dashboard does: loopback HTTP,
// the bearer token and settings the server itself owns, and the origin the server trusts.
// Nothing here invents state - it reads what the server believes so the harness can compare
// that belief against Win32 ground truth.

const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');

const TRUSTED_ORIGIN = 'https://xpui.app.spotify.com';
const DEFAULT_PORT = 38472;

/// settings.json lives in the server's own data directory, and the harness has no reason to
/// duplicate the port or the token. The path can be overridden for an isolated test instance.
function readSettings(explicitPath) {
  const file =
    explicitPath ||
    path.join(
      process.env.LOCALAPPDATA || process.env.APPDATA || '',
      'MemorySentryForSpicetify',
      'settings.json',
    );
  if (!fs.existsSync(file)) throw new Error(`Server settings not found at ${file}`);
  const raw = JSON.parse(fs.readFileSync(file, 'utf8'));
  return { file, port: Number(raw.port) || DEFAULT_PORT, token: String(raw.token || '') };
}

function request(settings, method, endpoint, body) {
  return new Promise((resolve, reject) => {
    const payload = body === undefined ? null : JSON.stringify(body);
    const req = http.request(
      {
        host: '127.0.0.1',
        port: settings.port,
        path: endpoint,
        method,
        headers: {
          Host: `127.0.0.1:${settings.port}`,
          Origin: TRUSTED_ORIGIN,
          Authorization: `Bearer ${settings.token}`,
          'Content-Type': 'application/json',
          Connection: 'close',
          ...(payload ? { 'Content-Length': Buffer.byteLength(payload) } : {}),
        },
      },
      (res) => {
        const chunks = [];
        res.on('data', (chunk) => chunks.push(chunk));
        res.on('end', () => {
          const text = Buffer.concat(chunks).toString('utf8');
          let parsed = null;
          try {
            parsed = JSON.parse(text);
          } catch (_) {
            parsed = null;
          }
          if (res.statusCode >= 400) {
            reject(
              new Error(
                `${method} ${endpoint} -> HTTP ${res.statusCode}: ${
                  (parsed && parsed.error) || text.slice(0, 200)
                }`,
              ),
            );
            return;
          }
          resolve(parsed === null ? { raw: text } : parsed);
        });
      },
    );
    req.on('error', (error) =>
      reject(new Error(`${method} ${endpoint} failed: ${error.message} (is the server running?)`)),
    );
    req.setTimeout(8000, () => req.destroy(new Error(`${method} ${endpoint} timed out`)));
    if (payload) req.write(payload);
    req.end();
  });
}

const status = (settings) => request(settings, 'GET', '/api/status');
const setConfig = (settings, patch) => request(settings, 'POST', '/api/config', patch);
const action = (settings, body) => request(settings, 'POST', '/api/action', body);
/// The focus beacon the dashboard posts. The harness can send it directly to separate
/// "did the server act on a foreground claim" from "did the browser really fire the event".
const beacon = (settings, focused, visible) =>
  request(settings, 'POST', '/api/window-state', { focused, visible });

// ---------------------------------------------------------------------------
// debug API - only answered by a server built with --features debug-api
// ---------------------------------------------------------------------------

const debug = (settings, endpoint, body) =>
  request(settings, 'POST', `/api/debug/${endpoint}`, body === undefined ? {} : body);

/// Window candidates, classification and stored state, all as the product reads them.
const debugWindows = (settings) => debug(settings, 'windows');
/// Per-process memory figures, enforced limits and recovery records. `pid` narrows it to one.
const debugProbe = (settings, pid) =>
  debug(settings, 'probe', pid === undefined ? {} : { pid: Number(pid) });
const debugSystem = (settings) => debug(settings, 'system');
/// Arm a cap on one process through the server's own apply path. Addressed by identity (preferred,
/// survives a restart) or pid, plus the ceiling in MB.
const forceCap = (settings, { identity, pid, maximumMB }) =>
  debug(settings, 'force-cap', {
    ...(identity ? { identity } : {}),
    ...(pid ? { pid: Number(pid) } : {}),
    maximumMB: Number(maximumMB),
  });
const releaseCap = (settings, { pid, all = false } = {}) =>
  debug(settings, 'release-cap', all ? { all: true } : pid ? { pid: Number(pid) } : { all: true });
const emptyWorkingSet = (settings, pid) => debug(settings, 'empty', { pid: Number(pid) });
/// Drive a real window through the server: action is minimize|restore|maximize|bottom|top|focus|blur.
/// `blur` needs no hwnd (it focuses the shell), so hwnd defaults to 0. Always resolves - the server
/// answers 200 with {ok:true,path} or {ok:false,error}, letting the caller report a failed verb instead
/// of the HTTP client throwing (scenario cleanup fires verbs at windows that may already be gone).
const windowControl = (settings, { hwnd, action }) =>
  debug(settings, 'window-control', { hwnd: Number(hwnd) || 0, action });

/// Fail early with the reason, instead of letting a scenario report a confusing 404. The capability
/// bit comes from the status payload, so it reflects how the running binary was compiled.
async function requireDebugApi(settings) {
  const st = await status(settings);
  if (st.debugApi !== true) {
    throw new Error(
      'the running server was built without --features debug-api. Install a debug build: ' +
        'scripts/build-package.ps1 -DebugApi, then run the produced *-debug.msi.',
    );
  }
  return st;
}

/// Poll status until the predicate holds or the budget runs out. Returns the last sample
/// either way, plus whether the condition was met, so a caller can report a timeout as
/// inconclusive instead of a failure.
async function waitForStatus(settings, predicate, { timeoutMs = 20000, intervalMs = 250 } = {}) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  let achieved = false;
  while (Date.now() < deadline) {
    try {
      last = await status(settings);
      if (predicate(last)) {
        achieved = true;
        break;
      }
    } catch (error) {
      last = { error: error.message };
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  return { achieved, sample: last };
}

module.exports = {
  action,
  beacon,
  debug,
  debugProbe,
  debugSystem,
  debugWindows,
  emptyWorkingSet,
  forceCap,
  readSettings,
  releaseCap,
  request,
  requireDebugApi,
  setConfig,
  status,
  waitForStatus,
  windowControl,
};
