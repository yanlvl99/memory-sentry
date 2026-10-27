// Memory Sentry Custom App entry point: mounts the dashboard and guards the bundle.
// The installer replaces __MSS_TOKEN__ with an internal per-install access token.
// Spicetify concatenates the manifest subfiles before this file, so the token must stay here.

const MSS_TOKEN = '__MSS_TOKEN__';

function MSSPositionPeek(event) {
  const cell = event && event.currentTarget;
  if (!cell || typeof cell.getBoundingClientRect !== 'function') return;
  const peek = cell.querySelector('.mss-peek');
  if (!peek) return;
  const rect = cell.getBoundingClientRect();
  const point = typeof event.clientX === 'number' ? event.clientX - rect.left : rect.width / 2;
  const half = (peek.offsetWidth || 0) / 2;
  const min = Math.min(half, rect.width / 2);
  const max = Math.max(rect.width - half, rect.width / 2);
  peek.style.left = `${Math.min(Math.max(point, min), max)}px`;
}

function MSSDebugToggle(value, onChange) {
  const Toggle = Spicetify.ReactComponent && Spicetify.ReactComponent.Toggle;
  if (typeof Toggle !== 'function') {
    return h('button', { className: 'mss-debug-fallback', type: 'button', role: 'switch', 'aria-checked': Boolean(value), 'aria-label': tx('debugMode'), onClick: () => onChange(!value) }, h('span', { className: 'mss-debug-fallback-thumb', 'aria-hidden': 'true' }));
  }
  return h(Toggle, {
    value: Boolean(value),
    onSelected: next => onChange(typeof next === 'boolean' ? next : !value),
    id: 'mss-debug-mode',
    'aria-label': tx('debugMode')
  });
}

