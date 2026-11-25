/* Interface designs own layouts and decorative assets; colour and background are independent. */
(() => {
  const defaultColour = '#ae1236';
  // Add complete designs here, with a stylesheet that covers each Umbra window.
  const themes = [
    {id:'umbra', name:'Default', description:'Ornate silver frames and jewel accents.', stylesheet:'designs/umbra.css'}
  ];
  const find = id => themes.find(theme => theme.id === id) || themes[0];
  const validColour = value => /^#[0-9a-f]{6}$/i.test(value || '');
  const page = document.documentElement;
  let savedColour = defaultColour, previewColour = null, activeTheme = 'umbra', painted;
  const base = new URL('.', document.currentScript.src);
  const channel = typeof BroadcastChannel === 'function' ? new BroadcastChannel('umbra-appearance') : null;
  const mix = (colour, amount) => '#' + colour.slice(1).match(/../g).map(c => Math.round(parseInt(c,16)*(1-amount)+255*amount).toString(16).padStart(2,'0')).join('');
  function paint(force = false) {
    const colour = previewColour || savedColour;
    const key = `${activeTheme}:${colour}`;
    if (!force && painted === key) return;
    painted = key;
    page.style.setProperty('--accent', colour);
    page.style.setProperty('--silver', colour === defaultColour ? '#cabfb1' : mix(colour,.60));
    page.style.setProperty('--shine', colour === defaultColour ? '#fff3e7' : mix(colour,.92));
    page.dataset.theme = activeTheme;
    const matrix = document.getElementById('appearance-matrix');
    if (matrix) {
      const colours = getComputedStyle(page);
      const channels = name => colours.getPropertyValue(name).trim().slice(1).match(/../g).map(c => parseInt(c,16)/255);
      const accent = channels('--accent'), silver = channels('--silver'), shine = channels('--shine');
      matrix.setAttribute('values', accent.map((ruby,i) => [ruby,(silver[i]+shine[i])/2-ruby,0,0,0].join(' ')).join(' ')+' 0 0 0 1 0');
    }
    page.style.setProperty('--frame-filter', colour === defaultColour && activeTheme === 'umbra' ? 'none' : 'url(#theme-frame)');
    window.dispatchEvent(new CustomEvent('umbra-colour', {detail:colour}));
  }
  function apply(id, colour = defaultColour) {
    const theme = find(id);
    const changed = activeTheme !== theme.id;
    activeTheme = theme.id;
    savedColour = validColour(colour) ? colour.toLowerCase() : defaultColour;
    let link = document.getElementById('interface-design');
    if (!link) { link = document.createElement('link'); link.id = 'interface-design'; link.rel = 'stylesheet'; document.head.append(link); }
    const href = new URL(theme.stylesheet, base).href;
    if (link.href !== href) link.href = href;
    paint();
    try { if(localStorage.getItem('umbra-interface-design')!==theme.id) localStorage.setItem('umbra-interface-design',theme.id); } catch {}
    if(changed) window.dispatchEvent(new CustomEvent('umbra-design',{detail:theme.id}));
  }
  function preview(colour, broadcast = true) {
    previewColour = validColour(colour) ? colour.toLowerCase() : null;
    paint();
    if (broadcast) channel?.postMessage({preview:previewColour});
  }
  function remember(colour) {
    savedColour = validColour(colour) ? colour.toLowerCase() : defaultColour;
    previewColour = null;
    try { localStorage.setItem('umbra-asset-colour', savedColour); } catch {}
    channel?.postMessage({saved:savedColour});
    paint();
  }
  channel?.addEventListener('message', ({data}) => {
    if (Object.hasOwn(data,'saved') && validColour(data.saved)) { savedColour = data.saved; previewColour = null; paint(); }
    else if (Object.hasOwn(data,'preview')) preview(data.preview,false);
  });
  window.UmbraAppearance = {themes, find, apply, preview, remember, defaultColour, validColour, get colour(){return savedColour;}};
  let colour, design;
  try { colour = localStorage.getItem('umbra-asset-colour'); design = localStorage.getItem('umbra-interface-design');localStorage.removeItem('umbra-asset-opacity'); } catch {}
  apply(design, colour);
  document.addEventListener('DOMContentLoaded', () => paint(true));
  window.addEventListener('storage', event => { if(event.key === 'umbra-asset-colour') { savedColour = validColour(event.newValue) ? event.newValue : defaultColour; paint(); } });
  window.addEventListener('pagehide', () => { if(previewColour) channel?.postMessage({preview:null}); });
})();
