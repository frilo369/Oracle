import {shortcutFromEvent,shortcutLabel} from './shortcuts.mjs';
const $=s=>document.querySelector(s);
const invoke=(kind,id=null)=>window.__TAURI__.core.invoke('action',{kind,id});
let state,chosen=103,capturing=false,noteDirty=false,noteChampion=103,favoritesOnly=false,toastTimer;
const modes={rift:'Summoner’s Rift',aram:'ARAM',arena:'Arena',mayhem:'ARAM: Mayhem'};
const escape=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
function toast(message){$('#toast').textContent=message;$('#toast').hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').hidden=true,3500);}
function error(e){toast(String(e?.message||e));}
async function change(patch){state=await invoke('save-settings',patch);paint();return state;}
function champions(){return state?.champions||[];}
function paint(){
  const settings=state.settings,target=state.guideTarget;
  window.UmbraAppearance?.apply('umbra',settings.assetColour);
  const label=shortcutLabel(settings.guideHotkey);
  $('#hero-key').textContent=label||'OFF';$('#hero-key').classList.toggle('long',label.length>3);
  if(!capturing)$('#record-key').innerHTML=`${escape(label||'Disabled')}<span>Change</span>`;
  $('#version').textContent=`VERSION ${state.version.replace(/\.0$/,'')}`;
  const connected=state.leagueConnected||state.phase==='InProgress';
  $('#connection').textContent=connected?'League connected':'Waiting for League';$('#connection').classList.toggle('connected',connected);
  const champion=champions().find(c=>c.id===target?.id);
  $('#match-state').textContent=champion?`${champion.name} · ${modes[target.mode]}`:connected?'League connected · waiting for champion & mode':'Waiting for your next match';
  $('#current-caption').textContent=champion?`${champion.name} · ${modes[target.mode]} · Ready to explore`:state.detectedChampion?'Mode unavailable. Choose your mode at the build desk.':'Start a match to connect automatically.';
  $('#open-current').disabled=!target;
  $('#favorite').classList.toggle('selected',settings.favorites.includes(chosen));$('#favorite').textContent=settings.favorites.includes(chosen)?'★':'☆';$('#favorite').setAttribute('aria-label',settings.favorites.includes(chosen)?'Remove champion from favorites':'Favorite this champion');
  $('#start-windows').checked=settings.startWithWindows;$('#launch-tray').checked=settings.launchInTray;
  if(document.activeElement!==$('#accent-colour'))$('#accent-colour').value=settings.assetColour;
  $('#shortcut-state').textContent=state.shortcutStatus==='conflict'?'Another app has this shortcut. Oracle will try detecting the key while League is in front.':settings.guideHotkey?'Active while League’s game window is in front.':'Shortcut disabled. You can still use the build desk.';
  $('#status-label').textContent=capturing?'Recording a new shortcut…':champion?`${champion.name} guide ready · ${label||'shortcut disabled'}`:'Oracle is ready in your tray';
  $('#app-error').hidden=!state.error;$('#app-error span').textContent=state.error||'';
  const favoriteRows=champions().filter(c=>settings.favorites.includes(c.id));
  $('#favorite-chips').innerHTML=favoriteRows.length?favoriteRows.map(c=>`<button data-champion="${c.id}">${escape(c.name)}</button>`).join(''):'<p>Star a champion at the build desk to keep them here.</p>';
}
function fillSelectors(){const html=champions().map(c=>`<option value="${c.id}">${escape(c.name)}</option>`).join('');for(const id of ['desk-champion','note-champion']){$('#'+id).innerHTML=html;$('#'+id).value=String(chosen);}}
function drawLibrary(){
  const search=$('#champion-search').value.trim().toLocaleLowerCase();
  const rows=champions().filter(c=>c.name.toLocaleLowerCase().includes(search)&&(!favoritesOnly||state.settings.favorites.includes(c.id)));
  $('#champion-count').textContent=`${rows.length} champions`;
  $('#champion-grid').innerHTML=rows.map(c=>`<button class="champion-card" data-champion="${c.id}" title="Explore ${escape(c.name)}"><img src="${escape(c.icon)}" alt="" loading="lazy" decoding="async"><strong>${escape(c.name)}</strong>${state.settings.favorites.includes(c.id)?'<span class="star">★</span>':''}</button>`).join('');$('#no-champions').hidden=rows.length>0;
}
function loadNote(){noteChampion=chosen;$('#note-champion').value=String(chosen);$('#note-text').value=state.settings.notes[String(chosen)]||'';noteDirty=false;paintNote();}
function paintNote(){$('#note-count').textContent=`${$('#note-text').value.length} / 2000`;$('#note-state').textContent=noteDirty?'Unsaved changes':state.settings.notes[String(noteChampion)]?'Saved on this PC':'No saved note';}
async function saveNote(){if(!noteDirty)return;const notes={...state.settings.notes};const text=$('#note-text').value;if(text.trim())notes[String(noteChampion)]=text;else delete notes[String(noteChampion)];await change({notes});noteDirty=false;paintNote();}
async function selectChampion(id){await saveNote();chosen=Number(id);$('#desk-champion').value=String(chosen);loadNote();paint();}
async function setTab(tab){if(capturing)await capture(false);for(const page of document.querySelectorAll('.page')){page.hidden=page.id!==tab;page.classList.toggle('active',page.id===tab);}for(const nav of document.querySelectorAll('.nav'))nav.classList.toggle('active',nav.dataset.tab===tab);if(tab==='library')drawLibrary();}
async function capture(on){await invoke('capture',on);capturing=on;$('#record-key').classList.toggle('recording',on);$('#capture-hint').hidden=!on;if(on)$('#record-key').innerHTML='…<span>Recording</span>';paint();}
async function openGuide(){const button=$('#open-guide');button.disabled=true;try{await invoke('open-guide',{champion:chosen,mode:$('#desk-mode').value});}catch(e){error(e);}finally{button.disabled=false;}}
async function init(){
  if(!window.__TAURI__){$('#app-error').hidden=false;$('#app-error span').textContent='Open the Oracle desktop app to connect to League.';return;}
  state=await window.__TAURI__.core.invoke('ready');
  if(!champions().some(c=>c.id===chosen))chosen=champions()[0]?.id;
  fillSelectors();loadNote();paint();
  document.addEventListener('click',async e=>{
    const tab=e.target.closest('[data-tab]')?.dataset.tab||e.target.closest('[data-go]')?.dataset.go;if(tab)try{await setTab(tab);}catch(e){error(e);}
    const action=e.target.closest('[data-action]')?.dataset.action;if(action)try{await saveNote();await invoke(action);}catch(e){error(e);}
    const card=e.target.closest('[data-champion]');if(card)try{await selectChampion(card.dataset.champion);await setTab('overview');}catch(e){error(e);}
  });
  document.addEventListener('mousedown',e=>{if(e.button===0&&e.target.closest('[data-tauri-drag-region]'))window.__TAURI__.window.getCurrentWindow().startDragging().catch(error);});
  $('#desk-champion').addEventListener('change',e=>selectChampion(e.target.value).catch(error));
  $('#note-champion').addEventListener('change',e=>selectChampion(e.target.value).catch(error));
  $('#desk-mode').addEventListener('change',()=>$('#provider-label').textContent=$('#desk-mode').value==='mayhem'?'Mayhem recommendations from Tencent · English game data':'Builds & statistics from MetaSRC');
  $('#open-guide').addEventListener('click',openGuide);
  $('#open-current').addEventListener('click',()=>invoke('open-current').catch(error));
  $('#favorite').addEventListener('click',()=>{const old=state.settings.favorites;change({favorites:old.includes(chosen)?old.filter(id=>id!==chosen):[...old,chosen]}).catch(error);});
  $('#champion-search').addEventListener('input',drawLibrary);
  $('#favorites-only').addEventListener('click',()=>{favoritesOnly=!favoritesOnly;$('#favorites-only').setAttribute('aria-pressed',String(favoritesOnly));drawLibrary();});
  $('#note-text').addEventListener('input',()=>{noteDirty=true;paintNote();});
  $('#save-note').addEventListener('click',()=>saveNote().then(()=>toast('Note saved on this PC.')).catch(error));
  $('#record-key').addEventListener('click',()=>capture(!capturing).catch(error));
  $('#reset-key').addEventListener('click',async()=>{try{await capture(false);await change({guideHotkey:'F1'});toast('Shortcut reset to F1.');}catch(e){error(e);}});
  $('#disable-key').addEventListener('click',async()=>{try{await capture(false);await change({guideHotkey:null});}catch(e){error(e);}});
  document.addEventListener('keydown',async e=>{
    if(!capturing)return;e.preventDefault();e.stopPropagation();
    if(e.key==='Escape'){await capture(false).catch(error);return;}
    if(e.repeat)return;const binding=shortcutFromEvent(e);if(!binding)return;
    try{await change({guideHotkey:binding});await capture(false);toast(`Guide shortcut saved: ${shortcutLabel(binding)}`);}catch(e){error(e);}
  },true);
  window.addEventListener('blur',()=>{if(capturing)capture(false).catch(error);});
  for(const [id,key]of [['start-windows','startWithWindows'],['launch-tray','launchInTray']])$('#'+id).addEventListener('change',async e=>{try{await change({[key]:e.target.checked});}catch(e){paint();error(e);}});
  $('#accent-colour').addEventListener('input',e=>window.UmbraAppearance?.apply('umbra',e.target.value));
  $('#accent-colour').addEventListener('change',e=>change({assetColour:e.target.value}).catch(error));
  $('#reset-colour').addEventListener('click',()=>change({assetColour:'#ae1236'}).catch(error));
  $('#dismiss-error').addEventListener('click',()=>invoke('dismiss-error').then(v=>{state=v;paint();}).catch(error));
  await window.__TAURI__.event.listen('oracle-error',({payload})=>error(payload));
  // Serial polling avoids overlapping responses and never overwrites a note being edited.
  async function poll(){try{state=await window.__TAURI__.core.invoke('ready');paint();}catch(e){error(e);}finally{setTimeout(poll,1500);}}
  setTimeout(poll,1500);
}
init().catch(error);
