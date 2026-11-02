// Resolves Spotify stock components once, with graceful fallbacks, so version drift is
// fixed in one place. Screens must use MSS.adapters instead of Spicetify.ReactComponent.
(function (MSS) {
  'use strict';
  const stock = (Spicetify && Spicetify.ReactComponent) || {};
  const pick = name => (typeof stock[name] === 'function' ? stock[name] : null);
  MSS.adapters = {
    Toggle: pick('Toggle'),
    ContextMenu: pick('ContextMenu'),
    Menu: pick('Menu'),
    MenuItem: pick('MenuItem'),
    TooltipWrapper: pick('TooltipWrapper'),
    IconComponent: pick('IconComponent'),
    TextComponent: pick('TextComponent'),
    Slider: pick('Slider'),
    ConfirmDialog: pick('ConfirmDialog'),
    PanelSkeleton: pick('PanelSkeleton'),
    PanelContent: pick('PanelContent'),
    PanelHeader: pick('PanelHeader')
  };
  MSS.adapters.available = Object.keys(stock).filter(name => typeof stock[name] === 'function').sort();
  MSS.adapters.report = () => Object.keys(MSS.adapters)
    .filter(name => name !== 'available' && name !== 'report')
    .reduce((acc, name) => { acc[name] = Boolean(MSS.adapters[name]); return acc; }, {});
})(window.MSS);
