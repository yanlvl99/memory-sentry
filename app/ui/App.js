// MSSApp: state, effects and imperative handlers, then compose the four screen cards
// (TopBar, Hero, Settings, Diagnostics) plus the fine print. Kept imperative because the
// low-limit warning modal uses direct DOM (Spicetify's PopupModal does not accept a vnode)
// and the polling loop needs a stable effect closure — both stay here, in one file.
// A cheap stability signature for the polled /api/status payload: everything the dashboard renders,
// minus the volatile per-poll timestamp (memory.sampledAt) that changes on every request even when
// nothing else did. When two consecutive polls share this signature the visible data is identical, so
// the caller skips setState and an idle tab avoids rebuilding the whole vnode tree for no reason.
function mssStatusSignature(data) {
  if (!data || typeof data !== 'object') return '';
  try {
    let memory = data.memory;
    if (memory && typeof memory === 'object') {
      memory = { ...memory };
      delete memory.sampledAt;
    }
    return JSON.stringify({ ...data, memory });
  } catch (_) {
    return null;
  }
}

function MSSApp() {
  const R = MSSReact;
  const [status, setStatus] = R.useState(null);
  const [error, setError] = R.useState('');
  const [settingsError, setSettingsError] = R.useState('');
  const [busy, setBusy] = R.useState(false);
  const [saving, setSaving] = R.useState(false);
  const [settingsLoaded, setSettingsLoaded] = R.useState(false);
  const [debug, setDebug] = R.useState(() => { try { return window.localStorage.getItem('mss.debugMode') === '1'; } catch (_) { return false; } });
  const [config, setConfig] = R.useState({ enabled: true, autoOptimize: true, customMode: false, hiddenDelaySeconds: 2, refreshIntervalMs: 700, trimToCap: true, optimizeWhenBackground: true, optimizeWhenMinimized: true, optimizeWhenHidden: true });
  const [manualInputs, setManualInputs] = R.useState({});
  const [expandedRole, setExpandedRole] = R.useState(null);
  const [hoveredKey, setHoveredKey] = R.useState(null);
  const [manualWarning, setManualWarning] = R.useState(null);
  const manualApplyTimers = R.useRef(new Map());
  const manualInputOrigins = R.useRef(new Map());
  const confirmedManualWarning = R.useRef(null);
  const previousWindowState = R.useRef(null);
  const toastNode = R.useRef(null);
  const toastTimer = R.useRef(null);
  const [manualMessage, setManualMessage] = R.useState(null);
  const [reloadNonce, setReloadNonce] = R.useState(0);
  const [reconnecting, setReconnecting] = R.useState(false);
  // Connection health is a three-state signal, not a boolean (presence/heartbeat guidance): 'online'
  // (last poll ok), 'lost' (we connected before but the last polls failed - transient drop) and
  // 'missing' (never reached the server this session - it is not installed/running). A single failed
  // poll must NOT flip the whole dashboard to the setup screen, so we only escalate after tracking
  // whether we ever connected. everConnected/failCount are refs so the backoff loop reads fresh values
  // without re-subscribing the effect.
  const [connection, setConnection] = R.useState('unknown');
  const everConnected = R.useRef(false);
  const failCount = R.useRef(0);
  // Signature of the last status we rendered; a poll whose signature matches leaves the state
  // untouched so an idle dashboard does not re-render. Reset on (re)connect transitions.
  const lastStatusSig = R.useRef(null);

  function applyDebug(next) {
    const enabled = Boolean(next);
    setDebug(enabled);
    try { window.localStorage.setItem('mss.debugMode', enabled ? '1' : '0'); } catch (_) { }
  }
  function retryNow() {
    setReconnecting(true);
    setReloadNonce(n => n + 1);
  }
  function showActivationToast(enabled) {
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastNode.current?.remove();
    const toast = document.createElement('div');
    toast.className = `mss-toast ${enabled ? 'mss-toast-on' : 'mss-toast-off'}`;
    toast.setAttribute('role', 'status');
    toast.setAttribute('aria-live', 'polite');
    const dot = document.createElement('span');
    dot.className = 'mss-toast-dot';
    dot.setAttribute('aria-hidden', 'true');
    const label = document.createElement('span');
    label.textContent = tx(enabled ? 'pluginActivatedToast' : 'pluginDisabledToast');
    toast.append(dot, label);
    document.body.appendChild(toast);
    toastNode.current = toast;
    window.requestAnimationFrame(() => toast.classList.add('mss-toast-show'));
    toastTimer.current = window.setTimeout(() => {
      toast.classList.remove('mss-toast-show');
      toast.classList.add('mss-toast-hide');
      window.setTimeout(() => {
        toast.remove();
        if (toastNode.current === toast) toastNode.current = null;
      }, 240);
    }, 2200);
  }
  async function setMode(mode) {
    if (saving || !helperRunning || typeof status?.config?.enabled !== 'boolean') return;
    setManualWarning(null);
    setManualMessage(null);
    const nextConfig = mode === 'disabled'
      ? { ...config, enabled: false }
      : { ...config, enabled: true, autoOptimize: true, customMode: true };
    const saved = await saveSettings(nextConfig);
    if (!saved) return;
    if (config.enabled !== nextConfig.enabled) showActivationToast(nextConfig.enabled);
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
        // Skip the re-render entirely when the payload is materially unchanged; error/connection
        // state still updates so a recovery is reflected even if the numbers did not move.
        const sig = mssStatusSignature(data);
        if (sig === null || sig !== lastStatusSig.current) {
          lastStatusSig.current = sig;
          setStatus(data);
        }
        setError(''); setReconnecting(false);
        everConnected.current = true; failCount.current = 0; setConnection('online');
        if (!settingsLoaded && data.config) {
          const loadedConfig = { enabled: data.config.enabled !== false, autoOptimize: data.config.autoOptimize !== false, customMode: data.config.customMode === true, hiddenDelaySeconds: data.config.hiddenDelaySeconds || 2, refreshIntervalMs: data.config.refreshIntervalMs || 700, trimToCap: data.config.trimToCap !== false, optimizeWhenBackground: data.config.optimizeWhenBackground !== false, optimizeWhenMinimized: data.config.optimizeWhenMinimized !== false, optimizeWhenHidden: data.config.optimizeWhenHidden !== false };
          setConfig(loadedConfig);
          setSettingsLoaded(true);
        }
      } catch (err) {
        if (alive) {
          const detail = err && err.message ? String(err.message) : String(err || 'Unknown error');
          setError(tx('statusError') + ' [' + detail + ']');
          setReconnecting(false);
          failCount.current += 1;
          // Never reached the server this session -> 'missing' (setup screen), but only after a few
          // consecutive failures so a slow logon-startup or a single blip does not flash the "not
          // installed" screen. Reached it before and now failing -> 'lost' (keep the dashboard with the
          // last data + a reconnect banner). 'unknown' means still loading, not offline.
          if (everConnected.current) setConnection('lost');
          else setConnection(failCount.current >= 3 ? 'missing' : 'unknown');
        }
      }
    }
    const isVisible = () => document.visibilityState !== 'hidden';
    // The visible poll cadence is the SAME setting the server uses for its watched memory refresh
    // (refresh_interval_ms, clamped identically to 250-10000 on the server). One setting drives both.
    const visibleMs = Math.max(250, Math.min(10000, Number(config.refreshIntervalMs) || 700));
    let timer = 0;
    let inFlight = false;
    async function tick() {
      // Never overlap: a visibility change or a slow fetch could otherwise start a second concurrent
      // tick, and both would schedule a timer, doubling the poll rate. Skip if one is already running.
      if (inFlight) return;
      inFlight = true;
      if (timer) { window.clearTimeout(timer); timer = 0; }
      try {
        await refresh();
      } finally {
        inFlight = false;
      }
      if (!alive) return;
      // Normal cadence while healthy; exponential backoff (capped at 10s) after failures so a down
      // server is not hammered. A self-rescheduling timeout (not setInterval) lets the delay change
      // per attempt, which a fixed interval cannot.
      const base = isVisible() ? visibleMs : 4000;
      const delay = failCount.current > 0 ? Math.min(10000, base * 2 ** Math.min(failCount.current, 4)) : base;
      timer = window.setTimeout(tick, delay);
    }
    function onVisibility() {
      // Coming back into view: poll immediately (skip any backoff wait) and resume the fast cadence.
      if (isVisible()) { if (timer) window.clearTimeout(timer); void tick(); }
    }
    void tick();
    document.addEventListener('visibilitychange', onVisibility);
    return () => { alive = false; if (timer) window.clearTimeout(timer); document.removeEventListener('visibilitychange', onVisibility); };
  }, [settingsLoaded, reloadNonce, config.refreshIntervalMs]);

  R.useEffect(() => () => {
    for (const timer of manualApplyTimers.current.values()) window.clearTimeout(timer);
    manualApplyTimers.current.clear();
    manualInputOrigins.current.clear();
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastNode.current?.remove();
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
    confirmButton.textContent = tx('waitBeforeApply', { seconds: MSSManualWarningSeconds(warning.readyAt, performance.now()) });
    confirmButton.disabled = true;
    const cancelButton = document.createElement('button');
    cancelButton.type = 'button';
    cancelButton.className = 'mss-btn mss-btn-secondary';
    cancelButton.textContent = tx('cancel');
    actions.append(confirmButton, cancelButton);
    const ackRow = document.createElement('label');
    ackRow.className = 'mss-ack';
    ackRow.setAttribute('style', 'display:flex;gap:8px;align-items:center;margin:8px 0');
    const ackBox = document.createElement('input');
    ackBox.type = 'checkbox';
    ackBox.className = 'mss-ack-box';
    const ackText = document.createElement('span');
    ackText.textContent = tx('ackLowLimitRisk');
    ackRow.append(ackBox, ackText);
    panel.append(warningCopy, ackRow, actions);

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
      const seconds = MSSManualWarningSeconds(warning.readyAt, performance.now());
      warningCopy.textContent = tx('lowLimitWarning', {
        capMB: warning.capMB,
        defaultMB: warning.defaultMB
      }) + (seconds > 0
        ? ' ' + tx('lowLimitWait', { countdown: tx(seconds === 1 ? 'second' : 'seconds', { value: seconds }) })
        : ' ' + tx('lowLimitReady'));
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
    const onConfirm = () => {
      if (ackBox.checked) { try { window.localStorage.setItem('mss.skipLowLimit', '1'); } catch (_) { } }
      confirmManualLimit(warning, countdownComplete);
    };
    const onCancel = () => closeWarning(false);
    const onBackdropClick = event => {
      if (event.target === fallbackOverlay) closeWarning(false);
    };
    confirmButton.addEventListener('click', onConfirm);
    cancelButton.addEventListener('click', onCancel);
    fallbackOverlay?.addEventListener('click', onBackdropClick);
    const onDocumentClick = event => { if (!panel.contains(event.target)) closeWarning(false); };
    const onDocumentKeydown = event => { if (event.key === 'Escape') closeWarning(false); };
    document.addEventListener('click', onDocumentClick, true);
    document.addEventListener('keydown', onDocumentKeydown);

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
      document.removeEventListener('click', onDocumentClick, true);
      document.removeEventListener('keydown', onDocumentKeydown);
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
      const body = { ...nextConfig };
      const result = await MSSRequest('/api/config', { method: 'POST', body });
      const savedConfig = { enabled: typeof result.config?.enabled === 'boolean' ? result.config.enabled : nextConfig.enabled !== false, autoOptimize: result.config.autoOptimize, customMode: result.config.customMode === true, hiddenDelaySeconds: result.config.hiddenDelaySeconds, refreshIntervalMs: result.config.refreshIntervalMs, trimToCap: result.config.trimToCap !== false, optimizeWhenBackground: result.config.optimizeWhenBackground !== false, optimizeWhenMinimized: result.config.optimizeWhenMinimized !== false, optimizeWhenHidden: result.config.optimizeWhenHidden !== false };
      setConfig(savedConfig);
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
      await MSSRequest('/api/action', { method: 'POST', body: { action: 'setManualCap', identity: process.identity, capMB } });
      const updatedStatus = await MSSRequest('/api/status');
      setStatus(updatedStatus);
      setManualInputs(previous => ({ ...previous, [process.identity || `${process.pid}:${process.creation}`]: String(capMB) }));
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
    void applyManualLimit({ identity: warning.key, role: warning.role }, warning.capMB);
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
    if (capMB < defaultMB && window.localStorage.getItem('mss.skipLowLimit') !== '1') {
      const readyAt = performance.now() + 5000;
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
      await MSSRequest('/api/action', { method: 'POST', body: { action: 'clearManualCap', identity: process.identity || key } });
      const updatedStatus = await MSSRequest('/api/status');
      setStatus(updatedStatus);
      setManualInputs(previous => ({ ...previous, [key]: String(process.defaultLimitMB) }));
      setManualMessage({ text: tx('limitCleared'), error: false });
    } catch (err) {
      setManualMessage({ text: err?.message || tx('helperApiError'), error: true });
    } finally { setBusy(false); }
  }

  async function pinCurrentCeiling(process) {
    if (process.manualCapMB != null) return;
    const key = process.identity || `${process.pid}:${process.creation}`;
    const capMB = Number(manualInputs[key] ?? process.limitMB ?? process.defaultLimitMB ?? 36);
    await applyManualLimit(process, capMB);
  }

  const spotify = status?.spotify || {};
  const memory = status?.memory || {};
  const telemetry = status?.telemetry || {};
  const helperRunning = Boolean(status?.helper?.running);
  const targets = Array.isArray(memory.targets) ? memory.targets : [];
  const dispMB = t => Number((memory.privateWorkingSet ? t.privateWorkingSetMB : t.workingSetMB) || 0);
  const listMaxMB = Math.max(1, ...targets.map(dispMB));
  const totalMB = targets.reduce((sum, t) => sum + dispMB(t), 0);
  const sortedTargets = [...targets].sort((a, b) => String(a.role).localeCompare(String(b.role)) || dispMB(b) - dispMB(a));
  const events = Array.isArray(status?.recentEvents) ? status.recentEvents : [];
  // Only Custom (enabled) and Disabled remain, so per-process caps are always editable when enabled.
  const capsEditable = config.enabled === true;
  const serverLabel = status?.helper ? `${status.helper.runtime || 'Server'} ${status.helper.version || ''}` : (MSS_PT ? 'Servidor' : 'Server');
  const cappedCount = targets.filter(target => target.capped).length;
  // Only a server we have NEVER reached this session shows the full setup screen. A server that was
  // connected and then dropped keeps the dashboard (last known data) with a non-blocking banner, so a
  // momentary hiccup does not throw away the view or claim the app is not installed.
  const serverMissing = connection === 'missing';
  const connectionLost = connection === 'lost' || (status != null && !helperRunning);

  if (serverMissing) {
    return h('main', { className: 'mss-page' },
      h(TopBar, { error, helperRunning, connection }),
      h(MSSServerOffline, { error, reconnecting, onRetry: retryNow })
    );
  }

  return h('main', { className: 'mss-page' },
    h(TopBar, { error, helperRunning, connection }),
    connectionLost ? h('div', { className: 'mss-notice mss-notice-warn', role: 'status' },
      h('span', { className: 'mss-spinner', 'aria-hidden': 'true' }), tx('connectionLost')) : null,
    h(Hero, {
      status, config, memory, spotify, debug, saving, busy, helperRunning,
      targets, sortedTargets, dispMB, totalMB, listMaxMB, cappedCount, serverLabel,
      expandedRole, setExpandedRole, hoveredKey, setHoveredKey,
      manualInputs, manualMessage, manualReady: capsEditable,
      setMode, onSchedule: scheduleManualLimit, onClear: clearManualLimit, onPin: pinCurrentCeiling, onCopy: mssCopy
    }),
    h(Settings, {
      config, saving, helperRunning, status,
      onField: setField, onTrigger: setTrigger, activeTriggers
    }),
    settingsError ? h('div', { className: 'mss-notice mss-notice-error', role: 'alert' }, settingsError) : null,
    h(Diagnostics, { status, telemetry, events, debug, onDebug: applyDebug }),
    h(Fineprint),
    h(Footer)
  );
}

