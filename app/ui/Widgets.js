// Small shared helpers and the accessible switch widget. Anything that lives here must be a
// pure function (no closure over App state) so screens can compose them freely.
function mssCopy(text) {
  try { if (typeof navigator !== 'undefined' && navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(String(text)); } catch (error) { /* clipboard unavailable */ }
}
function mssPidColor(pid) {
  const hue = (Number(pid) * 47 + 12) % 360;
  return `hsl(${hue}, 62%, 55%)`;
}
function MSSDebugToggle(value, onChange) {
  return h('button', {
    type: 'button',
    className: 'mss-switch',
    role: 'switch',
    'aria-checked': Boolean(value),
    'aria-label': tx('debugMode'),
    onClick: () => onChange(!value)
  }, h('span', { className: 'mss-switch-knob', 'aria-hidden': 'true' }));
}
// A toggle chip: one selectable option in a small multi-select group (e.g. the "optimize when"
// window states). Material selection-controls and Apple HIG favour a clear filled/hollow affordance
// over a bare native checkbox for touch-first settings; the dot makes the on/off state legible at a
// glance and the whole pill is the hit target. role=checkbox keeps it correct for screen readers.
function MSSChip(props) {
  const { label, checked, disabled, onClick } = props;
  return h('button', {
    type: 'button',
    className: `mss-chip${checked ? ' mss-chip-on' : ''}`,
    role: 'checkbox',
    'aria-checked': Boolean(checked),
    disabled: Boolean(disabled),
    title: label,
    onClick
  }, h('span', { className: 'mss-chip-dot', 'aria-hidden': 'true' }), h('span', { className: 'mss-chip-label' }, label));
}
// A numeric slider. Apple HIG, Material and Primer all recommend a slider (not a dropdown) for a
// continuous numeric setting: the user sees the whole range at once and can land on any value,
// while the live readout keeps the exact number legible. aria-valuetext feeds screen readers the
// formatted value ("2.5 seconds") instead of the bare number, and the native range keeps arrow keys.
// Crucially it does NOT persist on every frame: a native range fires onChange continuously while
// dragging, which would spam the save endpoint and let the async echo yank the thumb mid-drag. So we
// hold a local draft for smooth live feedback and only commit to the server when the user lets go
// (pointer up), finishes a keyboard step (key up) or tabs away (blur).
function MSSSlider(props) {
  const R = MSSReact;
  const { label, min, max, step, value, format, disabled, onChange } = props;
  const [draft, setDraft] = R.useState(Number(value));
  // Re-sync the draft to the authoritative value only when it changes from outside (initial load or
  // the echo after a commit). During a drag we never save, so `value` stays frozen and this does not
  // fight the thumb.
  R.useEffect(() => { setDraft(Number(value)); }, [value]);
  const text = format ? format(draft) : String(draft);
  const commit = () => { const next = Number(draft); if (next !== Number(value)) onChange(next); };
  // Fill percentage drives the green portion of the track (see .mss-range::-webkit-slider-runnable-
  // track). It updates instantly while scrubbing (no lag behind the pointer); the CSS transitions are
  // reserved for the thumb's hover/press micro-interactions so the animation reads as "smooth" without
  // making the value feel sluggish.
  const span = Number(max) - Number(min);
  const pct = span > 0 ? Math.min(100, Math.max(0, ((Number(draft) - Number(min)) / span) * 100)) : 0;
  return h('div', { className: 'mss-slider' },
    h('input', {
      className: 'mss-range',
      type: 'range',
      min, max, step,
      value: draft,
      disabled: Boolean(disabled),
      'aria-label': label,
      'aria-valuetext': text,
      style: { '--pct': `${pct}%` },
      onChange: event => setDraft(Number(event.target.value)),
      onPointerUp: commit,
      onKeyUp: commit,
      onBlur: commit
    }),
    h('span', { className: 'mss-slider-value' }, text)
  );
}
// A labelled settings row: a fixed-width label column beside a control column. The grid vertically
// centers both columns (see .mss-setting align-items), so chips, sliders and selects all line up on
// the same midline regardless of their height - this is the single source of truth for row spacing.
// An optional `description` renders as M3 list-item supporting text under the label, so every option
// explains what it does and how it behaves. controlProps lets a row add semantics to the control cell
// (e.g. role=group for a chip cluster).
function MSSSettingRow(props) {
  const { label, description, controlProps, children } = props;
  return h('div', { className: 'mss-setting' },
    h('div', { className: 'mss-setting-label' },
      h('span', { className: 'mss-setting-name' }, label),
      description ? h('span', { className: 'mss-setting-desc' }, description) : null
    ),
    h('div', Object.assign({ className: 'mss-setting-control' }, controlProps), children)
  );
}
// A reusable, themed dropdown. Centralizing the native <select> keeps the disabled/value/onChange
// plumbing identical everywhere and gives screen readers a label. options is [{ value, label }].
function MSSSelect(props) {
  const { label, value, options, disabled, onChange } = props;
  return h('select', {
    className: 'mss-select',
    'aria-label': label,
    disabled: Boolean(disabled),
    value: String(value),
    onChange: event => onChange(event.target.value)
  }, (options || []).map(o => h('option', { key: String(o.value), value: String(o.value) }, o.label)));
}
// A single segmented-control button used by the hero's mode picker. Icons are supplied by
// MSSIcon so hover/selection styling stays purely in CSS. In iconOnly mode (the default for
// the compact header picker) the label moves into the tooltip + aria-label so the segment
// stays balanced; Apple HIG and Primer both recommend exactly this for icon-only segments.
function MSSSegButton(props) {
  const { label, iconName, active, disabled, iconOnly, onClick } = props;
  return h('button', {
    type: 'button',
    className: `mss-seg-btn${active ? ' mss-on' : ''}${iconOnly ? ' mss-mode-btn' : ''}`,
    'aria-pressed': Boolean(active),
    'aria-label': label,
    disabled: Boolean(disabled),
    title: label,
    onClick
  }, MSSIcon(iconName), iconOnly ? null : h('span', { className: 'mss-seg-label' }, label));
}
