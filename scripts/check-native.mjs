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
  if(firstLaunch){assert.equal(await access(join(output,'Dashboard-WebView2')).then(()=>true,()=>false),false,'new profiles must launch directly in the tray');firstLaunch=false;}
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
  await dashboard.locator('#desk-champion').selectOption('1');await dashboard.locator('#desk-mode').selectOption('mayhem');
  await until(async()=>(await ready()).settings.lastGuide?.mode==='mayhem');
  await dashboard.locator('#record-key').click();await dashboard.keyboard.press('Control+g');
  await dashboard.waitForFunction(()=>document.querySelector('#record-key').textContent==='Ctrl + G');
  await dashboard.locator('#reset-key').click();await dashboard.waitForFunction(()=>document.querySelector('#record-key').textContent==='F1');
  helper=spawn(helperExe,[],{windowsHide:true,stdio:['pipe','pipe','inherit']});let pending='';
  helper.stdout.on('data',bytes=>{pending+=bytes;let n;while((n=pending.indexOf('\n'))>=0){replies.push(pending.slice(0,n).trim());pending=pending.slice(n+1);}});
  await until(()=>replies.shift()==='ready');
  await action('capture',true);await delay(400);assert.equal(await input('reserve'),'reserved:True');await action('capture',false);await delay(500);
  assert.equal(await input('focus'),'focused:True');await delay(350);await openWithKey('f1','Cold F1 with a global shortcut conflict');
  assert.equal(JSON.parse(await readFile(join(output,'guide-shortcut.json'),'utf8')).registered,false);
  const page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('/guide/mayhem.html')));
  page.on('pageerror',e=>errors.push(e.message));await page.waitForFunction(()=>document.querySelectorAll('.augment-row').length>0);
  const parsed=parseMayhemBuild(JSON.parse(await readFile(join(root,'tests/fixtures/mayhem-annie.json'),'utf8')),1);
  assert.equal(await page.locator('#skill-select').inputValue(),parsed.skillOrders[0].priority.join('-'));
  assert.ok((await page.locator('#skill-select option').allTextContents())[0].startsWith('Best ·'));
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
  await action('hide');await delay(250);assert.equal(child.exitCode,null);await forward();
  assert.deepEqual(errors,[]);
  await writeFile(join(output,'verified.json'),JSON.stringify({observations,compactUI:true,removedPages:true,defaultTray:true,skillRanking:true,activationHelper:true,fullscreenLikeReturn:true,persistence:true,trayPersistence:true,singleInstance:true,captureSuppressed:true,matchEndSuppressed:true,errors},null,2));
  console.log('Native acceptance passed: '+output);
}finally{clearTimeout(watchdog);if(helper?.exitCode===null)helper.kill();await stop();}