// Dedicated full-screen empty state shown when the local Memory Sentry server cannot be reached.
// Explains why a separate per-user process is required, surfaces the raw diagnostic in a monospace
// block, offers a numbered recovery path, and probes the connection again on demand.
function MSSServerOffline(props) {
  const { error, reconnecting, onRetry } = props;
  return h('section', { className: 'mss-offline', role: 'alert' },
    h('div', { className: 'mss-offline-icon' }, MSSIcon('server')),
    h('h2', { className: 'mss-offline-title' }, tx('helperMissingTitle')),
    h('p', { className: 'mss-offline-lead' }, tx('serverOfflineLead')),
    h('p', { className: 'mss-offline-why' }, tx('serverOfflineWhy')),
    error ? h('pre', { className: 'mss-offline-diag' }, error) : null,
    h('ol', { className: 'mss-offline-steps' },
      h('li', null, tx('serverOfflineStep1')),
      h('li', null, tx('serverOfflineStep2')),
      h('li', null, tx('serverOfflineStep3'))
    ),
    h('div', { className: 'mss-actions' },
      h('button', { className: 'mss-btn mss-btn-primary', onClick: onRetry, disabled: reconnecting },
        reconnecting ? h('span', { className: 'mss-spinner', 'aria-hidden': 'true' }) : null,
        tx(reconnecting ? 'retryWorking' : 'retryButton')),
      h('button', { className: 'mss-btn mss-btn-secondary', onClick: MSSOpenGitHub }, tx('helperButton')),
      h('button', { className: 'mss-btn mss-btn-secondary', onClick: MSSOpenInstallGuide }, tx('installGuideButton'))
    )
  );
}
