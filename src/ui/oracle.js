import {shortcutFromEvent,shortcutLabel} from './shortcuts.mjs';
import {captureController,revisionGate,mergeSnapshot,refreshController} from './async-state.mjs';
const $=s=>document.querySelector(s);
const invoke=(kind,id=null)=>window.__TAURI__.core.invoke('action',{kind,id});
const modes={rift:'Summoner’s Rift',aram:'ARAM',arena:'Arena',mayhem:'ARAM: Mayhem'};
let state,chosen=103,lastShortcut='F1',toastTimer,mutation=Promise.resolve(),keySaving=false,appliedColour,latestUpdate;
const revisions=revisionGate(),failedPortraits=new Set();
const recording=captureController(on=>invoke('capture',on),()=>{if(state)paint();});
const champions=()=>state?.champions||[];
function toast(message){$('#toast').textContent=String(message);$('#toast').hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').hidden=true,3200);}
function error(e){toast(e?.message||e);}
function paintUpdate(update){
  if(!update||latestUpdate&&update.revision<latestUpdate.revision)return;
  if(latestUpdate&&JSON.stringify(update)===JSON.stringify(latestUpdate))return;
  latestUpdate=update;$('#update-status').textContent=update.message;$('#check-updates').disabled=update.busy;$('#check-updates').textContent=update.busy?'Updating…':'Check for updates';
}
function paint(){
  paintUpdate(state.update);
  const capturing=recording.active;
  const settings=state.settings,target=state.guideTarget,champion=champions().find(c=>c.id===target?.id);
  if(appliedColour!==settings.assetColour){window.UmbraAppearance?.apply('umbra',settings.assetColour);appliedColour=settings.assetColour;}
  const label=shortcutLabel(settings.guideHotkey);
  if(settings.guideHotkey)lastShortcut=settings.guideHotkey;
  $('#record-key').textContent=capturing?(recording.pending?'Starting…':'Press keys…'):label||'Off';$('#record-key').setAttribute('aria-busy',String(recording.pending));$('#record-key').classList.toggle('recording',capturing);$('#record-key').classList.toggle('long',label.length>5);
  $('#shortcut-state').textContent=capturing?'Press your shortcut · Esc cancels.':state.shortcutStatus==='conflict'?'Shortcut in use · in-game key detection is active.':settings.guideHotkey?'Click the key to change it.':'Disabled · click the key to enable.';
  $('#version').textContent=state.version.replace(/\.0$/,'');
  const connected=state.leagueConnected||state.phase==='InProgress'||!!champion;
  $('#connection').textContent=connected?'League connected':'Waiting for League';$('#connection').classList.toggle('connected',connected);
  $('#match-champion').textContent=champion?.name||'No active match';$('#match-detail').textContent=champion?modes[target.mode]:state.detectedChampion?'Choose the mode below.':'Your champion connects automatically.';
  const art=champion?.icon&&!failedPortraits.has(champion.icon)?champion.icon:'oracle-portrait.svg';if($('#match-icon').getAttribute('src')!==art)$('#match-icon').src=art;
  $('#live-mark').hidden=!champion;$('#open-current').disabled=!target;
  $('#start-windows').checked=settings.startWithWindows;$('#launch-tray').checked=settings.launchInTray;$('#enable-key').checked=!!settings.guideHotkey;
  $('#enable-overlay').checked=!!settings.overlayEnabled;
  $('#overlay-status').textContent=state.overlay?.status==='unavailable'||state.overlay?.visible?state.overlay.message:'Mayhem · Borderless · English names';
  if(document.activeElement!==$('#accent-colour'))$('#accent-colour').value=settings.assetColour;
  $('#app-error').hidden=!state.error;$('#app-error span').textContent=state.error||'';
}
function change(patch){
  const done=revisions.begin();
  const result=mutation.catch(()=>{}).then(async()=>{state=await invoke('save-settings',patch);paint();return state;});
  mutation=result.finally(done);mutation.catch(()=>{});return mutation;
}
function setTab(tab){if(!['overview','settings'].includes(tab))return;capture(false).catch(error);for(const page of document.querySelectorAll('.page'))page.hidden=page.id!==tab;for(const button of document.querySelectorAll('nav [data-tab]')){button.classList.toggle('active',button.dataset.tab===tab);button.setAttribute('aria-selected',String(button.dataset.tab===tab));}}
const capture=on=>recording.set(on);
const guideSelection=()=>({champion:chosen,mode:$('#desk-mode').value});
async function rememberGuide(selection=guideSelection()){return change({lastGuide:selection});}
async function openGuide(){const button=$('#open-guide'),selection=guideSelection();button.disabled=true;try{await rememberGuide(selection);await invoke('open-guide',selection);}catch(e){error(e);}finally{button.disabled=false;}}
async function init(){
  if(!window.__TAURI__){$('#app-error').hidden=false;$('#app-error span').textContent='Open the Oracle desktop app.';return;}
  state=await window.__TAURI__.core.invoke('ready');chosen=state.settings.lastGuide?.champion||chosen;if(!champions().some(c=>c.id===chosen))chosen=champions()[0]?.id;
  $('#desk-champion').replaceChildren(...champions().map(c=>new Option(c.name,String(c.id))));$('#desk-champion').value=String(chosen);$('#desk-mode').value=state.settings.lastGuide?.mode||'rift';
  const provider=()=>$('#provider-label').textContent=$('#desk-mode').value==='mayhem'?'Tencent':'MetaSRC';provider();paint();
  document.addEventListener('click',e=>{const tab=e.target.closest('[data-tab]')?.dataset.tab;if(tab)setTab(tab);const action=e.target.closest('[data-action]')?.dataset.action;if(action)capture(false).then(()=>invoke(action)).catch(error);});
  document.addEventListener('mousedown',e=>{if(e.button===0&&e.target.closest('[data-tauri-drag-region]'))window.__TAURI__.window.getCurrentWindow().startDragging().catch(error);});
  $('#desk-champion').addEventListener('change',e=>{chosen=Number(e.target.value);rememberGuide().catch(error);});$('#desk-mode').addEventListener('change',()=>{provider();rememberGuide().catch(error);});
  $('#open-guide').addEventListener('click',openGuide);$('#open-current').addEventListener('click',async()=>{const button=$('#open-current');button.disabled=true;try{await invoke('open-current');}catch(e){error(e);}finally{paint();}});
  $('#record-key').addEventListener('click',()=>capture(!recording.active).catch(error));
  $('#reset-key').addEventListener('click',async()=>{try{await capture(false);await change({guideHotkey:'F1'});toast('Shortcut reset to F1.');}catch(e){error(e);}});
  $('#enable-key').addEventListener('change',e=>change({guideHotkey:e.target.checked?lastShortcut:null}).catch(e=>{paint();error(e);}));
  document.addEventListener('keydown',async e=>{if(!recording.active)return;e.preventDefault();e.stopPropagation();if(e.key==='Escape'){await capture(false).catch(error);return;}if(e.repeat||recording.pending||keySaving)return;const binding=shortcutFromEvent(e);if(!binding)return;keySaving=true;try{await change({guideHotkey:binding});await capture(false);toast(`Shortcut saved: ${shortcutLabel(binding)}`);}catch(e){error(e);}finally{keySaving=false;}},true);
  window.addEventListener('blur',()=>capture(false).catch(error));
  for(const [id,key]of [['start-windows','startWithWindows'],['launch-tray','launchInTray'],['enable-overlay','overlayEnabled']])$('#'+id).addEventListener('change',e=>change({[key]:e.target.checked}).catch(e=>{paint();error(e);}));
  $('#preview-overlay').addEventListener('click',async()=>{const button=$('#preview-overlay');button.disabled=true;try{await invoke('overlay-preview');}catch(e){error(e);}finally{button.disabled=false;}});
  $('#accent-colour').addEventListener('input',e=>window.UmbraAppearance?.apply('umbra',e.target.value));$('#accent-colour').addEventListener('change',e=>change({assetColour:e.target.value}).catch(e=>{appliedColour=undefined;paint();error(e);}));$('#reset-colour').addEventListener('click',()=>change({assetColour:'#ae1236'}).catch(error));
  $('#dismiss-error').addEventListener('click',()=>invoke('dismiss-error').then(v=>{state.error=v.error;paint();}).catch(error));$('#match-icon').addEventListener('error',()=>{const icon=$('#match-icon');if(!icon.src.endsWith('/oracle-portrait.svg')){failedPortraits.add(icon.getAttribute('src'));icon.src='oracle-portrait.svg';}});
  await window.__TAURI__.event.listen('oracle-error',({payload})=>error(payload));
  await window.__TAURI__.event.listen('oracle-update',({payload})=>paintUpdate(payload));
  $('#check-updates').addEventListener('click',async()=>{const button=$('#check-updates');button.disabled=true;try{paintUpdate((await invoke('check-updates')).update);}catch(e){button.disabled=false;error(e);}});
  const refresh=refreshController(async()=>{
    try{const revision=revisions.snapshot();if(revisions.accepts(revision)){const snapshot=await window.__TAURI__.core.invoke('ready',{compact:true});if(revisions.accepts(revision)){const next=mergeSnapshot(state,snapshot);if(next!==state){state=next;paint();}}}}catch(e){error(e);}
  });
  const dashboard=window.__TAURI__.window.getCurrentWindow();
  // Native visibility is authoritative: WebView2 focus/document visibility can lag
  // a hide/show. Listen before the initial query so a concurrent hide wins.
  let visibilityRevision=0;
  await window.__TAURI__.event.listen('oracle-dashboard-visible',({payload})=>{visibilityRevision++;refresh.setActive(payload);});
  const revision=visibilityRevision,[visible,minimized]=await Promise.all([dashboard.isVisible(),dashboard.isMinimized()]);if(revision===visibilityRevision)refresh.setActive(visible&&!minimized);
}
init().catch(error);
