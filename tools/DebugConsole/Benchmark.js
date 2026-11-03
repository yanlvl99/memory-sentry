'use strict';

// Repeatable memory benchmark for one managed process, measured entirely through the server's debug
// API (no native layer here). It answers the half of docs/BENCHMARKING.md that a script CAN observe:
// how far a working-set ceiling cuts resident memory, and what that costs in page-fault rate and CPU,
// plus the engine's own restore telemetry when the ceiling comes back off.
//
// It deliberately does NOT invent the human-only half: "restore-to-interaction time" (focus until the
// UI accepts a click) is not observable from the API, so the verdict says so instead of guessing. A
// lower working set is reported as a lower working set - not as "memory freed", because trimmed pages
// may sit in the standby/transition lists and come back through a soft fault (see BENCHMARKING.md).
//
//   node Index.js benchmark [--identity NAME | --pid N] [--mb CEILING] [--seconds N] [--interval MS]
//
// Steps: baseline (no explicit ceiling) -> arm a ceiling -> sample under it -> release -> sample the
// rebound, capturing restore duration/failures. A cap armed in a state this configuration does not
// manage is handed back by the engine; the run detects that and says so rather than reporting a
// reduction that never happened. Cleanup releases every cap and restores the window.

const Api = require('./ServerApi.js');

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const median = (values) => {
  const finite = values.filter((v) => Number.isFinite(v)).sort((a, b) => a - b);
  if (!finite.length) return null;
  const mid = Math.floor(finite.length / 2);
  return finite.length % 2 ? finite[mid] : (finite[mid - 1] + finite[mid]) / 2;
};
const fixed = (value, digits = 1) => (Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : '-');
const pct = (before, after) =>
  Number.isFinite(before) && Number.isFinite(after) && before > 0 ? ((after - before) / before) * 100 : null;

/// One process's observable figures from a probe row. Kept flat so the median/report code stays simple.
function pick(probe, pid) {
  const process = (probe.processes || []).find((p) => Number(p.pid) === Number(pid));
  if (!process) return null;
  return {
    present: true,
    workingSetMB: process.workingSetMB,
    privateWorkingSetMB: process.privateWorkingSetMB,
    privateMB: process.privateMB,
    pageFaultsPerSec: process.pageFaultsPerSec,
    cpuPercent: process.cpuPercent,
    capped: !!process.capped,
    hardMax: !!(process.osLimits && process.osLimits.hardMaxEnabled),
    osMaxMB: process.osLimits && Number.isFinite(process.osLimits.maximumMB) ? process.osLimits.maximumMB : null,
    appliedMB: process.recovery && process.recovery.appliedMaximumMB != null ? process.recovery.appliedMaximumMB : null,
  };
}

/// Collect n samples of one pid, ignoring transient read errors so a hiccup does not abort the run.
async function sample(settings, pid, n, intervalMs) {
  const rows = [];
  for (let i = 0; i < n; i++) {
    try {
      const probe = await Api.debugProbe(settings);
      const row = pick(probe, pid);
      if (row) rows.push(row);
    } catch (_) {
      // A dropped sample is just a smaller median; the verdict counts how many were kept.
    }
    if (i < n - 1) await sleep(intervalMs);
  }
  return rows;
}

const medianField = (rows, field) => median(rows.map((r) => r[field]));

