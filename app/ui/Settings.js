// Settings card. Every row is a reusable MSSSettingRow (label | control, vertically centered) so the
// card stays consistent and adding a setting is one call. Controls are shared widgets: MSSChip for
// the multi-select triggers, MSSSlider for continuous numeric values, MSSSelect for enumerated modes.
// The mode picker lives in the hero, not here.
function Settings(props) {
  const { config, saving, helperRunning, status, onField, onTrigger, activeTriggers } = props;
  const controlDisabled = saving || !helperRunning || !config.enabled;
  const triggerOptions = [
    { key: 'optimizeWhenBackground', label: tx('whenBackground') },
    { key: 'optimizeWhenMinimized', label: tx('whenMinimized') },
    { key: 'optimizeWhenHidden', label: tx('whenHidden') }
  ];
  return h('section', { className: 'mss-toolbar' },
    h('div', { className: 'mss-toolbar-head' },
      h('div', null,
        h('div', { className: 'mss-toolbar-title' }, tx('backgroundSettingsTitle')),
        h('div', { className: 'mss-toolbar-hint' }, tx('backgroundSettingsHint'))
      ),
      h('span', { className: 'mss-saved', 'aria-live': 'polite' }, h('span', { className: 'mss-saved-dot', 'aria-hidden': 'true' }), tx(saving ? 'saving' : 'autoSavedShort'))
    ),
    h(MSSSettingRow, { label: tx('whenLabel'), description: tx('whenDesc'), controlProps: { role: 'group', 'aria-label': tx('whenLabel') } },
      triggerOptions.map(option => h(MSSChip, {
        key: option.key,
        label: option.label,
        checked: Boolean(config[option.key]),
        disabled: controlDisabled || (Boolean(config[option.key]) && activeTriggers() <= 1),
        onClick: () => onTrigger(option.key, !config[option.key])
      }))
    ),
    h(MSSSettingRow, { label: tx('delayShort'), description: tx('delayDesc') },
      h(MSSSlider, {
        label: tx('delayShort'),
        min: 0.5, max: 10, step: 0.5,
        value: Number(config.hiddenDelaySeconds),
        disabled: controlDisabled,
        format: v => MSSCountValue(v),
        onChange: v => onField('hiddenDelaySeconds', v)
      })
    ),
    h(MSSSettingRow, { label: tx('refreshShort'), description: tx('refreshDesc') },
      h(MSSSlider, {
        label: tx('refreshShort'),
        min: 250, max: 2000, step: 50,
        value: Number(config.refreshIntervalMs),
        disabled: controlDisabled,
        format: v => tx('refreshValue', { value: v }),
        onChange: v => onField('refreshIntervalMs', v)
      })
    ),
    h(MSSSettingRow, { label: tx('releaseShort'), description: tx('releaseDesc') },
      h(MSSSelect, {
        label: tx('releaseShort'),
        disabled: controlDisabled,
        value: config.trimToCap !== false ? 'ceiling' : 'aggressive',
        options: [{ value: 'ceiling', label: tx('releaseCeiling') }, { value: 'aggressive', label: tx('releaseAggressive') }],
        onChange: v => onField('trimToCap', v === 'ceiling')
      })
    )
  );
}
