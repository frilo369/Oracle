// Disposable native WebView2 acceptance. Never reads or changes an installed Oracle profile.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createServer} from 'node:net';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url));
const executable=resolve(root,process.argv[2]||'target/release/oracle.exe');
assert.ok(executable.toLowerCase().startsWith(resolve(root,'target').toLowerCase()+'\\')||executable.toLowerCase().startsWith(resolve(root,'.test').toLowerCase()+'\\'));
const output=join(root,'.test','native-'+Date.now());await mkdir(output,{recursive:true});
const helperExe=join(output,'game-fixture.exe');
await new Promise((done,fail)=>{const p=spawn(join(process.env.WINDIR,'Microsoft.NET/Framework64/v4.0.30319/csc.exe'),['/nologo','/target:exe','/r:System.Windows.Forms.dll','/r:System.Drawing.dll','/out:'+helperExe,join(root,'tests/guide-borderless.cs')],{windowsHide:true,stdio:'inherit'});p.on('error',fail);p.on('exit',code=>code===0?done():fail(Error('Helper compile failed')));});
let port;
let child,browser,helper;
const env={...process.env,ORACLE_DATA_DIR:output};
const observations=[];
async function until(fn,ms=20000){const end=Date.now()+ms;do{const value=await fn();if(value)return value;if(child?.exitCode!==null)throw Error('Oracle exited '+child.exitCode);await delay(80);}while(Date.now()<end);throw Error('Timed out: '+fn.toString().slice(0,180));}
async function launch(quiet=false){const server=createServer();await new Promise(done=>server.listen(0,'127.0.0.1',done));port=server.address().port;await new Promise(done=>server.close(done));env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=`--remote-debugging-port=${port}`;child=spawn(executable,['--fixture',...(quiet?['--tray']:[])],{cwd:root,env,windowsHide:true,stdio:'ignore'});if(quiet){await delay(750);const forward=spawn(executable,[],{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>forward.once('exit',done));assert.equal(forward.exitCode,0);}await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);return until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')&&!p.url().includes('/guide/')));}
let dashboard;
const action=(kind,id=null)=>dashboard.evaluate(({kind,id})=>window.__TAURI__.core.invoke('action',{kind,id}),{kind,id});
const status=()=>action('fixture',{});
const replies=[];
async function input(line){helper.stdin.write(line+'\n');return until(()=>replies.shift());}
const watchdog=setTimeout(()=>{child?.kill();helper?.kill();},180000);watchdog.unref();
try {
  dashboard=await launch(true);await dashboard.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  const errors=[];dashboard.on('pageerror',e=>errors.push(e.message));
  await dashboard.screenshot({path:join(output,'overview.png')});
  for(const tab of ['library','notebook','settings']){await dashboard.locator(`[data-tab="${tab}"]`).click();await dashboard.locator('#'+tab).waitFor({state:'visible'});await dashboard.screenshot({path:join(output,tab+'.png')});assert.equal(await dashboard.locator('#'+tab).evaluate(el=>el.scrollWidth<=el.clientWidth+2),true,tab+' must not overflow horizontally');}
  await dashboard.locator('[data-tab="library"]').click();await dashboard.locator('#champion-search').fill('Ahri');assert.equal(await dashboard.locator('.champion-card').count(),1);await dashboard.locator('.champion-card').click();await dashboard.locator('#favorite').click();await dashboard.waitForFunction(()=>document.querySelector('#favorite').textContent==='★');
  await dashboard.locator('[data-tab="notebook"]').click();await dashboard.locator('#note-text').fill('Respect the matchup. Çampion 🤍');await dashboard.locator('#save-note').click();await dashboard.waitForFunction(()=>document.querySelector('#note-state').textContent==='Saved on this PC');
  await dashboard.locator('[data-tab="settings"]').click();await dashboard.locator('#record-key').click();await dashboard.keyboard.press('Control+g');await dashboard.waitForFunction(()=>document.querySelector('#record-key').textContent.includes('Ctrl + G'));
  assert.equal((JSON.parse(await readFile(join(output,'settings.json'),'utf8'))).guideHotkey,'Ctrl+KeyG');
  await dashboard.locator('#reset-key').click();await dashboard.waitForFunction(()=>document.querySelector('#record-key').textContent.includes('F1'));
  helper=spawn(helperExe,[],{windowsHide:true,stdio:['pipe','pipe','inherit']});let pending='';helper.stdout.on('data',bytes=>{pending+=bytes;let n;while((n=pending.indexOf('\n'))>=0){replies.push(pending.slice(0,n).trim());pending=pending.slice(n+1);}});await until(()=>replies.shift()==='ready');
  // Reserve F1 in another process before Oracle returns to the game, to test its fallback.
  const bringDashboard=spawn(executable,[],{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>bringDashboard.once('exit',done));await delay(650);
  assert.equal(await input('reserve'),'reserved:True');
  assert.equal(await input('focus'),'focused:True');await delay(650);await input('f1');await until(async()=>(await status()).foreground);
  const page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('/guide/mayhem.html')));page.on('pageerror',e=>errors.push(e.message));await page.waitForFunction(()=>document.querySelectorAll('.augment-row').length>0);await page.screenshot({path:join(output,'mayhem.png')});
  observations.push({scenario:'native F1 with another process holding the shortcut',...await status()});assert.equal((await status()).maximized,true);assert.equal((await status()).fullscreen,false);
  assert.equal(JSON.parse(await readFile(join(output,'guide-shortcut.json'),'utf8')).registered,false,'global registration must conflict in this scenario');await input('release');
  await page.evaluate(()=>window.__oracleMarker='preserved');await input('focus');await delay(650);await input('f1');await until(async()=>(await status()).foreground);assert.equal(await page.evaluate(()=>window.__oracleMarker),'preserved');
  await action('fixture',{close:true});await until(async()=>!(await status()).visible);assert.equal(await input('front'),'focused:True');
  await input('focus');await delay(650);await input('shift-f1');await delay(500);assert.equal((await status()).visible,false,'extra modifier cannot trigger F1');
  await input('f1');await until(async()=>(await status()).foreground);assert.equal(await page.evaluate(()=>window.__oracleMarker),'preserved','closing keeps the page');
  await action('fixture',{minimize:true});await input('focus');await delay(650);await input('f1');await until(async()=>(await status()).foreground);assert.equal(await page.evaluate(()=>window.__oracleMarker),'preserved');
  observations.push({scenario:'refocus, close/reopen, minimize/restore without reload',...await status()});
  await action('save-settings',{guideHotkey:'Ctrl+KeyG'});await action('fixture',{close:true});await input('focus');await delay(650);await input('ctrl-g');await until(async()=>(await status()).foreground);observations.push({scenario:'native Ctrl+G',...await status()});
  await action('capture',true);await action('fixture',{close:true});await input('focus');await delay(650);await input('ctrl-g');await delay(600);assert.equal((await status()).visible,false,'capture suppresses the guide');await action('capture',false);
  await action('fixture',{target:null});await input('focus');await delay(650);await input('ctrl-g');await delay(600);assert.equal((await status()).visible,false,'match end suppresses the guide');
  // Restart against exactly the same disposable profile and inspect persistence.
  helper.stdin.write('quit\n');await new Promise(done=>helper.once('exit',done));
  await browser.close();browser=null;
  const stop=spawn(executable,['--quit'],{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>stop.once('exit',done));if(child.exitCode===null)await new Promise(done=>child.once('exit',done));
  dashboard=await launch();await dashboard.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  const saved=await dashboard.evaluate(()=>window.__TAURI__.core.invoke('ready'));
  assert.equal(saved.settings.guideHotkey,'Ctrl+KeyG');assert.equal(saved.settings.notes['103'],'Respect the matchup. Çampion 🤍');assert.ok(saved.settings.favorites.includes(103));
  await action('hide');await delay(400);assert.equal(child.exitCode,null,'closing the dashboard must retain the process');
  const second=spawn(executable,[],{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>second.once('exit',done));assert.equal(second.exitCode,0,'second instance forwards to the first');
  assert.deepEqual(errors,[]);
  await writeFile(join(output,'verified.json'),JSON.stringify({observations,notebook:true,favorites:true,customShortcut:true,persistence:true,trayPersistence:true,singleInstance:true,captureSuppressed:true,matchEndSuppressed:true,errors},null,2));
  console.log('Native acceptance passed: '+output);
}finally{clearTimeout(watchdog);if(browser)await browser.close();if(helper?.exitCode===null)helper.kill();if(child?.exitCode===null){const quit=spawn(executable,['--quit'],{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>quit.once('exit',done));if(child.exitCode===null)child.kill();}}
