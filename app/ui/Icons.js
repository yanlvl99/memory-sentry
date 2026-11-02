// Lucide-style icon set (https://lucide.dev, ISC licence) inlined so the Custom App bundle
// carries no external dependency. Every icon shares a 24x24 viewBox and stroke="currentColor"
// so the parent button controls size and colour through CSS. Add new glyphs by extending the
// MSS_ICON_PATHS map; nothing else needs to change.
const MSS_ICON_PATHS = {
  auto: 'M13 2 3 14h9l-1 8 10-12h-9l1-8z',
  manual: null, // composite (three sliders) handled via MSS_ICON_BODY below
  disabled: 'M18.36 6.64a9 9 0 1 1-12.73 0 M12 2v10',
  custom: 'm12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3Z'
};
// Icons that need more than one primitive are declared as raw SVG markup fragments here.
const MSS_ICON_BODY = {
  manual: '<line x1="4" y1="6" x2="20" y2="6"/><line x1="4" y1="12" x2="20" y2="12"/><line x1="4" y1="18" x2="20" y2="18"/><circle cx="9" cy="6" r="2"/><circle cx="14" cy="12" r="2"/><circle cx="12" cy="18" r="2"/>',
  auto: '<path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/>',
  disabled: '<path d="M18.36 6.64a9 9 0 1 1-12.73 0"/><line x1="12" y1="2" x2="12" y2="12"/>',
  custom: '<path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3Z"/>',
  server: '<rect x="2" y="3" width="20" height="8" rx="2"/><rect x="2" y="13" width="20" height="8" rx="2"/><line x1="6" y1="7" x2="6.01" y2="7"/><line x1="6" y1="17" x2="6.01" y2="17"/>'
};
function MSSIcon(name) {
  const body = MSS_ICON_BODY[name];
  if (!body) return null;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${body}</svg>`;
  return h('span', { className: `mss-icon mss-icon-${name}`, dangerouslySetInnerHTML: { __html: svg } });
}
