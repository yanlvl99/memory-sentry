// Fine print (safety accordion) and the static footer. Splitting it out keeps App.js focused
// on state + orchestration while the copy itself stays near the strings in I18n.
function Fineprint() {
  return h('details', { className: 'mss-fineprint' },
    h('summary', null, tx('safetySummary')),
    h('p', null, tx('intro')),
    h('p', null, tx('safety'))
  );
}

function Footer() {
  return h('footer', { className: 'mss-footer' },
    h('span', null, 'Windows 10/11 x64'),
    h('span', null, 'GPL-3.0'),
    h('span', null, MSS_PT ? 'nada sai do PC' : 'nothing leaves the PC'),
    h('span', null, `${tx('footer')} · ${MSS_PT ? 'Idioma automático: português' : 'Auto language: English'}`)
  );
}
