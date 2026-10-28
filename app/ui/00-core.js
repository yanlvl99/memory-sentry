// Memory Sentry dashboard core: shared constants and the MSS namespace.
// Spicetify concatenates manifest subfiles in the declared order, so keep this file first.
const MSSReact = Spicetify.React;
const h = MSSReact.createElement;
const MSS_API = 'http://127.0.0.1:38472';
const MSS_GITHUB_URL = 'https://github.com/yanlvl99/memory-sentry';
const MSS_RELEASES_URL = `${MSS_GITHUB_URL}/releases`;
const MSS_INSTALL_GUIDE_URL = `${MSS_GITHUB_URL}/blob/main/docs/INSTALLATION.md`;
const MSS_LOCALE = (typeof navigator !== 'undefined' && /^pt(?:-|$)/i.test(navigator.language || '')) ? 'pt-BR' : 'en-US';
const MSS_PT = MSS_LOCALE === 'pt-BR';
const MSS_UNCAPPED_SCALE_MB = 512;

window.MSS = window.MSS || { version: '1.0.0', adapters: {} };
