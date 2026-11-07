// Memory Sentry dashboard core: shared constants and the MSS namespace.
// Spicetify concatenates manifest subfiles in the declared order, so keep this file first.
const MSSReact = typeof Spicetify !== 'undefined' ? Spicetify.React : null;
const h = MSSReact ? MSSReact.createElement : null;
const MSS_API = 'http://127.0.0.1:38472';
const MSS_GITHUB_URL = 'https://github.com/yanlvl99/memory-sentry';
const MSS_RELEASES_URL = `${MSS_GITHUB_URL}/releases`;
const MSS_INSTALL_GUIDE_URL = `${MSS_GITHUB_URL}/blob/main/docs/INSTALLATION.md`;
const MSS_LOCALE = (typeof navigator !== 'undefined' && /^pt(?:-|$)/i.test(navigator.language || '')) ? 'pt-BR' : 'en-US';
const MSS_PT = MSS_LOCALE === 'pt-BR';

// Presence marker: the entry point bails when the window global is missing (bundle mismatch).
window.MSS = window.MSS || {};

// One normalizer for dashboard config coming from any direction (initial state, GET status
// payload, POST config response) so the copies cannot drift. `previous` seeds the fields the
// server may omit (a partial POST response) — it defaults to the built-in defaults.
const MSS_CONFIG_DEFAULTS = { enabled: true, autoOptimize: true, customMode: false, hiddenDelaySeconds: 2, refreshIntervalMs: 700, trimToCap: true, optimizeWhenBackground: true, optimizeWhenMinimized: true, optimizeWhenHidden: true };
function MSSNormalizeConfig(raw, previous) {
  const r = (raw && typeof raw === 'object') ? raw : {};
  const prev = previous || MSS_CONFIG_DEFAULTS;
  const num = (value, fallback) => (Number(value) > 0 ? Number(value) : fallback);
  return {
    enabled: typeof r.enabled === 'boolean' ? r.enabled : prev.enabled,
    autoOptimize: r.autoOptimize !== false,
    customMode: r.customMode === true,
    hiddenDelaySeconds: num(r.hiddenDelaySeconds, prev.hiddenDelaySeconds),
    refreshIntervalMs: num(r.refreshIntervalMs, prev.refreshIntervalMs),
    trimToCap: r.trimToCap !== false,
    optimizeWhenBackground: r.optimizeWhenBackground !== false,
    optimizeWhenMinimized: r.optimizeWhenMinimized !== false,
    optimizeWhenHidden: r.optimizeWhenHidden !== false
  };
}
