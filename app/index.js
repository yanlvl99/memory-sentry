// Memory Sentry Custom App. Source and default locale are English; Portuguese is selected automatically.
// The installer replaces __MSS_TOKEN__ with an internal per-install access token.

const MSS_TOKEN = '__MSS_TOKEN__';

function MSSEnsureStyles() {
  if (document.getElementById('mss-styles')) return;
  const style = document.createElement('style');
  style.id = 'mss-styles';
  style.textContent = `
    .mss-page{max-width:1200px;margin:0 auto;padding:28px 24px 48px;color:var(--spice-text,#f5f5f5);font-family:var(--font-family,inherit)}
    .mss-hero{display:flex;gap:20px;align-items:flex-start;justify-content:space-between;margin-bottom:22px}.mss-kicker{font-size:11px;font-weight:800;letter-spacing:.14em;text-transform:uppercase;color:var(--spice-subtext,#a7a7a7);margin-bottom:8px}.mss-title{font-size:clamp(30px,4vw,42px);line-height:1.08;letter-spacing:-.04em;font-weight:800;margin:0 0 10px}.mss-copy{color:var(--spice-subtext,#a7a7a7);font-size:14px;max-width:690px;line-height:1.6}
    .mss-pill{display:inline-flex;align-items:center;gap:8px;border:1px solid var(--spice-button,#1ed760);border-radius:999px;padding:8px 11px;font-size:12px;font-weight:700;white-space:nowrap}.mss-dot{width:7px;height:7px;border-radius:50%;background:var(--spice-button,#1ed760);box-shadow:0 0 0 4px color-mix(in srgb,var(--spice-button,#1ed760) 17%,transparent)}
    .mss-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,300px),1fr));gap:12px;margin:16px 0}.mss-card,.mss-section{background:var(--spice-card,#282828);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);border-radius:15px;padding:18px;min-width:0}.mss-card-label{font-size:12px;color:var(--spice-subtext,#a7a7a7);font-weight:700;margin-bottom:12px}.mss-number{font-size:31px;line-height:1;font-weight:800;letter-spacing:-.04em;font-variant-numeric:tabular-nums}.mss-unit{font-size:13px;color:var(--spice-subtext,#a7a7a7);font-weight:600;margin-left:5px}.mss-note{font-size:12px;color:var(--spice-subtext,#a7a7a7);line-height:1.48;margin-top:10px}
    .mss-section{margin-top:15px}.mss-section-head{display:flex;justify-content:space-between;align-items:center;gap:12px;margin-bottom:12px}.mss-h2{font-size:17px;font-weight:800;letter-spacing:-.02em;margin:0}.mss-small{font-size:12px;color:var(--spice-subtext,#a7a7a7);line-height:1.5}.mss-list{display:flex;flex-direction:column;gap:2px}.mss-process{display:grid;grid-template-columns:minmax(0,1fr) auto auto;gap:13px;align-items:center;padding:11px 0;border-bottom:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent)}.mss-process:last-child{border-bottom:0}.mss-process-name{font-size:13px;font-weight:700}.mss-process-meta{font-size:11px;color:var(--spice-subtext,#a7a7a7);margin-top:4px;line-height:1.4}.mss-memory{font-size:13px;font-variant-numeric:tabular-nums;font-weight:800;white-space:nowrap}.mss-tag{font-size:10px;text-transform:uppercase;letter-spacing:.06em;border-radius:6px;background:color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);padding:5px 7px;color:var(--spice-subtext,#a7a7a7);white-space:nowrap}
    .mss-settings{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:16px}.mss-setting{display:flex;flex-direction:column;gap:8px;min-width:0}.mss-setting-label{font-weight:700;font-size:13px}.mss-select{background:var(--spice-main,#121212);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 18%,transparent);color:var(--spice-text,#fff);border-radius:9px;padding:10px 11px;font-size:13px;width:100%}.mss-check{display:flex;align-items:flex-start;gap:10px;font-size:13px;line-height:1.45;cursor:pointer}.mss-check input{accent-color:var(--spice-button,#1ed760);margin-top:3px}.mss-actions{display:flex;align-items:center;gap:9px;flex-wrap:wrap;margin-top:16px}.mss-btn{border:0;border-radius:999px;padding:10px 15px;font-size:12px;font-weight:800;cursor:pointer;color:var(--spice-text,#fff);background:color-mix(in srgb,var(--spice-text,#fff) 10%,transparent)}.mss-btn:hover{filter:brightness(1.12)}.mss-btn:disabled{opacity:.45;cursor:not-allowed}.mss-btn-primary{background:var(--spice-button,#1ed760);color:var(--spice-button-text,#061006)}.mss-btn-danger{background:#d32f2f;color:#fff;border:1px solid #ff7777}.mss-btn-secondary{background:transparent;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 20%,transparent)}.mss-notice{border-radius:10px;padding:12px 14px;font-size:12px;line-height:1.5;margin:12px 0;background:color-mix(in srgb,var(--spice-text,#fff) 6%,transparent);color:var(--spice-subtext,#a7a7a7)}.mss-notice-warn{background:color-mix(in srgb,#e7a83e 12%,transparent);color:var(--spice-text,#fff);border:1px solid color-mix(in srgb,#e7a83e 30%,transparent)}.mss-modal .mss-notice-warn{background:color-mix(in srgb,#d32f2f 30%,var(--spice-card,#282828));color:#fff;border:2px solid #ff5252;font-weight:700}.mss-notice-error{background:color-mix(in srgb,#e05252 12%,transparent);color:var(--spice-text,#fff);border:1px solid color-mix(in srgb,#e05252 30%,transparent)}.mss-events{display:flex;flex-direction:column;gap:0}.mss-event{display:grid;grid-template-columns:90px minmax(0,1fr);gap:12px;padding:8px 0;border-bottom:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);font-size:12px;line-height:1.45}.mss-event-time{color:var(--spice-subtext,#a7a7a7);font-variant-numeric:tabular-nums}.mss-event-msg{overflow-wrap:anywhere}.mss-empty{padding:16px 0;color:var(--spice-subtext,#a7a7a7);font-size:13px}.mss-footer{margin-top:20px;color:var(--spice-subtext,#a7a7a7);font-size:11px;line-height:1.55}.mss-manual-controls{grid-column:1/-1;display:flex;align-items:center;gap:8px;flex-wrap:wrap}.mss-input{width:96px;background:var(--spice-main,#121212);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 20%,transparent);color:var(--spice-text,#fff);border-radius:8px;padding:8px;font-size:13px}.mss-modal-backdrop{position:fixed;inset:0;z-index:99999;background:#000b;display:grid;place-items:center;padding:20px}.mss-modal{max-width:500px;width:100%;background:var(--spice-card,#282828);color:var(--spice-text,#fff);border:1px solid color-mix(in srgb,var(--spice-text,#fff) 20%,transparent);border-radius:15px;padding:20px;box-shadow:0 18px 60px #0008}
    .mss-page{animation:mss-page-enter .46s ease-out both}.mss-hero{padding:24px;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 10%,transparent);border-radius:18px;background:linear-gradient(135deg,color-mix(in srgb,var(--spice-card,#282828) 96%,var(--spice-button,#1ed760)),var(--spice-main,#121212))}.mss-kicker{display:inline-block;border-radius:999px;padding:5px 8px;background:color-mix(in srgb,var(--spice-button,#1ed760) 12%,transparent)}.mss-card{background:linear-gradient(155deg,var(--spice-card,#282828),color-mix(in srgb,var(--spice-card,#282828) 85%,var(--spice-main,#121212)));transition:transform .18s ease}.mss-card:hover{transform:translateY(-3px)}.mss-grid>.mss-card{animation:mss-card-enter .42s ease-out both}.mss-grid>.mss-card:nth-child(2){animation-delay:60ms}.mss-grid>.mss-card:nth-child(3){animation-delay:120ms}.mss-grid>.mss-card:nth-child(n+4){animation-delay:180ms}.mss-section{background:linear-gradient(145deg,var(--spice-card,#282828),color-mix(in srgb,var(--spice-card,#282828) 92%,var(--spice-main,#121212)))}.mss-process{transition:transform .16s ease}.mss-process:hover{transform:translateX(3px)}.mss-btn{transition:transform .15s ease,filter .15s ease}.mss-btn:hover{transform:translateY(-1px)}.mss-power{position:relative;display:inline-flex;align-items:center;gap:9px;padding:8px 12px;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 18%,transparent);border-radius:999px;background:color-mix(in srgb,var(--spice-card,#282828) 86%,transparent);font-size:12px;font-weight:800;cursor:pointer;white-space:nowrap}.mss-power-on{border-color:color-mix(in srgb,var(--spice-button,#1ed760) 55%,transparent)}.mss-power-off{border-color:color-mix(in srgb,#e05252 65%,transparent)}.mss-power input{position:absolute;width:1px;height:1px;opacity:0}.mss-power-dot{width:9px;height:9px;flex:0 0 9px;border-radius:50%;background:#e05252}.mss-power-on .mss-power-dot{background:var(--spice-button,#1ed760);box-shadow:0 0 0 4px color-mix(in srgb,var(--spice-button,#1ed760) 17%,transparent)}.mss-power input:focus-visible+.mss-power-dot{outline:2px solid var(--spice-text,#fff);outline-offset:4px}.mss-screen-flash{position:fixed;inset:0;z-index:2147483647;display:grid;place-items:center;pointer-events:none;color:#fff;animation:mss-screen-transition .72s cubic-bezier(.2,.7,.2,1) both}.mss-screen-flash::before{content:"";position:absolute;inset:0}.mss-screen-flash-on::before{background:radial-gradient(ellipse at center,color-mix(in srgb,#1ed760 48%,transparent),color-mix(in srgb,#071b10 88%,transparent))}.mss-screen-flash-off::before{background:radial-gradient(ellipse at center,color-mix(in srgb,#e05252 42%,transparent),color-mix(in srgb,#1c0707 90%,transparent))}.mss-screen-flash span{position:relative;padding:14px 20px;border:1px solid #ffffff66;border-radius:999px;background:#101010aa;font-size:13px;font-weight:900;letter-spacing:.12em;text-align:center;box-shadow:0 8px 32px #0005}@keyframes mss-page-enter{from{opacity:0}to{opacity:1}}@keyframes mss-card-enter{from{opacity:0;transform:translateY(12px)}to{opacity:1;transform:translateY(0)}}@keyframes mss-screen-transition{0%{opacity:0;transform:scale(1.025)}15%{opacity:1}65%{opacity:.88}100%{opacity:0;transform:scale(1)}}
    @media(max-width:850px){.mss-page{padding:22px 14px 36px}.mss-grid{grid-template-columns:1fr}.mss-hero{flex-direction:column}.mss-settings{grid-template-columns:1fr}.mss-process{grid-template-columns:minmax(0,1fr) auto}.mss-process .mss-memory{grid-column:2;grid-row:1}.mss-tag{display:none}.mss-power{align-self:flex-start}}
    @media(prefers-reduced-motion:reduce){.mss-page,.mss-grid>.mss-card,.mss-screen-flash{animation-duration:1ms!important}.mss-card,.mss-process,.mss-btn{transition:none!important}}
    .mss-page,.mss-grid>.mss-card{animation:none}
    .mss-hero{display:grid;grid-template-columns:minmax(0,1fr) minmax(235px,280px);align-items:center;gap:clamp(22px,4vw,40px);padding:clamp(22px,3vw,32px)}
    .mss-hero-copy{min-width:0}.mss-kicker{display:block;padding:0;background:none;letter-spacing:.11em;line-height:1.5}.mss-title{font-size:clamp(34px,4.2vw,48px);margin:8px 0 12px}.mss-copy{max-width:62ch}.mss-window-state{display:block;margin-top:14px}
    .mss-hero-controls{display:grid;align-content:start;gap:10px;min-width:0;padding:14px;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);border-radius:15px;background:color-mix(in srgb,var(--spice-main,#121212) 42%,transparent)}
    .mss-pill{box-sizing:border-box;width:100%;min-height:38px;justify-content:flex-start;background:color-mix(in srgb,var(--spice-main,#121212) 45%,transparent);border-color:color-mix(in srgb,var(--spice-text,#fff) 18%,transparent)}.mss-pill-connected{border-color:color-mix(in srgb,var(--spice-button,#1ed760) 45%,transparent)}.mss-pill-waiting{border-color:color-mix(in srgb,#d6a03d 55%,transparent)}.mss-pill-error{border-color:color-mix(in srgb,#e05252 60%,transparent)}.mss-pill-error .mss-dot{box-shadow:0 0 0 4px color-mix(in srgb,#e05252 17%,transparent)}.mss-pill-waiting .mss-dot{box-shadow:0 0 0 4px color-mix(in srgb,#d6a03d 17%,transparent)}
    .mss-power{box-sizing:border-box;width:100%;min-width:0;display:flex;align-items:center;justify-content:space-between;gap:12px;padding:11px 12px;border-radius:12px;white-space:normal;background:color-mix(in srgb,var(--spice-card,#282828) 92%,transparent);transition:border-color .16s ease,background-color .16s ease}.mss-power-on{border-color:color-mix(in srgb,var(--spice-button,#1ed760) 38%,transparent)}.mss-power-off{border-color:color-mix(in srgb,#e05252 42%,transparent)}.mss-power-disabled{opacity:.58;cursor:not-allowed}.mss-power-copy{display:grid;gap:3px;min-width:0}.mss-power-title{font-size:12px;font-weight:800;line-height:1.35}.mss-power-hint{font-size:10px;line-height:1.4;color:var(--spice-subtext,#a7a7a7)}.mss-power input{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.mss-switch-track{width:38px;height:22px;flex:0 0 38px;display:flex;align-items:center;padding:3px;border-radius:999px;background:color-mix(in srgb,var(--spice-text,#fff) 22%,transparent);transition:background-color .16s ease}.mss-switch-thumb{width:16px;height:16px;border-radius:50%;background:#fff;box-shadow:0 1px 3px #0005;transition:transform .16s ease}.mss-power input:checked~.mss-switch-track{background:var(--spice-button,#1ed760)}.mss-power input:checked~.mss-switch-track .mss-switch-thumb{transform:translateX(16px)}.mss-power input:focus-visible~.mss-switch-track{outline:2px solid var(--spice-text,#fff);outline-offset:3px}
    .mss-debug-row{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:5px 2px 0;border-top:1px solid color-mix(in srgb,var(--spice-text,#fff) 9%,transparent)}.mss-debug-label{font-size:12px;color:var(--spice-subtext,#a7a7a7)}.mss-debug-fallback{position:relative;width:38px;height:22px;flex:0 0 38px;padding:3px;border:0;border-radius:999px;background:color-mix(in srgb,var(--spice-text,#fff) 22%,transparent);cursor:pointer;transition:background-color .16s ease}.mss-debug-fallback[aria-checked=true]{background:var(--spice-button,#1ed760)}.mss-debug-fallback-thumb{display:block;width:16px;height:16px;border-radius:50%;background:#fff;box-shadow:0 1px 3px #0005;transition:transform .16s ease}.mss-debug-fallback[aria-checked=true] .mss-debug-fallback-thumb{transform:translateX(16px)}.mss-debug-fallback:focus-visible{outline:2px solid var(--spice-text,#fff);outline-offset:3px}
    .mss-card{transition:border-color .16s ease,box-shadow .16s ease}.mss-card:hover{transform:none;border-color:color-mix(in srgb,var(--spice-text,#fff) 16%,transparent);box-shadow:0 5px 16px #0002}.mss-process{transition:background-color .14s ease}.mss-process:hover{transform:none;background:color-mix(in srgb,var(--spice-text,#fff) 3%,transparent)}.mss-btn{transition:filter .14s ease}.mss-btn:hover{transform:none;filter:brightness(1.07)}
    .mss-screen-flash{display:flex;align-items:flex-start;justify-content:center;padding:clamp(62px,11vh,104px) 16px 0;animation:mss-screen-transition .52s ease-out both}.mss-screen-flash-on::before{background:color-mix(in srgb,var(--spice-button,#1ed760) 12%,transparent)}.mss-screen-flash-off::before{background:color-mix(in srgb,#e05252 11%,transparent)}.mss-screen-flash span{padding:9px 14px;border-color:color-mix(in srgb,#fff 18%,transparent);background:color-mix(in srgb,var(--spice-card,#282828) 92%,transparent);font-size:11px;letter-spacing:.07em;box-shadow:0 4px 16px #0003}
    @keyframes mss-screen-transition{0%{opacity:0}18%{opacity:.9}100%{opacity:0}}
    @media(max-width:850px){.mss-power{align-self:stretch}}
    .mss-safety-note{padding:14px 16px;background:color-mix(in srgb,#d6a03d 6%,var(--spice-card,#282828));color:var(--spice-text,#f5f5f5);border:1px solid color-mix(in srgb,#d6a03d 18%,transparent);border-left:3px solid #d6a03d}
    .mss-setting-group-head{display:flex;align-items:baseline;justify-content:space-between;gap:12px;margin:18px 0 10px}.mss-setting-group-title{margin:0;font-size:13px;font-weight:800}.mss-settings{gap:12px}.mss-setting{padding:14px;border:1px solid color-mix(in srgb,var(--spice-text,#fff) 8%,transparent);border-radius:12px;background:color-mix(in srgb,var(--spice-main,#121212) 28%,transparent)}.mss-select:disabled{opacity:.62;cursor:not-allowed}
    @media(max-width:680px){.mss-hero{grid-template-columns:minmax(0,1fr);align-items:stretch;gap:22px}.mss-hero-controls{width:100%}.mss-title{font-size:clamp(32px,9vw,42px)}.mss-setting-group-head{align-items:flex-start;flex-direction:column;gap:3px}}
    @media(prefers-reduced-motion:reduce){.mss-screen-flash{animation-duration:1ms!important}.mss-card,.mss-process,.mss-btn,.mss-power,.mss-switch-thumb,.mss-debug-fallback,.mss-debug-fallback-thumb{transition:none!important}}
  `;
  document.head.appendChild(style);
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

function render() { MSSEnsureStyles(); return h(MSSApp, {}); }
