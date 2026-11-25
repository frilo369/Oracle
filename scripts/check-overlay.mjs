// Real Windows OCR + native click-through labels, isolated from installed Oracle/League.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createServer} from 'node:net';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url)),exe=resolve(root,process.argv[2]||'target/release/oracle.exe');
assert.ok(['target','.test'].some(dir=>exe.toLowerCase().startsWith(resolve(root,dir).toLowerCase()+'\\')));
const output=join(root,'.test','overlay-'+Date.now());await mkdir(output,{recursive:true});
const server=createServer();await new Promise(done=>server.listen(0,'127.0.0.1',done));const port=server.address().port;await new Promise(done=>server.close(done));
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
let child,browser,page,preview;const errors=[],observations=[];
async function run(file,args){const p=spawn(file,args,{cwd:root,env,windowsHide:true,stdio:['ignore','pipe','pipe']});let text='';p.stdout.on('data',b=>text+=b);p.stderr.on('data',b=>text+=b);await new Promise(done=>p.once('exit',done));assert.equal(p.exitCode,0,text);return text;}
async function until(fn,ms=20000){const end=Date.now()+ms;do{const v=await fn();if(v)return v;if(child.exitCode!==null)throw Error('Oracle exited');await delay(60);}while(Date.now()<end);throw Error('Timed out: '+fn.toString());}
async function forward(args=[]){await run(exe,args);}
const action=(kind,id=null)=>page.evaluate(({kind,id})=>window.__TAURI__.core.invoke('action',{kind,id}),{kind,id});
const ready=()=>page.evaluate(()=>window.__TAURI__.core.invoke('ready',{compact:true}));
async function shown(ids,width=null){return until(async()=>{let s;try{s=JSON.parse(await readFile(join(output,'overlay-state.json'),'utf8'));}catch{return false;}return s?.visible&&(!width||s.bounds?.width===width)&&JSON.stringify(s.labels.map(l=>l.id))===JSON.stringify(ids)?s:false;},60000);}
async function launch(){
  child=spawn(exe,['--fixture'],{cwd:root,env,windowsHide:true,stdio:'ignore'});await delay(800);await forward();
  await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));
  page.on('pageerror',e=>errors.push(e.message));await page.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
}
async function stop(){if(browser){await browser.close();browser=null;}if(child?.exitCode===null){await forward(['--quit']);if(child.exitCode===null)await new Promise(done=>child.once('exit',done));}}
const watchdog=setTimeout(()=>child?.kill(),180000);watchdog.unref();
try{
  const captureExe=join(output,'capture.exe');
  await run(join(process.env.WINDIR,'Microsoft.NET/Framework64/v4.0.30319/csc.exe'),['/nologo','/r:System.Drawing.dll',`/out:${captureExe}`,join(root,'tests/overlay-capture.cs')]);
  await launch();await until(async()=>['idle','setup'].includes((await ready()).overlay?.status));
  assert.equal((await ready()).settings.overlayEnabled,false,'experimental overlay defaults off until explicitly enabled');
  await page.locator('nav [data-tab="settings"]').click();await page.locator('#enable-overlay').check();await until(async()=>(await ready()).settings.overlayEnabled===true);
  await page.screenshot({path:join(output,'settings.png')});
  const start=Date.now();await page.locator('#preview-overlay').click();
  preview=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('/overlay-preview.html')));
  preview.on('pageerror',e=>errors.push(e.message));
  await preview.evaluate(()=>document.fonts.ready);await preview.screenshot({path:join(output,'preview-page.png')});await action('overlay-preview');
  assert.equal(await preview.evaluate(async()=>{try{await window.__TAURI__.core.invoke('ready');return true;}catch{return false;}}),false,'preview must not receive dashboard command access');
  const first=await shown([3055,1205,1415]);
  assert.deepEqual(first.labels.map(l=>l.text),['Pick 13.85%   |   Tier S','Pick 8.25%   |   Tier S','Pick 5.32%   |   Tier S']);
  const inspect=await action('fixture',{overlayInspect:true});
  console.log('Native overlay: '+JSON.stringify(inspect));
  for(const [key,value]of Object.entries({clickThrough:true,noActivate:true,hitOverlay:false,foregroundIsTarget:true,visible:true}))assert.equal(inspect[key],value,key);
  assert.equal(inspect.backgroundPixel,0x001a100f,'the native label background must actually be painted');
  console.log(await run(captureExe,[String(first.target),String(child.pid),join(output,'native-overlay.png'),String(first.hwnd)]));
  observations.push({name:'Actual OCR and native labels',milliseconds:Date.now()-start,inspect,labels:first.labels});
  const blur=Date.now();await forward();await until(async()=>(await ready()).overlay.visible===false);
  observations.push({name:'Focus change hides labels',milliseconds:Date.now()-blur});
  await action('overlay-preview');await shown([3055,1205,1415]);
  for(const size of [[800,600],[900,650],[1100,700]]){
    await action('fixture',{overlayPreviewSize:size});await preview.waitForFunction(([w,h])=>innerWidth>=w-30&&innerWidth<=w+1&&innerHeight>=h-60&&innerHeight<=h+1,size);
    const width=await preview.evaluate(()=>Math.round(innerWidth*devicePixelRatio));
    const resized=await shown([3055,1205,1415],width);
    assert.equal(await preview.evaluate(()=>document.documentElement.scrollHeight>innerHeight+1||document.documentElement.scrollWidth>innerWidth),false,'preview must fit its minimum size');
    const descriptions=await preview.locator('article p').evaluateAll(nodes=>nodes.map(n=>n.getBoundingClientRect().bottom*devicePixelRatio));
    assert.ok(resized.labels.every((l,i)=>l.y>descriptions[i]),'labels must remain beneath descriptions');
    assert.ok(resized.labels.every((l,i)=>Math.abs(l.x/width-[.183,.5,.817][i])<.05),'labels must follow resized card columns');
    assert.equal((await action('fixture',{overlayInspect:true})).foregroundIsTarget,true);
    await run(captureExe,[String(resized.target),String(child.pid),join(output,`native-overlay-${size.join('x')}.png`)]);
    observations.push({name:'Resize '+size.join('x'),labels:resized.labels});
  }
  const roll=Date.now();await preview.locator('.reroll').first().click();
  const rerolled=await shown([1105,1205,1415]);assert.equal(rerolled.labels[0].text,'Pick 4.85%   |   Tier S');
  observations.push({name:'Reroll',milliseconds:Date.now()-roll});
  const select=Date.now();await preview.locator('.pick').nth(1).click();await until(async()=>(await ready()).overlay.visible===false);
  observations.push({name:'Select hides labels',milliseconds:Date.now()-select});
  await action('fixture',{closeOverlayPreview:true});await forward();
  await until(async()=>(await ready()).overlay.target===null);
  // A validated QA profile must never capture real League, even with this setting on.
  await action('fixture',{target:{id:1,mode:'mayhem'}});await delay(800);assert.equal((await ready()).overlay.target,null);
  await stop();await launch();assert.equal((await ready()).settings.overlayEnabled,true,'setting survives restart');
  await until(async()=>['idle','setup'].includes((await ready()).overlay?.status));assert.equal((await ready()).overlay.target,null);
  await page.locator('nav [data-tab="settings"]').click();await page.locator('#enable-overlay').uncheck();await until(async()=>(await ready()).settings.overlayEnabled===false);
  assert.deepEqual(errors,[]);
  await writeFile(join(output,'verified.json'),JSON.stringify({observations,settingPersists:true,qaCaptureIsolated:true,errors},null,2));
  console.log('Overlay acceptance passed: '+output);
}catch(e){
  console.error('Evidence: '+output);if(page&&!page.isClosed())console.error(JSON.stringify((await ready()).overlay));
  try{console.error((await readFile(join(output,'overlay-ocr.json'),'utf8')).slice(0,6000));}catch{}
  throw e;
}finally{clearTimeout(watchdog);await stop();}
