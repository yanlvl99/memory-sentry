// The hero card. Everything the tool "is" lives here: the composition bar, the mode picker
// (which now travels with the bar because they describe the same object), the hover readout,
// the role legend, and the expandable role panel. App.js passes the derived values; Hero just
// arranges them.
function Hero(props) {
  const {
    status, config, memory, spotify, debug, saving, busy, helperRunning,
    targets, sortedTargets, dispMB, totalMB, cappedCount, serverLabel,
    expandedRole, setExpandedRole, hoveredKey, setHoveredKey,
    manualInputs, manualMessage, manualReady,
    setMode, onSchedule, onClear, onPin, onCopy
  } = props;

  const modeDisabled = saving || !helperRunning || typeof status?.config?.enabled !== 'boolean';
  const isCustom = config.enabled && config.customMode;
  const isDisabled = !config.enabled;

  // A renderer sub-label is a live disambiguator only: the Chromium id is a per-session counter
  // that resets each launch, so it means nothing for a lone renderer. Utilities keep their stable
  // service name (network/media/...) unconditionally.
  const rendererCount = targets.filter(t => t.role === 'Renderer').length;
  const subFor = p => (p.subLabel && !(p.role === 'Renderer' && rendererCount < 2)) ? p.subLabel : null;

  const hovered = hoveredKey ? targets.find(p => `${p.pid}:${p.creation}` === hoveredKey) : null;
  let hoverBody;
  if (!hovered) hoverBody = h('span', { className: 'mss-hero-hover-idle' }, tx('compositionHint'));
  else {
    const mb = dispMB(hovered);
    const pct = totalMB > 0 ? Math.round((mb / totalMB) * 100) : 0;
    const ceiling = hovered.capped ? Number(hovered.limitMB ?? hovered.manualCapMB ?? Number(hovered.defaultLimitMB || 36)) : null;
    const stateLabel = hovered.manual ? tx('tagManual') : (hovered.capped ? tx('tagAdaptive') : tx('tagNoCeiling'));
    const parts = [MSSRoleLabel(hovered.role), String(hovered.pid), `${MSSFormatMB(mb)} MB`, tx('pctOfTotal', { value: pct }), stateLabel];
    if (hovered.subLabel && subFor(hovered)) parts.splice(1, 0, subFor(hovered));
    if (ceiling != null) parts.push(`${tx('capLabel')} ${MSSFormatMB(ceiling)} MB`);
    hoverBody = h('span', { className: 'mss-hero-hover-body' }, parts.join(' · '));
  }

  const roleLegend = (() => {
    const byRole = new Map();
    for (const p of sortedTargets) { if (!byRole.has(p.role)) byRole.set(p.role, { pid: p.pid, total: 0, count: 0 }); const entry = byRole.get(p.role); entry.total += dispMB(p); entry.count += 1; }
    return [...byRole.entries()];
  })();

  const expandedProcesses = expandedRole ? targets.filter(p => p.role === expandedRole) : [];

  return h('section', { className: 'mss-hero' },
    h('div', { className: 'mss-hero-head' },
      h('div', { className: 'mss-hero-id' },
        h('div', { className: 'mss-hero-title' }, tx('heroTitle')),
        h('div', { className: 'mss-hero-stats' },
          h('span', { className: 'mss-hero-stat' }, `${Number(memory.targetCount || 0).toLocaleString(MSS_LOCALE)} ${tx('processesCard').toLowerCase()} · ${cappedCount} ${tx('withCap')}`),
          targets.length && totalMB > 0 ? h('span', { className: 'mss-hero-stat mss-hero-stat-strong', title: tx(memory.privateWorkingSet ? 'privateWorkingSetNote' : 'ramNote') }, `${MSSFormatMB(totalMB)} MB`) : null,
          h('span', { className: 'mss-hero-stat mss-hero-stat-muted', title: debug ? (spotify.windowStateReason || '') : undefined }, `${MSSWindowLabel(spotify.windowState || 'not-detected')}${debug && spotify.windowStateReason ? ` (${String(spotify.windowStateReason).split(' ')[0]})` : ''} · ${serverLabel}`)
        )
      ),
      h('div', { className: 'mss-seg mss-seg-mode', role: 'group', 'aria-label': tx('modeLabel') },
        h(MSSSegButton, { label: tx('segCustom'), iconName: 'custom', active: isCustom, disabled: modeDisabled, iconOnly: true, onClick: () => { void setMode('custom'); } }),
        h(MSSSegButton, { label: tx('segDisabled'), iconName: 'disabled', active: isDisabled, disabled: modeDisabled, iconOnly: true, onClick: () => { void setMode('disabled'); } })
      )
    ),
    isDisabled ? h('div', { className: 'mss-notice mss-notice-warn', role: 'status' }, status?.cap?.cappedProcesses ? tx('pluginDisabledRestoring', { count: status.cap.cappedProcesses }) : tx('pluginDisabledNote')) : null,
    targets.length && totalMB > 0 ? h('div', { className: 'mss-composition', role: 'group', 'aria-label': tx('compositionAria') }, sortedTargets.map(p => {
      const key = `${p.pid}:${p.creation}`;
      const active = expandedRole === p.role;
      const mb = dispMB(p);
      const pct = Math.round((mb / totalMB) * 100);
      return h('button', { type: 'button', key, className: `mss-composition-seg${active ? ' mss-seg-active' : ''}`, style: { width: `${(mb / totalMB) * 100}%`, background: mssPidColor(p.pid) }, 'aria-expanded': active, 'aria-label': `${MSSRoleLabel(p.role)}${subFor(p) ? ' ' + subFor(p) : ''} · ${p.pid} · ${MSSFormatMB(mb)} MB · ${tx('pctOfTotal', { value: pct })}`, onMouseEnter: () => setHoveredKey(key), onMouseLeave: () => setHoveredKey(null), onFocus: () => setHoveredKey(key), onBlur: () => setHoveredKey(null), onClick: () => setExpandedRole(active ? null : p.role) });
    })) : null,
    targets.length ? h('div', { className: 'mss-hero-hover', role: 'status', 'aria-live': 'polite' }, hoverBody) : null,
    targets.length ? h('div', { className: 'mss-legend', role: 'group', 'aria-label': tx('legendAria') }, roleLegend.map(([role, info]) => {
      const active = expandedRole === role;
      return h('button', { type: 'button', key: role, className: `mss-legend-item${active ? ' mss-legend-active' : ''}`, 'aria-pressed': active, onClick: () => setExpandedRole(active ? null : role), title: `${MSSRoleLabel(role)} · ${MSSFormatMB(info.total)} MB` },
        h('span', { className: 'mss-legend-dot', 'aria-hidden': 'true', style: { background: mssPidColor(info.pid) } }),
        h('span', { className: 'mss-legend-label' }, MSSRoleLabel(role)),
        info.count > 1 ? h('span', { className: 'mss-legend-multi' }, `×${info.count}`) : null,
        h('span', { className: 'mss-legend-count' }, `${MSSFormatMB(info.total)} MB`)
      );
    })) : null,
    targets.length === 0 ? h('div', { className: 'mss-empty' }, tx('noneDetected')) : null,
    expandedRole ? h(MSSRolePanel, {
      role: expandedRole,
      processes: expandedProcesses,
      debug, memory, config, manualInputs, busy, manualReady,
      onCopy: onCopy, onSchedule: onSchedule, onClear: onClear, onPin: onPin,
      onClose: () => setExpandedRole(null)
    }) : null,
    manualMessage ? h('div', { className: `mss-notice${manualMessage.error ? ' mss-notice-error' : ''}` }, manualMessage.text) : null
  );
}
