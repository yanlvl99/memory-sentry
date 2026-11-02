// The single-row topbar: brand on the left, connection pill on the right. Kept as a pure
// component so App.js can pass the current status without the topbar knowing anything about
// memory, config, or the polling loop.
function TopBar(props) {
  const { error, helperRunning, connection } = props;
  // Prefer the explicit three-state signal from the poll loop; fall back to the legacy inference so
  // the pill is never wrong if connection is not yet set. 'lost' (amber, reconnecting) is deliberately
  // distinct from 'missing' (red, not installed) so a dropped connection is not read as a fresh install.
  const state = connection || (error ? 'missing' : (helperRunning ? 'online' : 'unknown'));
  const tone = state === 'online' ? 'connected' : state === 'lost' ? 'lost' : state === 'missing' ? 'error' : 'waiting';
  const color = state === 'online' ? 'var(--spice-button,#1ed760)' : state === 'lost' ? '#d6a03d' : state === 'missing' ? '#e05252' : '#d6a03d';
  const label = state === 'online' ? tx('connected') : state === 'lost' ? tx('statusLost') : state === 'missing' ? tx('unavailable') : tx('waiting');
  return h('header', { className: 'mss-topbar' },
    h('div', { className: 'mss-brand' },
      h('h1', { className: 'mss-title' }, tx('title')),
      h('div', { className: 'mss-subtitle' }, tx('brandSubtitle'))
    ),
    h('div', { className: 'mss-topbar-controls' },
      h('div', { className: `mss-pill mss-pill-${tone}`, role: 'status' },
        h('span', { className: 'mss-dot', 'aria-hidden': 'true', style: { background: color } }), label
      )
    )
  );
}
