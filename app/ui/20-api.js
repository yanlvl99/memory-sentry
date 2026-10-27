// Local API client and formatting helpers shared by the dashboard.
async function MSSRequest(path, options = {}) {
  const response = await fetch(`${MSS_API}${path}`, {
    method: options.method || 'GET', cache: 'no-store', mode: 'cors', credentials: 'omit',
    headers: { Authorization: `Bearer ${MSS_TOKEN}`, ...(options.body ? { 'Content-Type': 'application/json' } : {}) },
    body: options.body ? JSON.stringify(options.body) : undefined
  });
  let payload = {};
  try { payload = await response.json(); } catch (_) { }
  if (!response.ok) throw new Error(payload.error || `${tx('helperApiError')} (HTTP ${response.status}).`);
  return payload;
}
function MSSFormatMB(value) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return '—';
  return Number(value).toLocaleString(MSS_LOCALE, { maximumFractionDigits: 1, minimumFractionDigits: 1 });
}
function MSSManualWarningSeconds(readyAt, now) {
  return Math.max(0, Math.ceil((readyAt - now) / 1000));
}
function MSSWindowLabel(value) {
  return T.window[value] || (MSS_TEXT.en.window[value] || 'Unknown state');
}
function MSSRoleLabel(role) {
  if (!MSS_PT) return role;
  return ({ Main: 'Principal', GPU: 'GPU', Renderer: 'Renderização', Utility: 'Utilitário', Crashpad: 'Relatório de falhas', Helper: 'Auxiliar', Audio: 'Áudio' })[role] || role;
}
function MSSOpenGitHub() {
  window.open(MSS_RELEASES_URL, '_blank', 'noopener,noreferrer');
}

function MSSOpenInstallGuide() {
  window.open(MSS_INSTALL_GUIDE_URL, '_blank', 'noopener,noreferrer');
}
function MSSDate(value) {
  return value ? new Date(value).toLocaleTimeString(MSS_LOCALE) : '—';
}
