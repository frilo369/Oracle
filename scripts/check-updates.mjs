// Startup/manual updater acceptance in the actual WebView; no real installation or network.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile,access} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createServer} from 'node:net';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url)),exe=resolve(root,process.argv[2]||'target/release/oracle.exe');
assert.ok(['target','.test'].some(dir=>exe.toLowerCase().startsWith(resolve(root,dir).toLowerCase()+'\\')));
const output=join(root,'.test','updates-'+Date.now());await mkdir(output,{recursive:true});
const server=createServer();await new Promise(done=>server.listen(0,'127.0.0.1',done));const port=server.address().port;await new Promise(done=>server.close(done));
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
let child,browser,page;const errors=[];
async function until(fn,ms=20000){const end=Date.now()+ms;do{const v=await fn();if(v)return v;if(child.exitCode!==null)throw Error('Oracle exited');await delay(60);}while(Date.now()<end);throw Error('Timed out: '+fn.toString());}
async function forward(args=[]){const p=spawn(exe,args,{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>p.once('exit',done));assert.equal(p.exitCode,0);}
const action=(kind,id=null)=>page.evaluate(({kind,id})=>window.__TAURI__.core.invoke('action',{kind,id}),{kind,id});
const update=()=>page.evaluate(async()=>(await window.__TAURI__.core.invoke('ready',{compact:true})).update);
const watchdog=setTimeout(()=>child?.kill(),90000);watchdog.unref();
try{
  child=spawn(exe,['--fixture','--fixture-update-startup'],{cwd:root,env,windowsHide:true,stdio:'ignore'});
  await delay(800);await forward();
  await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));page.on('pageerror',e=>errors.push(e.message));
  await page.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  await until(async()=>(await update()).status==='current'&&!(await update()).busy);
  assert.equal(JSON.parse(await readFile(join(output,'update-startup.json'),'utf8')).checked,true);
  await page.locator('nav [data-tab="settings"]').click();
  assert.equal(await page.evaluate(()=>document.querySelector('main').scrollHeight>document.querySelector('main').clientHeight+2),false,'settings including Updates must fit the compact window');
  await page.screenshot({path:join(output,'settings.png')});
  await action('fixture',{updateScenario:'offline'});await page.locator('#check-updates').click();
  await until(async()=>(await update()).status==='error'&&!(await update()).busy);
  await page.waitForFunction(()=>!document.querySelector('#check-updates').disabled&&document.querySelector('#update-status').textContent.includes('Could not reach GitHub'));
  await action('fixture',{updateScenario:'current'});await page.locator('#check-updates').click();
  await until(async()=>(await update()).status==='current'&&!(await update()).busy);
  await action('capture',true);await action('fixture',{updateScenario:'available'});await page.locator('#check-updates').click();
  await until(async()=>(await update()).status==='waiting');
  assert.equal(await page.locator('#check-updates').isDisabled(),true);
  await action('check-updates');await delay(600);
  assert.equal(await access(join(output,'update-install.json')).then(()=>true,()=>false),false,'a match must defer installation');
  await action('fixture',{target:null});await delay(600);
  assert.equal((await update()).status,'waiting','capture must also defer installation');
  await action('capture',false);await until(async()=>(await update()).status==='ready'&&!(await update()).busy);
  const handoff=JSON.parse(await readFile(join(output,'update-install.json'),'utf8'));assert.equal(handoff.verified,true);assert.equal(handoff.fixture,true);
  await action('fixture',{updateScenario:'corrupt'});await page.locator('#check-updates').click();
  await until(async()=>(await update()).status==='error'&&!(await update()).busy);
  const events=(await readFile(join(output,'guide-events.jsonl'),'utf8')).trim().split('\n').map(JSON.parse);
  assert.equal(events.filter(e=>e.event==='update-install').length,1,'duplicate checks and corrupted downloads cannot launch another installer');
  await page.waitForFunction(()=>document.querySelector('#update-status').textContent.includes('verification failed'));
  assert.deepEqual(errors,[]);
  await writeFile(join(output,'verified.json'),JSON.stringify({startup:true,settingsFit:true,manualCheck:true,offlineRecovery:true,matchDeferred:true,captureDeferred:true,duplicateSuppressed:true,digestVerified:true,corruptRejected:true,handoff,errors},null,2));
  console.log('Update acceptance passed: '+output);
}finally{clearTimeout(watchdog);if(browser)await browser.close();if(child?.exitCode===null){await forward(['--quit']);if(child.exitCode===null)await new Promise(done=>child.once('exit',done));}}
