// Memory Sentry Custom App. Source and default locale are English; Portuguese is selected automatically.
// The installer replaces __MSS_TOKEN__ with an internal per-install access token.

const MSS_TOKEN = '__MSS_TOKEN__';


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
  const [config, setConfig] = R.useState({ enabled: true, autoOptimize: true, hiddenDelaySeconds: 2, trimToCap: true });
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
  async function togglePluginEnabled(enabled) {
    if (saving || !helperRunning || typeof status?.config?.enabled !== 'boolean') return;
    const previousConfig = config;
    setManualWarning(null);
    setManualMessage(null);
    const saved = await saveSettings({ ...config, enabled: Boolean(enabled) });
    if (!saved) {
      setConfig(previousConfig);
      return;
    }
    showScreenFlash(Boolean(enabled));
  }

  R.useEffect(() => {
    let alive = true;
    async function refresh() {
      try {
        const data = await MSSRequest('/api/status');
        if (!alive) return;
        setStatus(data); setError('');
        if (!settingsLoaded && data.config) {
          const loadedConfig = { enabled: data.config.enabled !== false, autoOptimize: data.config.autoOptimize !== false, hiddenDelaySeconds: data.config.hiddenDelaySeconds || 2, trimToCap: data.config.trimToCap !== false };
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
      const savedConfig = { enabled: typeof result.config?.enabled === 'boolean' ? result.config.enabled : nextConfig.enabled !== false, autoOptimize: result.config.autoOptimize, hiddenDelaySeconds: result.config.hiddenDelaySeconds, trimToCap: result.config.trimToCap !== false };
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
  const card = (label, value, unit, note) => h('section', { className: 'mss-card' }, h('div', { className: 'mss-card-label' }, label), h('div', { className: 'mss-number' }, value, unit ? h('span', { className: 'mss-unit' }, unit) : null), debug && note ? h('div', { className: 'mss-note' }, note) : null);

  return h('main', { className: 'mss-page' },
    h('header', { className: 'mss-hero' },
      h('div', { className: 'mss-hero-copy' },
        h('div', { className: 'mss-kicker' }, tx('kicker')),
        h('h1', { className: 'mss-title' }, tx('title')),
        h('div', { className: 'mss-copy' }, tx('intro')),
        h('div', { className: 'mss-small mss-window-state' }, `${MSSWindowLabel(spotify.windowState || 'not-detected')} · ${MSS_PT ? 'Idioma automático: português' : 'Auto language: English'}`)
      ),
      h('aside', { className: 'mss-hero-controls' },
        h('div', { className: `mss-pill ${error ? 'mss-pill-error' : (helperRunning ? 'mss-pill-connected' : 'mss-pill-waiting')}` }, h('span', { className: 'mss-dot', style: { background: error ? '#e05252' : (helperRunning ? 'var(--spice-button,#1ed760)' : '#d6a03d') } }), error ? tx('unavailable') : (helperRunning ? tx('connected') : tx('waiting'))),
        h('label', { className: `mss-power ${config.enabled ? 'mss-power-on' : 'mss-power-off'}${saving || !helperRunning || typeof status?.config?.enabled !== 'boolean' ? ' mss-power-disabled' : ''}`, title: tx('pluginToggleHint') },
          h('input', { type: 'checkbox', role: 'switch', 'aria-label': tx('pluginToggleHint'), checked: Boolean(config.enabled), disabled: saving || !helperRunning || typeof status?.config?.enabled !== 'boolean', onChange: event => { void togglePluginEnabled(event.target.checked); } }),
          h('span', { className: 'mss-power-copy' },
            h('span', { className: 'mss-power-title' }, tx(config.enabled ? 'pluginEnabled' : 'pluginDisabled')),
            h('span', { className: 'mss-power-hint' }, tx('pluginToggleHint'))
          ),
          h('span', { className: 'mss-switch-track', 'aria-hidden': 'true' }, h('span', { className: 'mss-switch-thumb' }))
        ),
        h('div', { className: 'mss-debug-row', title: tx('debugHint') },
          h('span', { className: 'mss-debug-label' }, tx('debugMode')),
          MSSDebugToggle(debug, applyDebug)
        )
      )
    ),
    error ? h('section', { className: 'mss-notice mss-notice-error' }, h('strong', null, tx('helperMissingTitle')), h('p', { style: { margin: '7px 0 0' } }, `${tx('helperMissingBody')} ${error}`), h('div', { className: 'mss-actions' }, h('button', { className: 'mss-btn mss-btn-primary', onClick: MSSOpenGitHub }, tx('helperButton')), h('button', { className: 'mss-btn mss-btn-secondary', onClick: MSSOpenInstallGuide }, tx('installGuideButton')))) : null,
    !config.enabled ? h('div', { className: 'mss-notice mss-notice-warn', role: 'status' }, status?.cap?.cappedProcesses ? tx('pluginDisabledRestoring', { count: status.cap.cappedProcesses }) : tx('pluginDisabledNote')) : null,
    h('div', { className: 'mss-notice mss-notice-warn mss-safety-note' }, tx('safety')),
    h('div', { className: 'mss-grid' },
      card(tx(memory.privateWorkingSet ? 'privateWorkingSetRam' : 'spotifyRam'), MSSFormatMB(memory.spotifyTotalMB), 'MB', tx(memory.privateWorkingSet ? 'privateWorkingSetNote' : 'ramNote')),
      card(tx('spotifyProcesses'), Number(memory.targetCount || 0).toLocaleString(MSS_LOCALE), ''),
      debug ? card(tx('pageFaults'), telemetry.pageFaultDelta == null ? '—' : Number(telemetry.pageFaultDelta).toLocaleString(MSS_LOCALE), '', tx('pageFaultNote')) : null,
      debug ? card(tx('pageFaultRate'), telemetry.pageFaultsPerSecond == null ? '—' : Number(telemetry.pageFaultsPerSecond).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }), '/s', tx('pageFaultRateNote')) : null,
      debug ? card(tx('adaptiveChanges'), Number(telemetry.adaptiveAdjustments || 0).toLocaleString(MSS_LOCALE), '', tx('adaptiveNote')) : null,
      debug ? card(tx('restoreDuration'), telemetry.lastRestoreDurationMs == null ? '—' : Number(telemetry.lastRestoreDurationMs).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1 }), telemetry.lastRestoreDurationMs == null ? '' : 'ms', tx('restoreNote')) : null
    ),
    h('section', { className: 'mss-section' },
      h('div', { className: 'mss-section-head' }, h('h2', { className: 'mss-h2' }, tx('processes')), h('span', { className: 'mss-small' }, tx('updated', { time: MSSDate(memory.sampledAt) }))),
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
        return h('div', { className: 'mss-process', key },
          h('div', null, h('div', { className: 'mss-process-name' }, `${MSSRoleLabel(process.role)} · PID ${process.pid}`), meta ? h('div', { className: 'mss-process-meta' }, meta) : null),
          h('span', { className: 'mss-tag' }, process.manual ? (MSS_PT ? 'Manual' : 'Manual') : (process.protectedAudio ? (MSS_PT ? 'Áudio' : 'Audio') : (process.capped ? (MSS_PT ? 'Adaptativo' : 'Adaptive') : process.role))),
          h('div', { className: 'mss-memory' }, displayedMemoryMB == null ? '—' : `${MSSFormatMB(displayedMemoryMB)} MB${process.capped ? ` / ${process.limitMB} MB` : ''}`),
          manualReady ? h('div', { className: 'mss-manual-controls' },
            h('label', { className: 'mss-small', htmlFor: `mss-limit-${key}` }, tx('manualLimit')),
            h('input', { id: `mss-limit-${key}`, className: 'mss-input', type: 'number', min: 8, max: 512, step: 1, value: manualInputs[key] ?? String(process.manualCapMB ?? defaultMB), disabled: busy || !config.enabled, onChange: event => { const value = event.target.value; setManualInputs(previous => ({ ...previous, [key]: value })); scheduleManualLimit(process, key, value); } }),
            h('span', { className: 'mss-small' }, capText)
          ) : null
        );
      })) : h('div', { className: 'mss-empty' }, tx('noneDetected')),
      config.enabled && !config.autoOptimize && !manualReady ? h('div', { className: 'mss-notice mss-notice-warn' }, tx('saveBeforeManual')) : null,
      manualMessage ? h('div', { className: `mss-notice${manualMessage.error ? ' mss-notice-error' : ''}` }, manualMessage.text) : null,
      debug && telemetry.lastAdaptiveAdjustmentAt ? h('div', { className: 'mss-actions' }, h('span', { className: 'mss-small' }, tx('lastAdjustment', { time: MSSDate(telemetry.lastAdaptiveAdjustmentAt) }))) : null,
      debug && status?.lastAction ? h('div', { className: 'mss-notice' }, tx('lastAction', { count: status.lastAction.applied || 0 })) : null
    ),
    h('section', { className: 'mss-section' },
      h('div', { className: 'mss-section-head' }, h('h2', { className: 'mss-h2' }, tx('behavior')), h('span', { className: 'mss-small' }, debug && helperRunning ? `${status?.helper?.runtime || 'Server'} ${status?.helper?.version || ''}` : (MSS_PT ? 'Servidor' : 'Server'))),
      h('label', { className: 'mss-check', style: { marginBottom: '18px' } }, h('input', { type: 'checkbox', role: 'switch', disabled: saving || !helperRunning || !config.enabled, checked: Boolean(config.autoOptimize), onChange: event => { setManualWarning(null); setField('autoOptimize', event.target.checked); } }), h('span', null, h('strong', null, tx('active')), h('br'), h('span', { className: 'mss-small' }, tx('autoExplanation')))),
      !config.autoOptimize ? h('div', { className: 'mss-notice' }, h('strong', null, tx('manualMode')), ' · ', tx('manualExplanation')) : null,
      h('div', { className: 'mss-setting-group-head' }, h('h3', { className: 'mss-setting-group-title' }, tx('backgroundSettings')), h('span', { className: 'mss-small' }, tx('backgroundSettingsHint'))),
      h('div', { className: 'mss-settings' }, h('label', { className: 'mss-setting' }, h('span', { className: 'mss-setting-label' }, tx('delay')), h('select', { className: 'mss-select', disabled: saving || !helperRunning || !config.enabled, value: String(config.hiddenDelaySeconds), onChange: event => setField('hiddenDelaySeconds', Number(event.target.value)) }, [1, 2, 3, 5, 8, 10, 15, 20, 30].map(n => h('option', { key: n, value: String(n) }, tx('seconds', { value: n })))), h('span', { className: 'mss-small' }, tx('delayExplanation'))),
        h('label', { className: 'mss-setting' }, h('span', { className: 'mss-setting-label' }, tx('releaseMode')), h('select', { className: 'mss-select', disabled: saving || !helperRunning || !config.enabled, value: config.trimToCap !== false ? 'ceiling' : 'aggressive', onChange: event => setField('trimToCap', event.target.value === 'ceiling') }, h('option', { value: 'ceiling' }, tx('releaseCeiling')), h('option', { value: 'aggressive' }, tx('releaseAggressive'))), h('span', { className: 'mss-small' }, tx('releaseModeExplanation')))),
      settingsError ? h('div', { className: 'mss-notice mss-notice-error', role: 'alert' }, settingsError) : null,
      h('div', { className: 'mss-actions' }, h('span', { className: 'mss-small' }, tx(saving ? 'saving' : 'autoSaved')))
    ),
    debug ? h('section', { className: 'mss-section' }, h('div', { className: 'mss-section-head' }, h('h2', { className: 'mss-h2' }, tx('events')), h('span', { className: 'mss-small' }, status?.helper ? `${status.helper.runtime || 'Server'} ${status.helper.version || ''}` : (MSS_PT ? 'Servidor' : 'Server'))),
      events.length ? h('div', { className: 'mss-events' }, events.map((event, i) => h('div', { className: 'mss-event', key: `${event.time}-${i}` }, h('span', { className: 'mss-event-time' }, MSSDate(event.time)), h('span', { className: 'mss-event-msg' }, MSSLocalizeEvent(event.message))))) : h('div', { className: 'mss-empty' }, tx('noEvents')),
      status?.lastError ? h('div', { className: 'mss-notice mss-notice-error' }, MSSLocalizeEvent(status.lastError)) : null
    ) : (status?.lastError ? h('div', { className: 'mss-notice mss-notice-error' }, MSSLocalizeEvent(status.lastError)) : null),

    h('footer', { className: 'mss-footer' }, tx('footer'))
  );
}

function render() { return h(MSSApp, {}); }
