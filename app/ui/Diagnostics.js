// Diagnostics console: the debug switch plus, when enabled, a tabbed read-only view of everything
// the local server reports. All fields already exist in /api/status, so this is display-only (no
// server change). Redesigned after the Android Settings guidelines: a glanceable
// overview with the current values inline, related fields grouped under a heading + divider, a text
// filter and a severity filter instead of one overwhelming stack, numeric columns right-aligned, and
// progressive disclosure for the raw payload. The status.lastError notice stays outside the debug
// gate because an active error must remain visible even in normal mode.
const MSS_DEBUG_TABS = ['overview', 'processes', 'activity', 'advanced'];
function MSSDebugField(label, value, mono) {
  return h('div', { className: 'mss-diag-cell' }, h('span', null, label), h('strong', { className: mono ? 'mss-mono' : undefined }, value));
}
function MSSDebugSection(title, children) {
  return h('div', { className: 'mss-diag-section' },
    title ? h('div', { className: 'mss-diag-section-head' }, title) : null,
    children
  );
}
function Diagnostics(props) {
  const R = MSSReact;
  const { status, telemetry, events, debug, onDebug } = props;
  const spotify = status?.spotify || {};
  const helper = status?.helper || {};
  const cap = status?.cap || {};
  const memory = status?.memory || {};
  const targets = Array.isArray(memory.targets) ? memory.targets : [];
  const [tab, setTab] = R.useState('overview');
  const [filter, setFilter] = R.useState('');
  const [severity, setSeverity] = R.useState('all');
  const num = (v, suffix = '') => (v == null ? '—' : `${v}${suffix}`);
  const pct = (v) => (Number.isFinite(v) ? `${v.toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 })}%` : '—');
  const usedMB = (t) => MSSFormatMB(memory.privateWorkingSet ? t.privateWorkingSetMB : t.workingSetMB);
  const needle = filter.trim().toLowerCase();
  const matches = (t) => !needle
    || String(t.pid).includes(needle)
    || String(t.role || '').toLowerCase().includes(needle)
    || String(t.identity || '').toLowerCase().includes(needle)
    || String(t.subLabel || '').toLowerCase().includes(needle);
  const shownTargets = targets.filter(matches);
  const shownEvents = events.filter((e) => severity === 'all' || (e.level || 'info') === severity);
  // Learned cross-session usage badge (local-only telemetry). Shows the App-Standby-style bucket the
  // server derived from this identity's busy EWMA; 'Learning' until a score has been persisted.
  const learnedBadge = (t) => (t.usageBucket
    ? h('span', { className: `mss-learned mss-learned-${t.usageBucket}`, title: `${tx('learnedProfile')} · ${tx(`bucket_${t.usageBucket}`)} · ${t.usageScore}%` }, tx(`bucket_${t.usageBucket}`))
    : h('span', { className: 'mss-learned mss-learned-none', title: tx('learnedHint') }, tx('learnedNone')));

  const overviewTab = h('div', { className: 'mss-diag-tabpane' },
    MSSDebugSection(tx('debugTelemetry'), h('div', { className: 'mss-diag-grid' },
      MSSDebugField(tx('pageFaults'), telemetry.pageFaultDelta == null ? '—' : Number(telemetry.pageFaultDelta).toLocaleString(MSS_LOCALE)),
      MSSDebugField(tx('pageFaultRate'), telemetry.pageFaultsPerSecond == null ? '—' : Number(telemetry.pageFaultsPerSecond).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 })),
      MSSDebugField(tx('adaptiveChanges'), Number(telemetry.adaptiveAdjustments || 0).toLocaleString(MSS_LOCALE)),
      MSSDebugField(tx('debugCapRange'), `${num(cap.minMB)}–${num(cap.maxMB)} MB`),
      MSSDebugField(tx('restoreDuration'), telemetry.lastRestoreDurationMs == null ? '—' : `${Number(telemetry.lastRestoreDurationMs).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 })} ms`),
      MSSDebugField(tx('debugRestoreRestored'), num(telemetry.lastRestoreRestored)),
      MSSDebugField(tx('debugRestoreFailures'), num(telemetry.lastRestoreFailures)),
      MSSDebugField(tx('debugSampleInterval'), telemetry.sampleIntervalMs == null ? '—' : `${telemetry.sampleIntervalMs} ms`)
    )),
    MSSDebugSection(tx('debugServerState'), h('div', { className: 'mss-diag-grid' },
      MSSDebugField(tx('debugServer'), `${helper.runtime || 'Server'} ${helper.version || ''}`.trim(), true),
      MSSDebugField(tx('debugStarted'), helper.startedAt ? MSSDate(helper.startedAt) : '—'),
      MSSDebugField(tx('debugRootPid'), num(spotify.rootPid), true),
      MSSDebugField(tx('debugWindowState'), MSSWindowLabel(spotify.windowState || 'not-detected')),
      MSSDebugField(tx('debugHiddenFor'), num(spotify.hiddenSeconds, ' s')),
      MSSDebugField(tx('debugRestoreAt'), telemetry.lastRestoreAt ? MSSDate(telemetry.lastRestoreAt) : '—'),
      memory.systemTotalMB ? MSSDebugField(tx('dbgSystemMemory'), `${MSSFormatMB(memory.systemAvailableMB)} / ${MSSFormatMB(memory.systemTotalMB)} MB`) : null,
      memory.pressureLevel ? MSSDebugField(tx('dbgPressure'), tx(`pressure_${memory.pressureLevel}`)) : null,
      spotify.windowStateReason ? MSSDebugField(tx('debugReason'), spotify.windowStateReason, true) : null,
      telemetry.lastAdaptiveAdjustmentAt ? MSSDebugField(tx('lastAdjustmentShort'), MSSDate(telemetry.lastAdaptiveAdjustmentAt)) : null
    ))
  );

  const processesTab = h('div', { className: 'mss-diag-tabpane' },
    h('div', { className: 'mss-diag-filterrow' },
      h('input', { className: 'mss-input mss-diag-filter', type: 'search', placeholder: tx('dbgFilterPlaceholder'), 'aria-label': tx('dbgFilterPlaceholder'), value: filter, onChange: (e) => setFilter(e.target.value) }),
      h('span', { className: 'mss-diag-count' }, tx('dbgShowing', { shown: shownTargets.length, total: targets.length }))
    ),
    shownTargets.length ? h('table', { className: 'mss-diag-table' },
      h('thead', null, h('tr', null, [tx('colRole'), 'PID', tx('colIdentity'), tx('colCreated'), tx('colUsed'), tx('colCap'), tx('colPin'), tx('colState'), tx('colLearned'), tx('colCpu'), tx('colFaults'), tx('colErr')].map((label, i) => h('th', { key: i }, label)))),
      h('tbody', null, shownTargets.map((t) => h('tr', { key: `${t.pid}:${t.creation}` },
        h('td', null, `${MSSRoleLabel(t.role)}${t.subLabel ? ` ${t.subLabel}` : ''}`),
        h('td', { className: 'mss-mono' }, String(t.pid)),
        h('td', { className: 'mss-mono' }, t.identity || '—'),
        h('td', { className: 'mss-mono' }, t.creation || '—'),
        h('td', { className: 'mss-diag-num' }, `${usedMB(t)} MB`),
        h('td', { className: 'mss-diag-num' }, t.capped ? `${MSSFormatMB(t.limitMB)} MB` : '—'),
        h('td', { className: 'mss-diag-num' }, t.manualCapMB != null ? `${t.manualCapMB} MB` : '—'),
        h('td', null, t.manual ? tx('tagManual') : (t.capped ? t.adaptiveStatus || tx('tagAdaptive') : tx('tagNoCeiling'))),
        h('td', null, learnedBadge(t)),
        h('td', { className: 'mss-diag-num' }, pct(t.cpuPercent)),
        h('td', { className: 'mss-diag-num' }, num(t.pageFaultsDelta)),
        h('td', { className: 'mss-diag-num' }, t.errorCode ? String(t.errorCode) : '—')
      )))
    ) : h('div', { className: 'mss-empty' }, tx('dbgNoMatch'))
  );

  const sevSeg = (value, label) => h('button', { type: 'button', key: value, className: `mss-seg-btn${severity === value ? ' mss-on' : ''}`, 'aria-pressed': severity === value, onClick: () => setSeverity(value) }, label);
  const activityTab = h('div', { className: 'mss-diag-tabpane' },
    h('div', { className: 'mss-diag-filterrow' },
      h('div', { className: 'mss-seg mss-seg-sev', role: 'group', 'aria-label': tx('dbgSevLabel') },
        sevSeg('all', tx('dbgSevAll')), sevSeg('info', tx('dbgSevInfo')), sevSeg('warn', tx('dbgSevWarn')), sevSeg('error', tx('dbgSevError'))
      ),
      h('span', { className: 'mss-diag-count' }, tx('dbgShowing', { shown: shownEvents.length, total: events.length }))
    ),
    h('div', { className: 'mss-small' }, tx('events')),
    shownEvents.length ? h('div', { className: 'mss-events' }, shownEvents.map((event, i) => h('div', { className: 'mss-event', key: `${event.time}-${i}` },
      h('span', { className: `mss-event-level mss-event-level-${event.level || 'info'}` }, tx(`dbgSev${((event.level || 'info').charAt(0).toUpperCase() + (event.level || 'info').slice(1))}`)),
      h('span', { className: 'mss-event-time' }, MSSDate(event.time)),
      h('span', { className: 'mss-event-msg' }, MSSLocalizeEvent(event.message))
    ))) : h('div', { className: 'mss-empty' }, tx('noEvents'))
  );

  const advancedTab = h('div', { className: 'mss-diag-tabpane' },
    h('div', { className: 'mss-actions' },
      h('button', { type: 'button', className: 'mss-btn mss-btn-secondary', onClick: () => mssCopy(JSON.stringify(status, null, 2)) }, tx('dbgCopyStatus')),
      h('button', { type: 'button', className: 'mss-btn mss-btn-secondary', onClick: () => mssCopy(events.map((e) => `${e.time || ''} [${e.level || 'info'}] ${e.message || ''}`).join('\n')) }, tx('dbgCopyEvents'))
    ),
    h('pre', { className: 'mss-diag-raw mss-mono' }, JSON.stringify(status, null, 2)),
    h('div', { className: 'mss-diag-footer' }, tx('debugPrivacy'))
  );

  const pane = tab === 'processes' ? processesTab : tab === 'activity' ? activityTab : tab === 'advanced' ? advancedTab : overviewTab;
  const tabButton = (id) => h('button', { type: 'button', key: id, className: `mss-seg-btn${tab === id ? ' mss-on' : ''}`, 'aria-pressed': tab === id, onClick: () => setTab(id) }, tx(`dbgTab${id.charAt(0).toUpperCase() + id.slice(1)}`));

  return h('section', { className: 'mss-diag' },
    h('div', { className: 'mss-diag-head' },
      h('div', null,
        h('div', { className: 'mss-diag-title' }, tx('debugMode')),
        h('div', { className: 'mss-small' }, tx('debugHint'))
      ),
      MSSDebugToggle(debug, onDebug)
    ),
    debug ? h('div', { className: 'mss-diag-body' },
      h('div', { className: 'mss-seg mss-diag-tabs', role: 'tablist' }, MSS_DEBUG_TABS.map(tabButton)),
      pane,
      status?.lastAction ? h('div', { className: 'mss-notice' }, tx('lastAction', { count: status.lastAction.applied || 0 })) : null
    ) : null,
    status?.lastError ? h('div', { className: 'mss-notice mss-notice-error' }, MSSLocalizeEvent(status.lastError)) : null
  );
}