async function run(args) {
  const settings = Api.readSettings(args.settings);
  try {
    await Api.requireDebugApi(settings);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
    return;
  }

  const seconds = Number(args.seconds || 8);
  const intervalMs = Number(args.interval || 1000);
  const n = Math.max(2, Math.round((seconds * 1000) / intervalMs));

  const windows = await Api.debugWindows(settings);
  const probe = await Api.debugProbe(settings);
  const processes = probe.processes || [];

  let target;
  if (args.pid) target = processes.find((p) => Number(p.pid) === Number(args.pid));
  else if (args.identity) target = processes.find((p) => String(p.identity).includes(String(args.identity)));
  else {
    const finite = processes.filter((p) => Number.isFinite(p.workingSetMB));
    target = finite.length ? finite.reduce((a, b) => (a.workingSetMB <= b.workingSetMB ? a : b)) : null;
  }
  if (!target) {
    console.log('benchmark: no managed process to measure (start Spotify, or name one with --pid / --identity).');
    process.exitCode = 2;
    return;
  }
  const pid = Number(target.pid);
  const baselineWS = target.workingSetMB;
  // Default to cutting the observed baseline roughly in half: enough to bind and show a delta, not so
  // low it is guaranteed to thrash. --mb overrides for a specific ceiling.
  const ceiling = args.mb !== undefined ? Number(args.mb) : Math.max(4, Math.floor((baselineWS || 24) / 2));

  // The window verb comes from the server too. A main window is needed to move the app into a managed
  // background state so the engine leaves the ceiling in place while we sample.
  const rep = windows.live.representative;
  const mainCandidates = (windows.candidates || []).filter((c) => Number(c.pid) === Number(windows.stored.rootPid));
  const hwnd = rep ? rep.hwnd : mainCandidates.length ? mainCandidates[0].hwnd : null;
  const originalForeground = windows.live.foregroundHwnd;

  console.log(`=== benchmark ===`);
  console.log(`target   pid ${pid}  identity ${target.identity}  role ${target.role}`);
  console.log(`baseline working set ${fixed(baselineWS)}MB, ceiling to apply ${ceiling}MB, sampling ${n}x ${intervalMs}ms`);

  // Put the app in a background state (minimized) if we can reach one, then drop any cap so the
  // baseline is measured with no explicit ceiling in place.
  if (hwnd) await Api.windowControl(settings, { hwnd, action: 'minimize' });
  try {
    await Api.releaseCap(settings, { pid });
  } catch (_) {
    // Nothing recorded for this pid yet - that is the wanted baseline state.
  }
  await sleep(intervalMs);
  const before = await sample(settings, pid, n, intervalMs);
  const beforeCapped = before.filter((r) => r.capped).length;

  // Arm the ceiling through the product's own apply path and wait for the kernel to enforce it.
  let armed = false;
  try {
    await Api.forceCap(settings, { pid, maximumMB: ceiling });
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      const row = pick(await Api.debugProbe(settings), pid);
      if (row && row.capped && row.hardMax) {
        armed = true;
        break;
      }
      await sleep(200);
    }
  } catch (error) {
    console.log(`arm failed: ${error.message}`);
  }

  const during = armed ? await sample(settings, pid, n, intervalMs) : [];
  const duringCapped = during.filter((r) => r.capped).length;

  // Restore: release through the product, then sample the rebound and read the engine's restore
  // telemetry straight from status (lastRestoreRestored / failures / durationMs).
  let releaseNote = '';
  try {
    await Api.releaseCap(settings, { pid });
  } catch (error) {
    if (!/No cap is recorded/.test(error.message)) releaseNote = error.message;
  }
  await sleep(intervalMs);
  const after = await sample(settings, pid, n, intervalMs);

  let telemetry = {};
  try {
    const st = await Api.status(settings);
    telemetry = {
      restored: st.telemetry.lastRestoreRestored,
      failures: st.telemetry.lastRestoreFailures,
      durationMs: st.telemetry.lastRestoreDurationMs,
      at: st.telemetry.lastRestoreAt,
    };
  } catch (_) {
    // Telemetry is a bonus line; the memory deltas above already stand on their own.
  }

  const beforeWS = medianField(before, 'workingSetMB');
  const duringWS = medianField(during, 'workingSetMB');
  const afterWS = medianField(after, 'workingSetMB');
  const beforeFaults = medianField(before, 'pageFaultsPerSec');
  const duringFaults = medianField(during, 'pageFaultsPerSec');
  const beforeCpu = medianField(before, 'cpuPercent');
  const duringCpu = medianField(during, 'cpuPercent');
  const reduction = pct(beforeWS, duringWS);
  const faultIncrease = pct(beforeFaults, duringFaults);

  const table = [
    `samples kept:  baseline=${before.length}  capped=${during.length}  released=${after.length}`,
    `               workingSetMB   privateWS    faults/s     cpu%`,
    `baseline       ${fixed(beforeWS).padEnd(14)} ${fixed(medianField(before, 'privateWorkingSetMB')).padEnd(12)} ${fixed(beforeFaults).padEnd(11)} ${fixed(beforeCpu)}`,
    `capped(${ceiling}MB)   ${fixed(duringWS).padEnd(14)} ${fixed(medianField(during, 'privateWorkingSetMB')).padEnd(12)} ${fixed(duringFaults).padEnd(11)} ${fixed(duringCpu)}`,
    `released       ${fixed(afterWS).padEnd(14)} ${fixed(medianField(after, 'privateWorkingSetMB')).padEnd(12)} ${fixed(medianField(after, 'pageFaultsPerSec')).padEnd(11)} ${fixed(medianField(after, 'cpuPercent'))}`,
    `restore        restored=${telemetry.restored ?? '-'} failures=${telemetry.failures ?? '-'} durationMs=${fixed(telemetry.durationMs, 2)}${releaseNote ? `  (${releaseNote})` : ''}`,
  ];
  console.log('\n' + table.join('\n'));

  // The verdict. State the observed memory effect and the observed cost, then be explicit about what
  // this harness cannot see.
  let verdict;
  if (!armed) {
    verdict =
      `INCONCLUSIVE: no ceiling stayed enforced. With the window minimized the engine handed the limit ` +
      `back (capped samples: baseline ${beforeCapped}, capped ${duringCapped}), which means this ` +
      `configuration does not manage a reachable background state - check optimizeWhen* with "node Index.js system".`;
  } else if (reduction === null) {
    verdict = 'INCONCLUSIVE: not enough readable samples to compute a median.';
  } else {
    const savings = reduction <= 0 ? `reduced working set by ${fixed(-reduction)}% (${fixed(beforeWS)} -> ${fixed(duringWS)} MB)` : `working set rose ${fixed(reduction)}% (${fixed(beforeWS)} -> ${fixed(duringWS)} MB)`;
    const cost =
      faultIncrease === null
        ? 'page-fault cost not measurable'
        : `faults/s ${fixed(beforeFaults)} -> ${fixed(duringFaults)} (${fixed(faultIncrease, 0)}%), cpu ${fixed(beforeCpu)} -> ${fixed(duringCpu)}%`;
    const caution = Number.isFinite(faultIncrease) && faultIncrease > 100
      ? ' A large fault-rate rise suggests this ceiling may be too aggressive for this PC/workload.'
      : '';
    verdict =
      `${savings}; ${cost}.${caution} A lower working set is not proof memory was freed (trimmed pages can sit in standby). ` +
      `Restore-to-interaction time is human-only and not measured here.`;
  }
  console.log(`\nverdict: ${verdict}`);

  // Machine-readable summary, so the agent (or a script) can parse the figures without scraping text.
  console.log(
    '\n' +
      JSON.stringify(
        {
          benchmark: true,
          pid,
          identity: target.identity,
          role: target.role,
          ceilingMB: armed ? ceiling : null,
          armed,
          samples: { baseline: before.length, capped: during.length, released: after.length },
          workingSetMB: { baseline: beforeWS, capped: duringWS, released: afterWS },
          privateWorkingSetMB: { baseline: medianField(before, 'privateWorkingSetMB'), capped: medianField(during, 'privateWorkingSetMB') },
          pageFaultsPerSec: { baseline: beforeFaults, capped: duringFaults },
          cpuPercent: { baseline: beforeCpu, capped: duringCpu },
          reductionPct: reduction,
          faultRateDeltaPct: faultIncrease,
          restore: telemetry,
        },
        null,
        2,
      ),
  );

  // Never leave the machine in a state the benchmark created: release all caps, restore the window,
  // and hand focus back to whatever held it before.
  try {
    await Api.releaseCap(settings, { all: true });
    if (hwnd) await Api.windowControl(settings, { hwnd, action: 'restore' });
    if (originalForeground) {
      const winView = await Api.debugWindows(settings);
      const managed = new Set((winView.managedPids || []).map(Number));
      if (!managed.has(Number(winView.live.foregroundPid))) {
        await Api.windowControl(settings, { hwnd: originalForeground, action: 'focus' });
      }
    }
  } catch (_) {
    // Best-effort cleanup; the run already printed its verdict.
  }
}

module.exports = { run };
