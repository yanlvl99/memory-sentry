#!/usr/bin/env node
'use strict';

// Memory Sentry debug console.
//
// The server is the single source of truth and the single native layer. Every fact the harness needs
// - window candidates, classification, per-process memory, enforced limits, cap records - comes from
// its debug API, and every window verb (minimize/restore/maximize/z-order/focus/blur) is a POST to
// that API too. There is no local FFI: the Win32 calls that used to live here moved into the server's
// Windows crate so a debug run exercises the exact binary path a user's machine takes.
//
// Requires Node >= 18 and a server built with --features debug-api:
//   scripts/build-package.ps1 -DebugApi   then run the produced *-debug.msi (or swap the exe)
//
//   node Index.js status                     server belief
//   node Index.js windows                    candidates + classification as the server reads them
//   node Index.js compare                    stored vs live vs client, with the disagreements
//   node Index.js ram                        per-process memory, with the enforced-limit readback
//   node Index.js watch --seconds 30         sample all three over time
//   node Index.js force --identity Renderer --mb 24    arm a cap through the product's path
//   node Index.js release [--pid N | --all]            release it the same way
//   node Index.js empty --pid N                        trim a working set now
//   node Index.js min | restore | max | focus | back | top | blur   [--hwnd 0x.. | --pid N]
//   node Index.js beacon --focused true --visible true       post the dashboard's claim directly
//   node Index.js test smoke | bug2 | all                    automated scenarios
//   node Index.js benchmark [--pid N|--identity T] [--mb C] [--seconds N]   memory A/B via the API
//
// Window-control verbs act on the real desktop through the server. Each one prints the HWND it used and
// where that HWND came from, so a run is reproducible and never aims at a window nobody chose.

const Api = require('./ServerApi.js');
const Scenarios = require('./Scenarios.js');
const Benchmark = require('./Benchmark.js');

const parseArgs = (argv) => {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
    if (token.startsWith('--')) {
      const key = token.slice(2);
      const next = argv[i + 1];
      if (next !== undefined && !next.startsWith('--')) {
        out[key] = next;
        i++;
      } else {
        out[key] = true;
      }
    } else {
      out._.push(token);
    }
  }
  return out;
};

const flag = (value) => value === true || value === 'true' || value === '1';
const num = (value) => (value === undefined || value === null ? null : Number(value));
// ASCII on purpose: a Windows console without a UTF-8 code page turns an em dash into mojibake, and
// this output gets pasted into reports.
const fixed = (value, digits = 1) =>
  Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : '-';

// Handles are numeric in the server payload; this is display-only formatting, not a Win32 call.
const hex = (value) => `0x${Number(value).toString(16)}`;
// Number("0x...") already parses a hex string, so one coercion covers --hwnd in either base.
const parseHwnd = (value) => Number(value);

const print = (row) => console.log(row);
const json = (value) => print(JSON.stringify(value, null, 2));

/// The OS foreground as the server reads it: pid and raw handle. Returned instead of a local FFI call
/// so "who has focus" comes from the same query the engine's classification rule uses.
const foregroundOf = (windows) =>
  windows && windows.live
    ? { pid: windows.live.foregroundPid ?? null, hwnd: windows.live.foregroundHwnd ?? null }
    : { pid: null, hwnd: null };

/// Everything one command needs, fetched once: the server's belief, its live window view, and the
/// per-process truth (memory, enforced limits, cap records) from the same source.
async function collect(args) {
  const settings = Api.readSettings(args.settings);
  const status = await Api.requireDebugApi(settings);
  const windows = await Api.debugWindows(settings);
  const probe = await Api.debugProbe(settings);
  return { settings, status, windows, probe, foreground: foregroundOf(windows) };
}

const managedPids = (view) => (view.windows.live ? view.windows.managedPids : []).map(Number);

