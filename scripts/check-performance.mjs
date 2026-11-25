// Real WebView2 performance acceptance, using a disposable fixture profile only.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createServer} from 'node:net';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url));
const executable=resolve(root,process.argv[2]||'target/release/oracle.exe');
assert.ok(['target','.test'].some(dir=>executable.toLowerCase().startsWith(resolve(root,dir).toLowerCase()+'\\')));
const output=join(root,'.test','performance-'+Date.now());await mkdir(output,{recursive:true});
const server=createServer();await new Promise(done=>server.listen(0,'127.0.0.1',done));const port=server.address().port;await new Promise(done=>server.close(done));
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
let child,browser,dashboard;const errors=[];
async function until(fn,ms=20000){const end=Date.now()+ms;do{const value=await fn();if(value)return value;if(child?.exitCode!==null)throw Error('Oracle exited');await delay(60);}while(Date.now()<end);throw Error('Timed out: '+fn.toString());}
async function forward(args=[]){const p=spawn(executable,args,{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>p.once('exit',done));assert.equal(p.exitCode,0);}
const action=(kind,id=null)=>dashboard.evaluate(({kind,id})=>window.__TAURI__.core.invoke('action',{kind,id}),{kind,id});
const visibility=()=>dashboard.evaluate(async()=>({visible:await window.__TAURI__.window.getCurrentWindow().isVisible(),minimized:await window.__TAURI__.window.getCurrentWindow().isMinimized(),hidden:document.hidden,toast:document.querySelector('#toast').textContent}));
const watchdog=setTimeout(()=>child?.kill(),90000);watchdog.unref();
try{
  child=spawn(executable,['--fixture'],{cwd:root,env,windowsHide:true,stdio:'ignore'});
  await delay(800);await forward();
  await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  dashboard=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));
  dashboard.on('pageerror',e=>errors.push(e.message));
  await dashboard.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  await until(async()=>{const v=await visibility();return v.visible&&!v.minimized;});
  console.log('Initial visibility: '+JSON.stringify(await visibility()));
  const sizes=await dashboard.evaluate(async()=>{
    const full=await window.__TAURI__.core.invoke('ready'),compact=await window.__TAURI__.core.invoke('ready',{compact:true});
    return {fullBytes:new TextEncoder().encode(JSON.stringify(full)).length,compactBytes:new TextEncoder().encode(JSON.stringify(compact)).length,hasCatalogue:Object.hasOwn(compact,'champions')};
  });
  assert.equal(sizes.hasCatalogue,false);assert.ok(sizes.compactBytes<sizes.fullBytes/5);
  await delay(500);
  await dashboard.evaluate(()=>{
    window.__audit={polls:0,mutations:0,colours:0};const invoke=window.__TAURI__.core.invoke;
    window.__TAURI__.core={...window.__TAURI__.core,invoke:(command,args)=>{if(command==='ready')window.__audit.polls++;return invoke(command,args);}};
    window.__auditObserver=new MutationObserver(records=>window.__audit.mutations+=records.length);
    window.__auditObserver.observe(document.body,{subtree:true,attributes:true,childList:true,characterData:true});
    window.addEventListener('umbra-colour',()=>window.__audit.colours++);
  });
  await delay(2600);const idle=await dashboard.evaluate(()=>({...window.__audit}));
  assert.ok(idle.polls>0,'visible refresh: '+JSON.stringify({idle,visibility:await visibility()}));assert.equal(idle.mutations,0);assert.equal(idle.colours,0);
  await action('hide');await delay(400);await dashboard.evaluate(()=>window.__audit.polls=0);await delay(2600);
  const hiddenPolls=await dashboard.evaluate(()=>window.__audit.polls);assert.equal(hiddenPolls,0);
  await action('fixture',{target:{id:103,mode:'mayhem'}});const resumeAt=Date.now();await forward();
  await dashboard.waitForFunction(()=>document.querySelector('#match-champion').textContent==='Ahri');const resumeMs=Date.now()-resumeAt;
  await dashboard.evaluate(()=>window.__auditObserver.disconnect());
  await action('minimize');await delay(400);await dashboard.evaluate(()=>window.__audit.polls=0);await delay(2600);
  const minimizedPolls=await dashboard.evaluate(()=>window.__audit.polls);assert.equal(minimizedPolls,0);
  await action('fixture',{target:{id:1,mode:'mayhem'}});await forward();
  await dashboard.waitForFunction(()=>document.querySelector('#match-champion').textContent==='Annie');
  await action('open-guide',{champion:1,mode:'mayhem'});
  const page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('/guide/mayhem.html')));
  page.on('pageerror',e=>errors.push(e.message));await page.waitForFunction(()=>document.querySelectorAll('.augment-row').length>0&&!document.querySelector('#refresh').disabled);
  const search=await page.evaluate(()=>{
    const rows=[...document.querySelectorAll('.augment-row')],input=document.querySelector('#augment-search'),samples=[];
    for(let i=0;i<40;i++){input.value=i%2?'':'no-such-augment-xyz';const start=performance.now();input.dispatchEvent(new Event('input'));samples.push(performance.now()-start);}
    input.value='no-such-augment-xyz';input.dispatchEvent(new Event('input'));const filtered=rows.every(row=>row.hidden);
    input.value='';input.dispatchEvent(new Event('input'));
    return {rows:rows.length,filtered,restored:rows.every(row=>!row.hidden),sameNodes:rows.every((row,i)=>row===document.querySelectorAll('.augment-row')[i]),medianMs:samples.sort((a,b)=>a-b)[20]};
  });
  assert.ok(search.filtered&&search.restored&&search.sameNodes);assert.deepEqual(errors,[]);
  await page.screenshot({path:join(output,'mayhem.png')});
  await action('save-settings',{assetColour:'#2486ac'});
  await page.waitForFunction(()=>document.documentElement.style.getPropertyValue('--accent')==='#2486ac');
  const theme=await page.evaluate(()=>{
    let changes=0;window.addEventListener('umbra-colour',()=>changes++);
    window.UmbraAppearance.apply('umbra','#2486ac');window.UmbraAppearance.apply('umbra','#2486ac');
    return {changes,accent:document.documentElement.style.getPropertyValue('--accent'),silver:document.documentElement.style.getPropertyValue('--silver')};
  });
  assert.equal(theme.changes,0);assert.equal(theme.accent,'#2486ac');assert.equal(theme.silver,'#a7cfde');
  const result={...sizes,idle,hiddenPolls,minimizedPolls,resumeMs,search,theme,errors};await writeFile(join(output,'verified.json'),JSON.stringify(result,null,2));
  console.log(JSON.stringify(result,null,2));console.log('Performance acceptance passed: '+output);
}finally{clearTimeout(watchdog);if(browser)await browser.close();if(child?.exitCode===null){await forward(['--quit']);if(child.exitCode===null)await new Promise(done=>child.once('exit',done));}}
