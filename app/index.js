// Memory Sentry Custom App entry point: mounts the dashboard and guards the bundle.
// The installer substitutes the placeholder found in the declaration below with an internal per-install access token.
// Spicetify concatenates the manifest subfiles before this file, so the token must stay here.
// Every component, helper and i18n table lives under app/ui/ and is loaded through manifest.json
// as subfiles; this entry only wires MSSApp to the DOM.

const MSS_TOKEN = '__MSS_TOKEN__';

function render() {
  // Sem literal do placeholder aqui: o replace do instalador e global, e trocaria a condicao
  // tambem, tornando o guard sempre verdadeiro (tela preta na rc.9).
  if (MSS_TOKEN.startsWith('__MSS')) {
    console.error('[Memory Sentry] Installer did not substitute the access token. Reinstall the official MSI.');
    return null;
  }
  if (typeof h !== 'function' || !window.MSS) {
    console.error('[Memory Sentry] App bundle is incomplete. Run "spicetify apply" and reopen the app.');
    return null;
  }
  return h(MSSApp, {});
}