/// The window a verb should act on: an explicit --hwnd always wins, then the server's own
/// representative for the given --pid, then the representative it already picked. hwnd stays numeric
/// (that is what the server expects); hwndHex is for the printed line only.
function resolveTargets(args, view) {
  const candidates = view.windows.candidates || [];
  if (args.hwnd) {
    const hwnd = parseHwnd(args.hwnd);
    return [{ hwnd, hwndHex: hex(hwnd), source: '--hwnd', pid: null }];
  }
  if (args.pid) {
    const wanted = Number(args.pid);
    const mine = candidates.filter((c) => Number(c.pid) === wanted);
    if (!mine.length) throw new Error(`The server has no window candidate for PID ${wanted}.`);
    return mine.map((c) => ({ hwnd: c.hwnd, hwndHex: hex(c.hwnd), pid: c.pid, title: c.title, area: c.area }));
  }
  const rep = view.windows.live.representative;
  if (!rep) throw new Error('the server finds no window candidate at all (nothing to act on)');
  return [{ hwnd: rep.hwnd, hwndHex: hex(rep.hwnd), pid: rep.pid, title: null, area: rep.area, source: 'server representative' }];
}

function viewRow(view) {
  const stored = view.windows.stored;
  const live = view.windows.live;
  return {
    storedState: stored.windowState,
    storedReason: stored.windowStateReason,
    liveState: live.derivedState,
    liveForeground: live.spotifyIsForeground,
    foregroundPid: live.foregroundPid,
    foregroundHwnd: live.foregroundHwnd,
    rootPid: stored.rootPid,
    candidates: (view.windows.candidates || []).length,
    clientForeground: stored.clientForeground,
    clientStateAgeSeconds: stored.clientStateAgeSeconds,
    statusState: view.status.spotify.windowState,
    cappedServer: view.status.cap.cappedProcesses,
    cappedOs: (view.probe.processes || []).filter((p) => p.osLimits && p.osLimits.hardMaxEnabled).length,
  };
}

