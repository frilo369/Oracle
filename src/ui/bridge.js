(() => {
  if (!window.__TAURI__) return;
  window.umbraGuide = {invoke:(command,args={})=>window.__TAURI__.core.invoke('guide_data',{command,args})};
})();
