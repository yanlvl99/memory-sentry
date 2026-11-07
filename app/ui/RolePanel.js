// The role group drawer. Clicking a legend pill (or a bar segment) opens the whole role and
// lists every process in it. Each row keeps its OWN limit field keyed by the per-process identity
// (`Renderer:#7`, `Utility:network`, ...) — the group is only a container; the value is individual.
function MSSRolePanel(props) {
  const { role, processes, debug, memory, config, manualInputs, busy, manualReady, onCopy, onInput, onSchedule, onClear, onPin, onClose } = props;
  const canEdit = manualReady && config.enabled && !busy;
  const first = processes[0] || {};
  return h('div', { className: 'mss-role-panel', key: role },
    h('div', { className: 'mss-role-panel-head' },
      h('span', { className: 'mss-pid-dot', style: { background: mssPidColor(first.pid || 0) } }),
      h('strong', null, MSSRoleLabel(role)),
      h('span', { className: 'mss-role-panel-count' }, `× ${processes.length}`),
      h('button', { type: 'button', className: 'mss-icon-btn', 'aria-label': tx('closeOptions'), onClick: onClose }, '\u00D7')
    ),
    processes.map(process => {
      const key = MSSProcessKey(process);
      const defaultMB = Number(process.defaultLimitMB || 36);
      const displayedMemoryMB = memory.privateWorkingSet ? process.privateWorkingSetMB : process.workingSetMB;
      const memoryMB = Number(displayedMemoryMB || 0);
      // "pinned" is the SAVED intent (settings manual cap): it drives the segment control so a
      // focused user can still edit/release the pin. The status TAG, however, must reflect the LIVE
      // applied state (process.manual): in the foreground every limit is restored, so a saved pin
      // that is not currently applied should read the same "Sem limite" as an automatic process,
      // never a misleading "Manual". (Keeps parity with the Hero hover readout.)
      const pinned = process.manualCapMB != null;
      const ceilingMB = MSSCeilingMB(process, defaultMB);
      const over = ceilingMB != null && memoryMB > ceilingMB;
      const tagText = MSSProcessTag(process, over);
      return h('div', { className: 'mss-role-proc', key },
        h('span', { className: 'mss-proc-id' },
          h('span', { className: 'mss-pid-dot', style: { background: mssPidColor(process.pid) } }),
          h('span', { className: 'mss-role-proc-pid' }, String(process.pid)),
          process.subLabel ? h('span', { className: 'mss-role-proc-sub' }, process.subLabel) : null,
          debug && Number.isFinite(process.cpuPercent) ? h('span', { className: 'mss-role-proc-cpu' }, tx('cpu', { value: Number(process.cpuPercent).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }) })) : null,
          h('span', { className: 'mss-role-proc-mem' }, `${MSSFormatMB(displayedMemoryMB)} MB`),
          h('span', { className: `mss-tag${over ? ' mss-tag-over' : (process.capped ? ' mss-tag-on' : '')}` }, tagText)
        ),
        h('div', { className: 'mss-proc-manage' },
          canEdit ? h('div', { className: 'mss-seg mss-seg-proc', role: 'group', 'aria-label': tx('procModeLabel') },
            h('button', { type: 'button', className: `mss-seg-btn${pinned ? '' : ' mss-on'}`, 'aria-pressed': !pinned, disabled: busy || !config.enabled, title: tx('procAutoHint'), onClick: () => { if (pinned) onClear(process, key); } }, tx('segAuto')),
            h('button', { type: 'button', className: `mss-seg-btn${pinned ? ' mss-on' : ''}`, 'aria-pressed': pinned, disabled: busy || !config.enabled, title: tx('procManualHint'), onClick: () => { if (!pinned) onPin(process); } }, tx('segManual'))
          ) : null,
          canEdit && pinned ? h('label', { className: 'mss-identity-limit' },
            h('input', { id: `mss-limit-${key}`, className: 'mss-input', type: 'number', min: 8, max: 512, step: 1, 'aria-label': tx('manualLimit'), title: tx('manualLimit'), value: manualInputs[key] ?? String(process.manualCapMB ?? defaultMB), disabled: busy || !config.enabled, onChange: event => { const value = event.target.value; onInput(key, value); onSchedule(process, key, value); } }),
            h('span', { className: 'mss-small' }, 'MB')
          ) : null,
          h('button', { type: 'button', className: 'mss-chip-btn', onClick: () => onCopy(process.pid) }, tx('copyPid')),
          h('button', { type: 'button', className: 'mss-chip-btn', onClick: () => onCopy(`${process.subLabel ? MSSRoleLabel(process.role) + ' ' + process.subLabel + ' · ' : ''}${MSSFormatMB(displayedMemoryMB)} MB`) }, tx('copyMemory'))
        )
      );
    })
  );
}
