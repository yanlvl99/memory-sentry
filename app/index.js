// Memory Sentry Custom App entry point: mounts the dashboard and guards the bundle.
// The installer replaces __MSS_TOKEN__ with an internal per-install access token.
// Spicetify concatenates the manifest subfiles before this file, so the token must stay here.
// Every component, helper and i18n table lives under app/ui/ and is loaded through manifest.json
// as subfiles; this entry only wires MSSApp to the DOM and prints a one-shot adapter report.

const MSS_TOKEN = '__MSS_TOKEN__';

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
