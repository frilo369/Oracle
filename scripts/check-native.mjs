// Native acceptance against a disposable profile and an owned game stand-in.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile,access,stat} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createServer} from 'node:net';
import {chromium} from '../dependencies/playwright-core/index.mjs';
import {parseMayhemBuild} from '../src/ui/guide/mayhem-data.mjs';
const root=fileURLToPath(new URL('../',import.meta.url));
const executable=resolve(root,process.argv[2]||'target/release/oracle.exe');
assert.ok(['target','.test'].some(dir=>executable.toLowerCase().startsWith(resolve(root,dir).toLowerCase()+'\\')));
const output=join(root,'.test','native-'+Date.now());await mkdir(output,{recursive:true});
const helperExe=join(output,'game-fixture.exe');
await new Promise((done,fail)=>{
  const p=spawn(join(process.env.WINDIR,'Microsoft.NET/Framework64/v4.0.30319/csc.exe'),['/nologo','/target:exe','/r:System.Windows.Forms.dll','/r:System.Drawing.dll','/out:'+helperExe,join(root,'tests/guide-borderless.cs')],{windowsHide:true,stdio:'inherit'});
  p.on('error',fail);p.on('exit',code=>code===0?done():fail(Error('Helper compile failed')));
});
const env={...process.env,ORACLE_DATA_DIR:output};
let child,browser,dashboard,helper,firstLaunch=true;
const observations=[],errors=[],replies=[];
async function until(fn,ms=20000){
  const end=Date.now()+ms;
  do{const value=await fn();if(value)return value;if(child&&child.exitCode!==null)throw Error('Oracle exited '+child.exitCode);await delay(60);}while(Date.now()<end);
  throw Error('Timed out: '+fn.toString().slice(0,180));
}
async function forward(args=[]){const p=spawn(executable,args,{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>p.once('exit',done));assert.equal(p.exitCode,0);}
async function launch(){
  const server=createServer();await new Promise(done=>server.listen(0,'127.0.0.1',done));const port=server.address().port;await new Promise(done=>server.close(done));
  env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=`--remote-debugging-port=${port}`;
  child=spawn(executable,['--fixture'],{cwd:root,env,windowsHide:true,stdio:'ignore'});
  await delay(850);
  if(firstLaunch){assert.equal(await access(join(output,'Dashboard-WebView2')).then(()=>true,()=>false),false,'new profiles must launch directly in the tray');await forward(['--tray']);await delay(250);assert.equal(await access(join(output,'Dashboard-WebView2')).then(()=>true,()=>false),false,'a second quiet launch must stay in the tray');firstLaunch=false;}
  await forward();
  await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  dashboard=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')&&!p.url().includes('/guide/')));
  dashboard.on('pageerror',e=>errors.push(e.message));
  await dashboard.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
}
const action=(kind,id=null)=>dashboard.evaluate(({kind,id})=>window.__TAURI__.core.invoke('action',{kind,id}),{kind,id});
const ready=()=>dashboard.evaluate(()=>window.__TAURI__.core.invoke('ready'));
const status=()=>action('fixture',{});
async function input(line){helper.stdin.write(line+'\n');return until(()=>replies.shift());}
async function openWithKey(key,scenario){const started=Date.now();assert.equal(await input(key),'pressed','the fixture must own the foreground before sending its key');await until(async()=>(await status()).foreground);await until(async()=>{try{return (await stat(join(output,'guide-shown.json'))).mtimeMs>=started;}catch{return false;}});observations.push({scenario,milliseconds:Date.now()-started,...await status()});}
async function stop(){if(browser){await browser.close();browser=null;}if(child?.exitCode===null){await forward(['--quit']);if(child.exitCode===null)await new Promise(done=>child.once('exit',done));}}
const watchdog=setTimeout(()=>{child?.kill();helper?.kill();},180000);watchdog.unref();
try {
  await launch();
  assert.equal((await ready()).settings.launchInTray,true);
  assert.equal(await dashboard.locator('[data-tab="notebook"],[data-tab="library"],#notebook,#library').count(),0);
  for(const tab of ['overview','settings']){
    await dashboard.locator(`nav [data-tab="${tab}"]`).click();
    assert.deepEqual(await dashboard.evaluate(()=>({x:document.documentElement.scrollWidth>innerWidth,y:document.querySelector('main').scrollHeight>document.querySelector('main').clientHeight+2})),{x:false,y:false},'compact UI should fit without scrolling');
    await dashboard.screenshot({path:join(output,tab+'.png')});
  }
  assert.equal(await dashboard.locator('#launch-tray').isChecked(),true);
  await dashboard.locator('#launch-tray').uncheck();await until(async()=>(await ready()).settings.launchInTray===false);
  await dashboard.locator('#launch-tray').check();await until(async()=>(await ready()).settings.launchInTray===true);
  await dashboard.locator('nav [data-tab="overview"]').click();
  // Reproduce the native-response boundary: cancellation used to be dropped here.
  await dashboard.evaluate(()=>{
    const original=window.__TAURI__.core.invoke;
    window.__auditOriginalCore=window.__TAURI__.core;
    window.__TAURI__.core={...window.__TAURI__.core,invoke:async(command,args)=>{
      if(command==='action'&&args.kind==='capture'&&args.id===true){window.__auditCaptureStarted=true;await new Promise(resolve=>window.__auditReleaseCapture=resolve);}
      return original(command,args);
    }};
  });
  await dashboard.locator('#record-key').click();await dashboard.waitForFunction(()=>window.__auditCaptureStarted);
  await dashboard.locator('nav [data-tab="settings"]').click();
  await dashboard.evaluate(()=>window.__auditReleaseCapture());
  await dashboard.waitForFunction(()=>document.querySelector('#record-key').getAttribute('aria-busy')==='false');
  assert.equal((await ready()).guideCapture,false,'tab changes must cancel pending native capture');
  await dashboard.evaluate(()=>window.__TAURI__.core=window.__auditOriginalCore);
  await dashboard.locator('nav [data-tab="overview"]').click();
  await dashboard.locator('#desk-champion').selectOption('1');await dashboard.locator('#desk-mode').selectOption('mayhem');
  await until(async()=>(await ready()).settings.lastGuide?.mode==='mayhem');
  // A manual-open click must retain its champion even if the picker changes during saving.
  await dashboard.evaluate(()=>{
    const original=window.__TAURI__.core.invoke;window.__auditOriginalCore=window.__TAURI__.core;let held=false;
    window.__TAURI__.core={...window.__TAURI__.core,invoke:async(command,args)=>{
      if(command==='action'&&args.kind==='save-settings'&&args.id.lastGuide&&!held){held=true;window.__auditSaveStarted=true;await new Promise(resolve=>window.__auditReleaseSave=resolve);}
      if(command==='action'&&args.kind==='open-guide'){window.__auditOpened=args.id;return original('ready');}
      return original(command,args);
    }};
  });
  await dashboard.locator('#open-guide').click();await dashboard.waitForFunction(()=>window.__auditSaveStarted);
  await dashboard.locator('#desk-champion').selectOption('103');
  await dashboard.evaluate(()=>window.__auditReleaseSave());await dashboard.waitForFunction(()=>!!window.__auditOpened);
  assert.deepEqual(await dashboard.evaluate(()=>window.__auditOpened),{champion:1,mode:'mayhem'});
  await until(async()=>(await ready()).settings.lastGuide.champion===103);
  await dashboard.evaluate(()=>window.__TAURI__.core=window.__auditOriginalCore);
  await dashboard.locator('#desk-champion').selectOption('1');await until(async()=>(await ready()).settings.lastGuide.champion===1);
  await dashboard.locator('#record-key').click();await dashboard.waitForFunction(()=>document.querySelector('#record-key').getAttribute('aria-busy')==='false');await dashboard.keyboard.press('Control+g');
  await dashboard.waitForFunction(()=>document.querySelector('#record-key').textContent==='Ctrl + G');
  await dashboard.locator('#reset-key').click();await dashboard.waitForFunction(()=>document.querySelector('#record-key').textContent==='F1');
  await action('capture',true);await writeFile(join(output,'guide-used'),'');await delay(400);
  assert.equal(browser.contexts().flatMap(c=>c.pages()).some(p=>p.url().includes('/guide/mayhem.html')),false,'capture suppresses hidden preparation');
  const preparedAt=Date.now();await action('capture',false);
  await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('/guide/mayhem.html')));
  assert.equal((await status()).visible,false,'background preparation must remain hidden');
  observations.push({scenario:'Hidden preparation retries after shortcut capture ends',milliseconds:Date.now()-preparedAt});
  helper=spawn(helperExe,[],{windowsHide:true,stdio:['pipe','pipe','inherit']});let pending='';
  helper.stdout.on('data',bytes=>{pending+=bytes;let n;while((n=pending.indexOf('\n'))>=0){replies.push(pending.slice(0,n).trim());pending=pending.slice(n+1);}});
  await until(()=>replies.shift()==='ready');
  await action('capture',true);await delay(400);assert.equal(await input('reserve'),'reserved:True');await action('capture',false);await delay(500);
  assert.equal(await input('focus'),'focused:True');await delay(350);await openWithKey('f1','Prepared F1 with a global shortcut conflict');
  assert.equal(JSON.parse(await readFile(join(output,'guide-shortcut.json'),'utf8')).registered,false);
  const page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('/guide/mayhem.html')));
  page.on('pageerror',e=>errors.push(e.message));await page.waitForFunction(()=>document.querySelectorAll('.augment-row').length>0);
  const parsed=parseMayhemBuild(JSON.parse(await readFile(join(root,'tests/fixtures/mayhem-annie.json'),'utf8')),1);
  assert.equal(await page.locator('#skill-select').inputValue(),parsed.skillOrders[0].priority.join('-'));
  assert.ok((await page.locator('#skill-select option').allTextContents())[0].startsWith('Best ·'));
  const searchPerformance=await page.evaluate(()=>{
    const rows=[...document.querySelectorAll('.augment-row')],input=document.querySelector('#augment-search');
    const samples=[];
    for(let i=0;i<40;i++){input.value=i%2?'':'no-such-augment-xyz';const start=performance.now();input.dispatchEvent(new Event('input'));samples.push(performance.now()-start);}
    input.value='no-such-augment-xyz';input.dispatchEvent(new Event('input'));
    const filtered=rows.every(row=>row.hidden);
    input.value='';input.dispatchEvent(new Event('input'));
    return {rows:rows.length,filtered,restored:rows.every(row=>!row.hidden),sameNodes:rows.every((row,i)=>row===document.querySelectorAll('.augment-row')[i]),medianMs:samples.sort((a,b)=>a-b)[Math.floor(samples.length/2)]};
  });
  assert.equal(searchPerformance.filtered,true);assert.equal(searchPerformance.restored,true);assert.equal(searchPerformance.sameNodes,true,'typing must reuse the loaded artwork nodes');
  observations.push({scenario:'Mayhem search without rebuilding artwork',...searchPerformance});
  await page.screenshot({path:join(output,'mayhem.png')});
  await input('release');
  assert.equal((await status()).maximized,true);assert.equal((await status()).fullscreen,false);
  await page.evaluate(()=>window.__oracleMarker='preserved');
  await input('focus');await delay(400);await openWithKey('f1','Focus open guide without reloading');assert.equal(await page.evaluate(()=>window.__oracleMarker),'preserved');
  await action('fixture',{close:true});await until(async()=>!(await status()).visible);await delay(150);assert.equal(await input('front'),'focused:True',JSON.stringify(await status()));
  await input('shift-f1');await delay(500);assert.equal((await status()).visible,false,'extra modifiers cannot trigger F1');
  await openWithKey('f1','Reopen hidden guide without reloading');assert.equal(await page.evaluate(()=>window.__oracleMarker),'preserved');
  await action('fixture',{minimize:true});await input('focus');await delay(400);await openWithKey('f1','Restore minimized guide');assert.equal(await page.evaluate(()=>window.__oracleMarker),'preserved');
  // Fullscreen-like loss of activation: the game minimizes when another window takes focus.
  await action('fixture',{close:true});await until(async()=>!(await status()).visible);await input('focus');await delay(400);await input('fullscreen');
  const activationAt=Date.now();await action('fixture',{activation:true});await until(async()=>(await status()).foreground);
  assert.equal(JSON.parse(await readFile(join(output,'guide-activation.json'),'utf8')).foreground,true);
  assert.equal(await input('state'),'minimized:True');assert.equal(await page.evaluate(()=>window.__oracleMarker),'preserved');
  observations.push({scenario:'Own-process Windows activation helper after fullscreen-like minimization',milliseconds:Date.now()-activationAt,...await status()});
  await action('fixture',{close:true});await until(async()=>!(await status()).visible);await delay(200);assert.equal(await input('front'),'focused:True');assert.equal(await input('state'),'minimized:False');
  await delay(400);await openWithKey('f1','Fullscreen-like F1 and game restoration');await action('fixture',{close:true});await until(async()=>!(await status()).visible);await delay(200);assert.equal(await input('front'),'focused:True');
  await action('save-settings',{guideHotkey:'Ctrl+KeyG'});await delay(400);await input('focus');await openWithKey('ctrl-g','Native custom Ctrl+G');
  await action('capture',true);await action('fixture',{close:true});await input('focus');await delay(400);await input('ctrl-g');await delay(500);assert.equal((await status()).visible,false);await action('capture',false);
  await action('fixture',{target:null});await delay(400);await input('ctrl-g');await delay(500);assert.equal((await status()).visible,false);
  helper.stdin.write('quit\n');await new Promise(done=>helper.once('exit',done));await stop();
  await launch();const saved=await ready();assert.equal(saved.settings.guideHotkey,'Ctrl+KeyG');assert.deepEqual(saved.settings.lastGuide,{champion:1,mode:'mayhem'});assert.equal(saved.settings.launchInTray,true);
  assert.equal(await dashboard.locator('#desk-champion').inputValue(),'1');assert.equal(await dashboard.locator('#desk-mode').inputValue(),'mayhem');
  const compact=await dashboard.evaluate(()=>window.__TAURI__.core.invoke('ready',{compact:true}));
  const fullBytes=Buffer.byteLength(JSON.stringify(await ready())),compactBytes=Buffer.byteLength(JSON.stringify(compact));
  assert.equal(Object.hasOwn(compact,'champions'),false);assert.ok(compactBytes<fullBytes/5);
  await forward();
  await until(()=>dashboard.evaluate(()=>window.__TAURI__.window.getCurrentWindow().isVisible()));
  await delay(500);
  await dashboard.evaluate(()=>{
    window.__auditIdle={polls:0,mutations:0,colours:0};window.__auditOriginalCore=window.__TAURI__.core;
    const invoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core={...window.__TAURI__.core,invoke:(command,args)=>{if(command==='ready')window.__auditIdle.polls++;return invoke(command,args);}};
    window.__auditObserver=new MutationObserver(records=>window.__auditIdle.mutations+=records.length);
    window.__auditObserver.observe(document.body,{subtree:true,attributes:true,childList:true,characterData:true});
    window.addEventListener('umbra-colour',()=>window.__auditIdle.colours++);
  });
  await delay(2600);
  const idle=await dashboard.evaluate(async()=>({...window.__auditIdle,visible:await window.__TAURI__.window.getCurrentWindow().isVisible(),hidden:document.hidden}));
  assert.ok(idle.polls>0,'visible dashboard still refreshes: '+JSON.stringify(idle));assert.equal(idle.mutations,0,'unchanged state must not repaint');assert.equal(idle.colours,0,'unchanged state must not repaint the theme');
  await action('hide');await delay(400);assert.equal(child.exitCode,null);
  await dashboard.evaluate(()=>window.__auditIdle.polls=0);await delay(2600);
  const hiddenPolls=await dashboard.evaluate(()=>window.__auditIdle.polls);assert.equal(hiddenPolls,0,'tray-only dashboard must not poll');
  await action('fixture',{target:null});await forward();await dashboard.waitForFunction(()=>document.querySelector('#match-champion').textContent==='No active match');
  await dashboard.evaluate(()=>{window.__auditObserver.disconnect();window.__TAURI__.core=window.__auditOriginalCore;});
  observations.push({scenario:'Dashboard idle and tray work',fullBytes,compactBytes,...idle,hiddenPolls});
  assert.deepEqual(errors,[]);
  await writeFile(join(output,'verified.json'),JSON.stringify({observations,pendingCaptureCancelled:true,manualClickTargetPreserved:true,compactUI:true,removedPages:true,defaultTray:true,skillRanking:true,activationHelper:true,fullscreenLikeReturn:true,persistence:true,trayPersistence:true,singleInstance:true,captureSuppressed:true,matchEndSuppressed:true,errors},null,2));
  console.log('Native acceptance passed: '+output);
}finally{clearTimeout(watchdog);if(helper?.exitCode===null)helper.kill();await stop();}