const COMMANDS = {
  async status(args) {
    const view = await collect(args);
    json(viewRow(view));
  },

  async windows(args) {
    const view = await collect(args);
    json(view.windows);
  },

  /// Three readings of the same moment, side by side, all from the server. stored is what it last
  /// decided, live is its rule applied to fresh candidates, and the per-process block is the enforced
  /// limits read straight back from the kernel. A disagreement between stored and live is a timing or
  /// caching bug; a process whose recorded cap and OS ceiling disagree is an enforcement bug.
  async compare(args) {
    const view = await collect(args);
    const row = viewRow(view);
    const lines = [
      `stored windowState    ${row.storedState}`,
      `  reason              ${row.storedReason || '-'}`,
      `live windowState      ${row.liveState} (server rule on fresh candidates)`,
      `server root pid       ${row.rootPid ?? '-'}  candidates=${row.candidates}`,
      `client beacon         ${row.clientForeground} (age ${fixed(row.clientStateAgeSeconds, 0)}s)`,
      `os foreground         pid=${row.foregroundPid ?? '-'} hwnd=${row.foregroundHwnd ? hex(row.foregroundHwnd) : '-'}`,
      `server says fg        ${row.liveForeground} (foregroundPid=${row.foregroundPid ?? '-'})`,
      `capped  server=${row.cappedServer}  os=${row.cappedOs}`,
    ];
    const conflicts = [];
    if (row.storedState !== row.liveState)
      conflicts.push(`server stored "${row.storedState}" but fresh data says "${row.liveState}"`);
    if (row.clientForeground && !row.liveForeground)
      conflicts.push('the renderer claims focus but the OS foreground is not Spotify (the bug-2 shape)');
    if (!row.clientForeground && row.liveForeground && row.clientStateAgeSeconds !== null && row.clientStateAgeSeconds > 5)
      conflicts.push(`Spotify owns the foreground but no fresh beacon (age ${fixed(row.clientStateAgeSeconds, 0)}s) - restore must fall back to window state`);
    for (const p of view.probe.processes || []) {
      const enforced = p.osLimits && p.osLimits.hardMaxEnabled;
      if (p.capped && enforced === false)
        conflicts.push(`PID ${p.pid} ${p.identity}: server holds a cap the OS is not enforcing`);
      if (!p.capped && enforced === true)
        conflicts.push(`PID ${p.pid} ${p.identity}: OS enforces a ceiling the server does not record`);
    }
    lines.push(`conflicts             ${conflicts.length ? `${conflicts.length} ->` : 'none'}`);
    for (const c of conflicts) lines.push(`  ! ${c}`);
    print(lines.join('\n'));
  },

  /// Per-process memory, with the enforcement check as the last column: the ceiling the server recorded
  /// (applied) against the maximum the OS reports it is enforcing right now (osMax/osHardMax, read back
  /// by a separate kernel call from the one that set it). "enforced==applied" is what makes a cap
  /// meaningful - a limit that never binds to the kernel is not a cap.
  async ram(args) {
    const view = await collect(args);
    const rows = [
      'pid    role       identity              ws(MB)  privWS   capped  applied  osMax   osHardMax  enforced==applied',
    ];
    for (const p of view.probe.processes || []) {
      if (args.pid && Number(p.pid) !== Number(args.pid)) continue;
      if (args.identity && !String(p.identity).includes(String(args.identity))) continue;
      const applied = p.recovery && p.recovery.appliedMaximumMB != null ? p.recovery.appliedMaximumMB : null;
      const serverMax = p.osLimits && Number.isFinite(p.osLimits.maximumMB) ? p.osLimits.maximumMB : null;
      const hardMax = !!(p.osLimits && p.osLimits.hardMaxEnabled);
      const agree = p.capped
        ? applied !== null && serverMax !== null && Math.abs(applied - serverMax) <= 1 && hardMax
        : !hardMax;
      rows.push(
        [
          String(p.pid).padEnd(6),
          String(p.role).padEnd(10),
          String(p.identity).padEnd(20),
          String(fixed(p.workingSetMB)).padEnd(7),
          String(fixed(p.privateWorkingSetMB)).padEnd(8),
          String(!!p.capped).padEnd(7),
          String(applied != null ? applied : '-').padEnd(8),
          String(fixed(serverMax, 0)).padEnd(7),
          String(p.osLimits ? p.osLimits.hardMaxEnabled : '-').padEnd(10),
          agree ? 'yes' : 'NO',
        ].join(' '),
      );
    }
    print(rows.join('\n'));
  },

  async watch(args) {
    const seconds = Number(args.seconds || 20);
    const interval = Number(args.interval || 1000);
    const settings = Api.readSettings(args.settings);
    await Api.requireDebugApi(settings);
    print(`watching ${seconds}s every ${interval}ms (Ctrl+C stops)`);
    const deadline = Date.now() + seconds * 1000;
    while (Date.now() < deadline) {
      const view = await collect(args);
      const row = viewRow(view);
      const capped = (view.probe.processes || []).filter((p) => p.capped).length;
      print(
        new Date().toISOString().slice(11, 23) +
          ` stored=${row.storedState} live=${row.liveState} client=${row.clientForeground}/${fixed(row.clientStateAgeSeconds, 0)}s` +
          ` osFg=${row.foregroundPid ?? '-'} capped=${capped}/${(view.probe.processes || []).length}` +
          ` wins=${row.candidates}`,
      );
      await new Promise((r) => setTimeout(r, interval));
    }
  },

  async beacon(args) {
    const settings = Api.readSettings(args.settings);
    const focused = flag(args.focused);
    const visible = flag(args.visible);
    const sent = await Api.beacon(settings, focused, visible);
    const after = await Api.debugWindows(settings);
    json({ sent: { focused, visible }, result: sent, stored: after.stored, live: after.live });
  },

  /// Arm a cap on one process through the server's own apply path, then show the enforced readback.
  async force(args) {
    const view = await collect(args);
    const maximumMB = num(args.mb ?? args.maximumMB);
    if (!Number.isFinite(maximumMB)) throw new Error('--mb <MB> is required (4..64).');
    if (!args.identity && !args.pid) throw new Error('name the target with --identity NAME or --pid N.');
    const result = await Api.forceCap(view.settings, {
      identity: args.identity ? String(args.identity) : undefined,
      pid: args.pid ? Number(args.pid) : undefined,
      maximumMB,
    });
    json({ requested: { identity: args.identity ?? null, pid: args.pid ?? null, maximumMB }, probe: result });
  },

  async release(args) {
    const view = await collect(args);
    const scope = { all: flag(args.all) || !args.pid, pid: args.pid ? Number(args.pid) : undefined };
    const result = await Api.releaseCap(view.settings, scope);
    json({ scope: flag(args.all) || !args.pid ? 'all' : scope, result });
  },

  async empty(args) {
    const view = await collect(args);
    if (!args.pid) throw new Error('--pid N is required.');
    const result = await Api.emptyWorkingSet(view.settings, Number(args.pid));
    const probe = await Api.debugProbe(view.settings, Number(args.pid));
    json({ result, workingSetMB: probe.processes[0] ? probe.processes[0].workingSetMB : null });
  },

  async system(args) {
    const settings = Api.readSettings(args.settings);
    await Api.requireDebugApi(settings);
    json(await Api.debugSystem(settings));
  },

  async probe(args) {
    const settings = Api.readSettings(args.settings);
    await Api.requireDebugApi(settings);
    json(await Api.debugProbe(settings, args.pid ? Number(args.pid) : undefined));
  },

  async min(args) {
    await control(args, 'minimize');
  },
  async restore(args) {
    await control(args, 'restore');
  },
  async max(args) {
    await control(args, 'maximize');
  },
  async back(args) {
    await control(args, 'bottom');
  },
  async top(args) {
    await control(args, 'top');
  },

  /// Focus and blur go through the same endpoint but report differently, so they get their own handler.
  /// blur needs no window (it hands focus to the shell); focus resolves the target like the verbs do.
  async blur(args) {
    const settings = Api.readSettings(args.settings);
    await Api.requireDebugApi(settings);
    const result = await Api.windowControl(settings, { action: 'blur' });
    const after = await Api.debugWindows(settings);
    json({ action: 'blur', result, foregroundPid: after.live.foregroundPid, derivedState: after.live.derivedState });
  },

  async focus(args) {
    const view = await collect(args);
    const targets = resolveTargets(args, view);
    const results = [];
    for (const t of targets) results.push({ hwnd: t.hwndHex, ...(await Api.windowControl(view.settings, { hwnd: t.hwnd, action: 'focus' })) });
    const after = await Api.debugWindows(view.settings);
    json({ action: 'focus', targets: results, osForeground: foregroundOf(after) });
  },

  async test(args) {
    // args._[0] is the word "test" itself; the scenario name follows it.
    await Scenarios.run(args._[1] || 'bug2', args);
  },

  /// A repeatable memory A/B for one process, measured entirely through the debug API. Different from
  /// the pass/fail scenarios: it returns figures (working set, page-fault rate, CPU, restore telemetry)
  /// plus a verdict about the observable half, and never claims the human-only restore-to-interaction time.
  async benchmark(args) {
    await Benchmark.run(args);
  },

  async help() {
    print(
      [
        'Memory Sentry debug console (pure HTTP client; no native layer, no dependencies).',
        'Needs a server built with --features debug-api (scripts/build-package.ps1 -DebugApi).',
        '  status | windows | compare | ram | probe | system | watch | beacon | test <scenario>',
        '  force --identity NAME --mb N | release [--pid N | --all] | empty --pid N',
        '  min | restore | max | focus | back | top | blur   [--hwnd 0x.. | --pid N]',
        '  watch --seconds N --interval MS   ram [--pid N] [--identity Text]',
        '  beacon --focused true --visible true        test smoke|bug2|all',
        '  benchmark [--pid N|--identity T] [--mb C] [--seconds N] [--interval MS]',
        'Common: --settings <path> for an isolated server instance.',
      ].join('\n'),
    );
  },
};

async function control(args, verb) {
  const view = await collect(args);
  const targets = resolveTargets(args, view);
  const results = [];
  for (const t of targets) {
    const sent = await Api.windowControl(view.settings, { hwnd: t.hwnd, action: verb });
    results.push({ hwnd: t.hwndHex, pid: t.pid, from: t.source || 'server candidate', sent });
  }
  // Report the OS truth after the verb, because "the request was queued" only means the server posted
  // it - the state change is what the next read shows.
  const after = await Api.debugWindows(view.settings);
  json({
    action: verb,
    targets: results,
    storedState: after.stored.windowState,
    liveState: after.live.derivedState,
    candidates: (after.candidates || []).map((c) => `${hex(c.hwnd)}/${c.iconic ? 'iconic' : c.onScreen ? 'on-screen' : 'off'}`),
  });
}

(async () => {
  const args = parseArgs(process.argv.slice(2));
  const command = args._[0] || 'help';
  const handler = COMMANDS[command];
  if (!handler) {
    console.error(`Unknown command: ${command}`);
    await COMMANDS.help();
    process.exitCode = 2;
    return;
  }
  try {
    await handler(args);
  } catch (error) {
    console.error(`ERROR: ${error.message}`);
    process.exitCode = 1;
  }
})();