function MSSApp() {
  const R = MSSReact;
  const [status, setStatus] = R.useState(null);
  const [error, setError] = R.useState('');
  const [settingsError, setSettingsError] = R.useState('');
  const [busy, setBusy] = R.useState(false);
  const [saving, setSaving] = R.useState(false);
  const [settingsLoaded, setSettingsLoaded] = R.useState(false);
  const [savedAutoOptimize, setSavedAutoOptimize] = R.useState(null);
  const [debug, setDebug] = R.useState(() => { try { return window.localStorage.getItem('mss.debugMode') === '1'; } catch (_) { return false; } });
  const [config, setConfig] = R.useState({ enabled: true, autoOptimize: true, hiddenDelaySeconds: 2, trimToCap: true, optimizeWhenBackground: true, optimizeWhenMinimized: true, optimizeWhenHidden: true });
  const [manualInputs, setManualInputs] = R.useState({});
  const [manualWarning, setManualWarning] = R.useState(null);
  const manualApplyTimers = R.useRef(new Map());
  const manualInputOrigins = R.useRef(new Map());
  const confirmedManualWarning = R.useRef(null);
  const previousWindowState = R.useRef(null);
  const screenFlashNode = R.useRef(null);
  const screenFlashTimer = R.useRef(null);
  const [manualMessage, setManualMessage] = R.useState(null);

  function applyDebug(next) {
    const enabled = Boolean(next);
    setDebug(enabled);
    try { window.localStorage.setItem('mss.debugMode', enabled ? '1' : '0'); } catch (_) { }
  }
  function showScreenFlash(enabled) {
    if (screenFlashTimer.current) window.clearTimeout(screenFlashTimer.current);
    screenFlashNode.current?.remove();
    const overlay = document.createElement('div');
    overlay.className = `mss-screen-flash ${enabled ? 'mss-screen-flash-on' : 'mss-screen-flash-off'}`;
    overlay.setAttribute('role', 'status');
    overlay.setAttribute('aria-live', 'polite');
    const label = document.createElement('span');
    label.textContent = tx(enabled ? 'pluginActivatedFlash' : 'pluginDisabledFlash');
    overlay.appendChild(label);
    document.body.appendChild(overlay);
    screenFlashNode.current = overlay;
    screenFlashTimer.current = window.setTimeout(() => {
      overlay.remove();
      if (screenFlashNode.current === overlay) screenFlashNode.current = null;
      screenFlashTimer.current = null;
    }, 560);
  }
  async function setMode(mode) {
    if (saving || !helperRunning || typeof status?.config?.enabled !== 'boolean') return;
    setManualWarning(null);
    setManualMessage(null);
    const nextConfig = { ...config, enabled: mode !== 'disabled', autoOptimize: mode === 'automatic' };
    const saved = await saveSettings(nextConfig);
    if (!saved) return;
    if (config.enabled !== nextConfig.enabled) showScreenFlash(nextConfig.enabled);
  }

  const triggerKeys = ['optimizeWhenBackground', 'optimizeWhenMinimized', 'optimizeWhenHidden'];
  const activeTriggers = () => triggerKeys.filter(key => config[key]).length;
  function setTrigger(key, value) {
    if (!value && activeTriggers() <= 1) return;
    setField(key, value);
  }

  R.useEffect(() => {
    let alive = true;
    async function refresh() {
      try {
        const data = await MSSRequest('/api/status');
        if (!alive) return;
        setStatus(data); setError('');
        if (!settingsLoaded && data.config) {
          const loadedConfig = { enabled: data.config.enabled !== false, autoOptimize: data.config.autoOptimize !== false, hiddenDelaySeconds: data.config.hiddenDelaySeconds || 2, trimToCap: data.config.trimToCap !== false, optimizeWhenBackground: data.config.optimizeWhenBackground !== false, optimizeWhenMinimized: data.config.optimizeWhenMinimized !== false, optimizeWhenHidden: data.config.optimizeWhenHidden !== false };
          setConfig(loadedConfig);
          setSavedAutoOptimize(loadedConfig.autoOptimize);
          setSettingsLoaded(true);
        }
      } catch (err) {
        if (alive) {
          const detail = err && err.message ? String(err.message) : String(err || 'Unknown error');
          setError(tx('statusError') + ' [' + detail + ']');
        }
      }
    }
    refresh();
    const timer = window.setInterval(refresh, 2500);
    return () => { alive = false; window.clearInterval(timer); };
  }, [settingsLoaded]);

  R.useEffect(() => () => {
    for (const timer of manualApplyTimers.current.values()) window.clearTimeout(timer);
    manualApplyTimers.current.clear();
    manualInputOrigins.current.clear();
    if (screenFlashTimer.current) window.clearTimeout(screenFlashTimer.current);
    screenFlashNode.current?.remove();
  }, []);

  R.useEffect(() => {
    const windowState = status?.spotify?.windowState;
    if (!windowState) return;
    if (windowState === 'foreground' && previousWindowState.current && previousWindowState.current !== 'foreground') {
      setManualInputs({});
      manualInputOrigins.current.clear();
    }
    previousWindowState.current = windowState;
  }, [status?.spotify?.windowState]);

  R.useEffect(() => {
    if (!manualWarning) return undefined;
    const popup = Spicetify.PopupModal;
    const usePopupModal = typeof popup?.display === 'function' && typeof popup?.hide === 'function';
    const warning = manualWarning;
    const title = tx('lowLimitTitle', { role: MSSRoleLabel(warning.role), pid: warning.pid });
    const panel = document.createElement('section');
    panel.className = 'mss-modal';
    panel.setAttribute('role', 'alertdialog');
    panel.setAttribute('aria-modal', 'true');
    const warningCopy = document.createElement('p');
    warningCopy.className = 'mss-notice mss-notice-warn';
    warningCopy.setAttribute('aria-live', 'polite');
    const actions = document.createElement('div');
    actions.className = 'mss-actions';
    const confirmButton = document.createElement('button');
    confirmButton.type = 'button';
    confirmButton.className = 'mss-btn mss-btn-danger';
    confirmButton.textContent = tx('waitBeforeApply', { seconds: MSSManualWarningSeconds(warning.readyAt, Date.now()) });
    confirmButton.disabled = true;
    const cancelButton = document.createElement('button');
    cancelButton.type = 'button';
    cancelButton.className = 'mss-btn mss-btn-secondary';
    cancelButton.textContent = tx('cancel');
    actions.append(confirmButton, cancelButton);
    panel.append(warningCopy, actions);

    let fallbackOverlay = null;
    let modalOpen = true;
    let dismissed = false;
    let modalObserver = null;
    let popupWasConnected = false;
    const hideModal = () => {
      if (!modalOpen) return;
      modalOpen = false;
      if (usePopupModal) popup.hide();
      else fallbackOverlay?.remove();
    };
    if (usePopupModal) {
      popup.display({ title, content: panel, isLarge: false });
      popupWasConnected = panel.isConnected;
    } else {
      fallbackOverlay = document.createElement('div');
      fallbackOverlay.className = 'mss-modal-backdrop';
      const fallbackTitle = document.createElement('h2');
      fallbackTitle.className = 'mss-h2';
      fallbackTitle.textContent = title;
      panel.prepend(fallbackTitle);
      fallbackOverlay.appendChild(panel);
      document.body.appendChild(fallbackOverlay);
    }

    let countdownComplete = false;
    const timer = window.setInterval(() => {
      const seconds = MSSManualWarningSeconds(warning.readyAt, Date.now());
      warningCopy.textContent = tx('lowLimitWarning', {
        capMB: warning.capMB,
        defaultMB: warning.defaultMB,
        seconds
      });
      if (countdownComplete) return;
      if (seconds > 0) {
        confirmButton.textContent = tx('waitBeforeApply', { seconds });
        return;
      }
      countdownComplete = true;
      window.clearInterval(timer);
      confirmButton.disabled = false;
      confirmButton.textContent = tx('confirmLowLimit');
    }, 250);
    const closeWarning = modalAlreadyClosed => {
      if (dismissed) return;
      dismissed = true;
      window.clearInterval(timer);
      modalObserver?.disconnect();
      if (modalAlreadyClosed) modalOpen = false;
      else hideModal();
      setManualInputs(previous => ({ ...previous, [warning.key]: String(warning.previousValue ?? warning.defaultMB) }));
      setManualWarning(null);
    };
    const onConfirm = () => confirmManualLimit(warning, countdownComplete);
    const onCancel = () => closeWarning(false);
    const onBackdropClick = event => {
      if (event.target === fallbackOverlay) closeWarning(false);
    };
    confirmButton.addEventListener('click', onConfirm);
    cancelButton.addEventListener('click', onCancel);
    fallbackOverlay?.addEventListener('click', onBackdropClick);

    if (usePopupModal && typeof MutationObserver === 'function') {
      modalObserver = new MutationObserver(() => {
        if (panel.isConnected) {
          popupWasConnected = true;
        } else if (popupWasConnected) {
          closeWarning(true);
        }
      });
      modalObserver.observe(document.body, { childList: true, subtree: true });
    }
    return () => {
      window.clearInterval(timer);
      modalObserver?.disconnect();
      confirmButton.removeEventListener('click', onConfirm);
      cancelButton.removeEventListener('click', onCancel);
      fallbackOverlay?.removeEventListener('click', onBackdropClick);
      const wasConfirmed = confirmedManualWarning.current === warning.readyAt;
      if (wasConfirmed) confirmedManualWarning.current = null;
      else if (!dismissed) {
        setManualInputs(previous => ({ ...previous, [warning.key]: String(warning.previousValue ?? warning.defaultMB) }));
      }
      hideModal();
    };
  }, [manualWarning?.readyAt]);

  async function setField(key, value) {
    const previousConfig = config;
    const nextConfig = { ...config, [key]: value };
    setConfig(nextConfig);
    if (!await saveSettings(nextConfig)) setConfig(previousConfig);
  }
  async function saveSettings(nextConfig = config) {
    setSaving(true);
    setSettingsError('');
    try {
      const result = await MSSRequest('/api/config', { method: 'POST', body: nextConfig });
      const savedConfig = { enabled: typeof result.config?.enabled === 'boolean' ? result.config.enabled : nextConfig.enabled !== false, autoOptimize: result.config.autoOptimize, hiddenDelaySeconds: result.config.hiddenDelaySeconds, trimToCap: result.config.trimToCap !== false, optimizeWhenBackground: result.config.optimizeWhenBackground !== false, optimizeWhenMinimized: result.config.optimizeWhenMinimized !== false, optimizeWhenHidden: result.config.optimizeWhenHidden !== false };
      setConfig(savedConfig);
      setSavedAutoOptimize(savedConfig.autoOptimize);
      setError('');
      try { setStatus(await MSSRequest('/api/status')); } catch (_) { }
      return true;
    } catch (err) {
      setSettingsError(err?.message || tx('saveFailed'));
      return false;
    } finally { setSaving(false); }
  }

  async function applyManualLimit(process, capMB) {
    setBusy(true);
    setManualMessage(null);
    try {
      await MSSRequest('/api/action', { method: 'POST', body: { action: 'setManualCap', pid: process.pid, creation: process.creation, capMB } });
      const updatedStatus = await MSSRequest('/api/status');
      setStatus(updatedStatus);
      setManualInputs(previous => {
        const next = { ...previous };
        for (const target of updatedStatus.memory?.targets || []) {
          if (target.role === process.role) next[`${target.pid}:${target.creation}`] = String(capMB);
        }
        return next;
      });
      const background = ['minimized', 'hidden', 'background-visible'].includes(updatedStatus.spotify?.windowState);
      setManualMessage({ text: tx(background ? 'limitApplied' : 'limitSavedForBackground'), error: false });
    } catch (err) {
      setManualMessage({ text: err?.message || tx('helperApiError'), error: true });
    } finally { setBusy(false); }
  }
  function scheduleManualLimit(process, key, value) {
    const timers = manualApplyTimers.current;
    if (!manualInputOrigins.current.has(key)) {
      manualInputOrigins.current.set(key, String(manualInputs[key] ?? process.manualCapMB ?? process.defaultLimitMB));
    }
    if (timers.has(key)) window.clearTimeout(timers.get(key));
    const input = String(value);
    if (!input.trim()) return;
    const timer = window.setTimeout(() => {
      timers.delete(key);
      const capMB = Number(input);
      if (!Number.isInteger(capMB) || capMB < 8 || capMB > 512) {
        manualInputOrigins.current.delete(key);
        setManualMessage({ text: tx('limitRange'), error: true });
        return;
      }
      const previousValue = manualInputOrigins.current.get(key);
      manualInputOrigins.current.delete(key);
      void requestManualLimit(process, key, capMB, previousValue);
    }, 700);
    timers.set(key, timer);
  }
  function confirmManualLimit(warning, countdownComplete) {
    if (!countdownComplete) return;
    confirmedManualWarning.current = warning.readyAt;
    setManualWarning(null);
    void applyManualLimit({ pid: warning.pid, creation: warning.creation, role: warning.role }, warning.capMB);
  }
  async function requestManualLimit(process, key, capMB, previousValue) {
    const defaultMB = Number(process.defaultLimitMB);
    setManualMessage(null);
    if (!Number.isInteger(capMB) || capMB < 8 || capMB > 512) {
      setManualMessage({ text: tx('limitRange'), error: true });
      return;
    }
    if (capMB === process.manualCapMB) return;
    if (capMB === defaultMB) {
      if (process.manualCapMB != null) await clearManualLimit(process, key);
      return;
    }
    if (capMB < defaultMB) {
      const readyAt = Date.now() + 5000;
      setManualWarning({ key, pid: process.pid, creation: process.creation, role: process.role, capMB, defaultMB, previousValue: String(previousValue ?? process.manualCapMB ?? defaultMB), readyAt });
      return;
    }
    await applyManualLimit(process, capMB);
  }
  async function clearManualLimit(process, key) {
    setBusy(true);
    setManualMessage(null);
    try {
      const timer = manualApplyTimers.current.get(key);
      if (timer) window.clearTimeout(timer);
      manualApplyTimers.current.delete(key);
      manualInputOrigins.current.delete(key);
      await MSSRequest('/api/action', { method: 'POST', body: { action: 'clearManualCap', pid: process.pid, creation: process.creation } });
      const updatedStatus = await MSSRequest('/api/status');
      setStatus(updatedStatus);
      setManualInputs(previous => {
        const next = { ...previous };
        for (const target of updatedStatus.memory?.targets || []) {
          if (target.role === process.role) next[`${target.pid}:${target.creation}`] = String(target.defaultLimitMB);
        }
        return next;
      });
      setManualMessage({ text: tx('limitCleared'), error: false });
    } catch (err) {
      setManualMessage({ text: err?.message || tx('helperApiError'), error: true });
    } finally { setBusy(false); }
  }

  const spotify = status?.spotify || {};
  const memory = status?.memory || {};
  const telemetry = status?.telemetry || {};
  const helperRunning = Boolean(status?.helper?.running);
  const targets = Array.isArray(memory.targets) ? memory.targets : [];
  const events = Array.isArray(status?.recentEvents) ? status.recentEvents : [];
  const manualReady = config.enabled && !config.autoOptimize && savedAutoOptimize === false;

  const serverLabel = status?.helper ? `${status.helper.runtime || 'Server'} ${status.helper.version || ''}` : (MSS_PT ? 'Servidor' : 'Server');

  const cappedCount = targets.filter(target => target.capped).length;
  const maxMemoryMB = targets.reduce((max, target) => Math.max(max, memory.privateWorkingSet ? Number(target.privateWorkingSetMB || 0) : Number(target.workingSetMB || 0)), 0) || 1;

  return h('main', { className: 'mss-page' },
    h('header', { className: 'mss-topbar' },
      h('div', { className: 'mss-brand' },
        h('h1', { className: 'mss-title' }, tx('title')),
        h('div', { className: 'mss-subtitle' }, tx('brandSubtitle'))
      ),
      h('div', { className: 'mss-topbar-controls' },
        h('div', { className: `mss-pill ${error ? 'mss-pill-error' : (helperRunning ? 'mss-pill-connected' : 'mss-pill-waiting')}`, role: 'status' },
          h('span', { className: 'mss-dot', 'aria-hidden': 'true', style: { background: error ? '#e05252' : (helperRunning ? 'var(--spice-button,#1ed760)' : '#d6a03d') } }),
          error ? tx('unavailable') : (helperRunning ? tx('connected') : tx('waiting'))
        )
      )
    ),
    error ? h('section', { className: 'mss-notice mss-notice-error' }, h('strong', null, tx('helperMissingTitle')), h('p', { style: { margin: '7px 0 0' } }, `${tx('helperMissingBody')} ${error}`), h('div', { className: 'mss-actions' }, h('button', { className: 'mss-btn mss-btn-primary', onClick: MSSOpenGitHub }, tx('helperButton')), h('button', { className: 'mss-btn mss-btn-secondary', onClick: MSSOpenInstallGuide }, tx('installGuideButton')))) : null,
    h('section', { className: 'mss-summary' },
      h('div', { className: 'mss-summary-label' }, tx('ramCard')),
      h('div', { className: 'mss-summary-row' },
        h('span', { className: 'mss-summary-value', title: tx(memory.privateWorkingSet ? 'privateWorkingSetNote' : 'ramNote') }, MSSFormatMB(memory.spotifyTotalMB), h('small', null, 'MB')),
        h('span', { className: 'mss-summary-note' }, `${Number(memory.targetCount || 0).toLocaleString(MSS_LOCALE)} ${tx('processesCard').toLowerCase()} · ${cappedCount} ${tx('withCap')}`),
        h('span', { className: 'mss-summary-note', title: debug ? (spotify.windowStateReason || '') : undefined }, `${MSSWindowLabel(spotify.windowState || 'not-detected')}${debug && spotify.windowStateReason ? ` (${String(spotify.windowStateReason).split(' ')[0]})` : ''} · ${serverLabel}`)
      )
    ),
    h('section', { className: 'mss-toolbar' },
      h('div', { className: 'mss-toolbar-row' },
        h('div', { className: 'mss-seg', role: 'group', 'aria-label': tx('modeLabel') },
          h('button', { type: 'button', className: `mss-seg-btn${config.enabled && config.autoOptimize ? ' mss-on' : ''}`, 'aria-pressed': Boolean(config.enabled && config.autoOptimize), disabled: saving || !helperRunning || typeof status?.config?.enabled !== 'boolean', onClick: () => { void setMode('automatic'); } }, tx('segAuto')),
          h('button', { type: 'button', className: `mss-seg-btn${config.enabled && !config.autoOptimize ? ' mss-on' : ''}`, 'aria-pressed': Boolean(config.enabled && !config.autoOptimize), disabled: saving || !helperRunning || typeof status?.config?.enabled !== 'boolean', onClick: () => { void setMode('manual'); } }, tx('segManual')),
          h('button', { type: 'button', className: `mss-seg-btn${!config.enabled ? ' mss-on' : ''}`, 'aria-pressed': !config.enabled, disabled: saving || !helperRunning || typeof status?.config?.enabled !== 'boolean', onClick: () => { void setMode('disabled'); } }, tx('segDisabled'))
        ),
        h('div', { className: 'mss-when', role: 'group', 'aria-label': tx('whenLabel') },
          h('span', { className: 'mss-when-label' }, tx('whenLabel')),
          [['optimizeWhenBackground', 'whenBackground'], ['optimizeWhenMinimized', 'whenMinimized'], ['optimizeWhenHidden', 'whenHidden']].map(([key, label]) => h('label', { className: 'mss-check', key },
            h('input', { type: 'checkbox', checked: Boolean(config[key]), disabled: saving || !helperRunning || !config.enabled || (Boolean(config[key]) && activeTriggers() <= 1), onChange: event => setTrigger(key, event.target.checked) }),
            h('span', null, tx(label))
          ))
        )
      ),
      h('div', { className: 'mss-toolbar-row' },
        h('label', { className: 'mss-tool' }, h('span', null, tx('delayShort')),
          h('select', { className: 'mss-select', disabled: saving || !helperRunning || !config.enabled, value: String(config.hiddenDelaySeconds), onChange: event => setField('hiddenDelaySeconds', Number(event.target.value)) }, [1, 2, 3, 5, 8, 10, 15, 20, 30].map(n => h('option', { key: n, value: String(n) }, tx('seconds', { value: n }))))),
        h('label', { className: 'mss-tool' }, h('span', null, tx('releaseShort')),
          h('select', { className: 'mss-select', disabled: saving || !helperRunning || !config.enabled, value: config.trimToCap !== false ? 'ceiling' : 'aggressive', onChange: event => setField('trimToCap', event.target.value === 'ceiling') }, h('option', { value: 'ceiling' }, tx('releaseCeiling')), h('option', { value: 'aggressive' }, tx('releaseAggressive')))),
        h('span', { className: 'mss-tool-note' }, tx(saving ? 'saving' : 'autoSavedShort'))
      )
    ),
    !config.enabled ? h('div', { className: 'mss-notice mss-notice-warn', role: 'status' }, status?.cap?.cappedProcesses ? tx('pluginDisabledRestoring', { count: status.cap.cappedProcesses }) : tx('pluginDisabledNote')) : null,
    config.enabled && !config.autoOptimize ? h('div', { className: 'mss-notice' }, h('strong', null, tx('manualMode')), ' · ', tx('manualExplanation')) : null,
    h('section', { className: 'mss-table' },
      h('div', { className: 'mss-table-head' },
        h('span', null, tx('processes')),
        h('span', { className: 'mss-col-bar' }, tx('memoryLabel')),
        h('span', { className: 'mss-col-state' }, tx('stateLabel'))
      ),
      targets.length ? h('div', { className: 'mss-list' }, targets.map(process => {
        const key = `${process.pid}:${process.creation}`;
        const displayedMemoryMB = memory.privateWorkingSet ? process.privateWorkingSetMB : process.workingSetMB;
        const statusText = process.protectedAudio
          ? (config.autoOptimize ? tx('audioProtected') : (process.capped ? tx('manualActive', { value: process.limitMB }) : tx('manualCurrent', { value: process.defaultLimitMB })))
          : (process.capped ? tx(process.adaptiveStatus === 'sustained-high-cpu' ? 'reasonHigh' : (process.adaptiveStatus === 'low-cpu-and-memory-headroom' ? 'reasonLow' : (process.adaptiveStatus === 'baseline' ? 'reasonBaseline' : 'reasonHolding'))) : (process.included ? tx('willOptimize') : tx('notIncluded')));
        const meta = [debug && Number.isFinite(process.cpuPercent) ? tx('cpu', { value: Number(process.cpuPercent).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }) }) : null, debug && process.privateMB != null ? tx('privateMemory', { value: MSSFormatMB(process.privateMB) }) : null, debug ? statusText : null].filter(Boolean).join(' · ');
        const defaultMB = Number(process.defaultLimitMB || 36);
        const capText = process.capped && process.manual
          ? tx('manualActive', { value: process.limitMB ?? defaultMB })
          : process.manualCapMB != null
            ? tx('manualConfigured', { value: process.manualCapMB })
            : tx('manualCurrent', { value: defaultMB });
        const memoryMB = Number(displayedMemoryMB || 0);
        const ceilingMB = process.capped ? Number(process.limitMB ?? process.manualCapMB ?? defaultMB) : null;
        const ceilingText = ceilingMB != null ? `${tx('capLabel')} ${ceilingMB} MB` : tx('noCapLabel');
        const over = ceilingMB != null && memoryMB > ceilingMB;
        const pct = Math.min(100, Math.max(3, (memoryMB / maxMemoryMB) * 100));
        const peekText = displayedMemoryMB == null ? '—' : `${MSSFormatMB(displayedMemoryMB)} MB · ${ceilingText}`;
        return h('div', { className: 'mss-process', key },
          h('div', { className: 'mss-process-main' },
            h('div', { className: 'mss-process-name' }, MSSRoleLabel(process.role), h('span', { className: 'mss-role' }, MSS_ROLE_NAMES[process.role] || process.role)),
            meta ? h('div', { className: 'mss-process-meta' }, meta) : null
          ),
          h('div', { className: 'mss-bar-cell', tabIndex: 0, 'aria-label': peekText, onMouseMove: MSSPositionPeek, onMouseEnter: MSSPositionPeek },
            h('div', { className: 'mss-bar', 'aria-hidden': 'true' }, h('i', { className: over ? 'mss-fill-over' : '', style: { width: `${pct.toFixed(1)}%` } })),
            h('span', { className: 'mss-peek' }, peekText)
          ),
          h('span', { className: `mss-tag${over ? ' mss-tag-over' : (process.capped ? ' mss-tag-on' : '')}` }, process.manual ? (MSS_PT ? 'Manual' : 'Manual') : (process.protectedAudio ? (MSS_PT ? 'Áudio' : 'Audio') : (process.capped ? (MSS_PT ? 'Adaptativo' : 'Adaptive') : process.role))),
          manualReady ? h('div', { className: 'mss-manual-controls' },
            h('label', { className: 'mss-small', htmlFor: `mss-limit-${key}` }, tx('manualLimit')),
            h('input', { id: `mss-limit-${key}`, className: 'mss-input', type: 'number', min: 8, max: 512, step: 1, value: manualInputs[key] ?? String(process.manualCapMB ?? defaultMB), disabled: busy || !config.enabled, onChange: event => { const value = event.target.value; setManualInputs(previous => ({ ...previous, [key]: value })); scheduleManualLimit(process, key, value); } }),
            h('span', { className: 'mss-small' }, capText)
          ) : null
        );
      })) : h('div', { className: 'mss-empty' }, tx('noneDetected')),
      targets.length ? h('div', { className: 'mss-small mss-hover-hint' }, tx('hoverHint')) : null,
      config.enabled && !config.autoOptimize && !manualReady ? h('div', { className: 'mss-notice mss-notice-warn' }, tx('saveBeforeManual')) : null,
      manualMessage ? h('div', { className: `mss-notice${manualMessage.error ? ' mss-notice-error' : ''}` }, manualMessage.text) : null
    ),
    settingsError ? h('div', { className: 'mss-notice mss-notice-error', role: 'alert' }, settingsError) : null,
    h('section', { className: 'mss-diag' },
      h('div', { className: 'mss-diag-head' },
        h('div', null,
          h('div', { className: 'mss-diag-title' }, tx('debugMode')),
          h('div', { className: 'mss-small' }, tx('debugHint'))
        ),
        MSSDebugToggle(debug, applyDebug)
      ),
      debug ? h('div', { className: 'mss-diag-body' },
        h('div', { className: 'mss-diag-grid' },
          h('div', null, h('span', null, tx('pageFaults')), h('strong', null, telemetry.pageFaultDelta == null ? '—' : Number(telemetry.pageFaultDelta).toLocaleString(MSS_LOCALE))),
          h('div', null, h('span', null, tx('pageFaultRate')), h('strong', null, telemetry.pageFaultsPerSecond == null ? '—' : Number(telemetry.pageFaultsPerSecond).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }))),
          h('div', null, h('span', null, tx('adaptiveChanges')), h('strong', null, Number(telemetry.adaptiveAdjustments || 0).toLocaleString(MSS_LOCALE))),
          h('div', null, h('span', null, tx('restoreDuration')), h('strong', null, telemetry.lastRestoreDurationMs == null ? '—' : `${Number(telemetry.lastRestoreDurationMs).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 })} ms`))
        ),
        telemetry.lastAdaptiveAdjustmentAt ? h('div', { className: 'mss-small' }, tx('lastAdjustment', { time: MSSDate(telemetry.lastAdaptiveAdjustmentAt) })) : null,
        status?.lastAction ? h('div', { className: 'mss-notice' }, tx('lastAction', { count: status.lastAction.applied || 0 })) : null,
        h('div', { className: 'mss-small', style: { marginTop: '4px' } }, tx('events')),
        events.length ? h('div', { className: 'mss-events' }, events.map((event, i) => h('div', { className: 'mss-event', key: `${event.time}-${i}` }, h('span', { className: 'mss-event-time' }, MSSDate(event.time)), h('span', { className: 'mss-event-msg' }, MSSLocalizeEvent(event.message))))) : h('div', { className: 'mss-empty' }, tx('noEvents'))
      ) : null,
      status?.lastError ? h('div', { className: 'mss-notice mss-notice-error' }, MSSLocalizeEvent(status.lastError)) : null
    ),
    h('details', { className: 'mss-fineprint' }, h('summary', null, tx('safetySummary')), h('p', null, tx('intro')), h('p', null, tx('safety'))),
    h('footer', { className: 'mss-footer' },
      h('span', null, 'Windows 10/11 x64'),
      h('span', null, 'GPL-3.0'),
      h('span', null, MSS_PT ? 'nada sai do PC' : 'nothing leaves the PC'),
      h('span', null, `${tx('footer')} · ${MSS_PT ? 'Idioma automático: português' : 'Auto language: English'}`)
    )
  );
}

let mssProbeLogged = false;
function render() {
  if (typeof h !== 'function' || !window.MSS) {
    console.error('[Memory Sentry] App bundle is incomplete. Run "spicetify apply" and reopen the app.');
    return null;
  }
  if (!mssProbeLogged) {
    mssProbeLogged = true;
    try { console.info('[Memory Sentry] component adapters', window.MSS.adapters.report(), 'available:', window.MSS.adapters.available); } catch (_) { }
  }
  return h(MSSApp, {});
}