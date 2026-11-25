// End-to-end owned exclusive-fullscreen test. No League/installed-Oracle commands.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile,copyFile,rename} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url)),output=join(root,'.test','gamebar-'+Date.now());await mkdir(output,{recursive:true});
const inbox=join(process.env.LOCALAPPDATA,'Packages/Frilo369.OracleWidget_9y1ac7xxt8800/LocalState');
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=0'};
let oracle,browser,page,game,pumping=false,stopPump=false;const evidence=[];
async function run(exe,args){const p=spawn(exe,args,{cwd:root,env,windowsHide:true,stdio:['ignore','pipe','pipe']});let text='';p.stdout.on('data',b=>text+=b);p.stderr.on('data',b=>text+=b);await new Promise(r=>p.once('exit',r));assert.equal(p.exitCode,0,text);return text;}
async function until(fn,ms=20000){const end=Date.now()+ms;do{const v=await fn();if(v)return v;await delay(50);}while(Date.now()<end);throw Error('Timed out: '+fn.toString());}
const exe=join(root,'target/release/oracle.exe');
const action=(kind,id=null)=>page.evaluate(({kind,id})=>window.__TAURI__.core.invoke('action',{kind,id}),{kind,id});
const ready=()=>page.evaluate(()=>window.__TAURI__.core.invoke('ready',{compact:true}));
async function widget(){try{return JSON.parse(await readFile(join(inbox,'widget-status.json'),'utf8'));}catch{return null;}}
async function bitmap(name,preview){const png=join(output,name+'.png'),bmp=join(output,name+'.bmp');await preview.screenshot({path:png});await run(join(output,'capture.exe'),['decode',png,bmp]);return bmp;}
async function readState(){try{return JSON.parse(await readFile(join(output,'overlay-state.json'),'utf8'));}catch{return null;}}
const watchdog=setTimeout(()=>{game?.kill();oracle?.kill();},150000);watchdog.unref();
try{
  const status=await widget();assert.ok(status?.pinned&&status.clickThrough,'Open/pin Oracle in Game Bar and enable click-through first');
  // Exercise host recovery too; no Win+G or other user input during selections.
  spawn('explorer.exe',['ms-gamebar://launch/activate/Frilo369.OracleWidget_9y1ac7xxt8800_App_OracleAugments'],{stdio:'ignore'}).unref();
  await until(async()=>{const w=await widget();return w&&Date.now()-w.at<3000;});
  await run(join(process.env.WINDIR,'Microsoft.NET/Framework64/v4.0.30319/csc.exe'),['/nologo','/r:System.Drawing.dll',`/out:${join(output,'capture.exe')}`,join(root,'tests/overlay-capture.cs')]);
  const vs=await run(join(process.env['ProgramFiles(x86)'],'Microsoft Visual Studio/Installer/vswhere.exe'),['-latest','-products','*','-requires','Microsoft.VisualStudio.Component.VC.Tools.x86.x64','-property','installationPath']);
  const fixture=join(output,'oracle-overlay-fixture.exe'),cmd=join(output,'compile.cmd');
  await writeFile(cmd,`@echo off\r\ncall "${join(vs.trim(),'Common7/Tools/VsDevCmd.bat')}" -arch=x64 -host_arch=x64 >nul\r\ncl /nologo /std:c++17 /EHsc /O2 /MT /DWIN32_LEAN_AND_MEAN /DNOMINMAX "${join(root,'tests/overlay-fullscreen.cpp')}" /Fo"${join(output,'fixture.obj')}" /Fe"${fixture}" /link d3d11.lib dxgi.lib user32.lib\r\nexit /b %errorlevel%\r\n`);
  await run('cmd.exe',['/d','/c',cmd]);
  oracle=spawn(exe,['--fixture'],{cwd:root,env,windowsHide:true,stdio:'ignore'});await delay(800);await run(exe,[]);
  const port=await until(async()=>{try{return Number((await readFile(join(output,'Dashboard-WebView2/EBWebView/DevToolsActivePort'),'utf8')).split(/\r?\n/)[0]);}catch{return 0;}});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));await page.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  await action('overlay-preview');const preview=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('overlay-preview.html')));await preview.evaluate(()=>document.fonts.ready);
  const mayhem=await bitmap('mayhem',preview);
  await preview.evaluate(()=>{const names=['ADAPt','Recursion','Witchful Thinking'];document.querySelectorAll('h2').forEach((n,i)=>n.textContent=names[i]);});
  const arena=await bitmap('arena',preview);
  await preview.locator('.pick').first().click();const selected=await bitmap('selected',preview);
  await action('fixture',{closeOverlayPreview:true,overlayFixtureWindow:true});
  await mkdir(join(output,'gamebar'),{recursive:true});
  // Bridge test-only documents to the explicitly installed development widget.
  pumping=true;void (async()=>{while(!stopPump){try{const heartbeat=await readFile(join(inbox,'widget-status.json'));await writeFile(join(output,'gamebar/widget-status.json'),heartbeat);const payload=JSON.parse(await readFile(join(output,'gamebar/overlay.json'),'utf8'));payload.qa=true;await writeFile(join(inbox,'overlay.test.tmp'),JSON.stringify(payload));await rename(join(inbox,'overlay.test.tmp'),join(inbox,'overlay.json'));}catch{}await delay(40);}pumping=false;})();
  let stdout='',stderr='';game=spawn(fixture,[mayhem],{cwd:root,env,windowsHide:true,stdio:['pipe','pipe','pipe']});game.stdout.on('data',b=>stdout+=b);game.stderr.on('data',b=>stderr+=b);
  const info=await until(()=>{if(game.exitCode!==null)throw Error('Fullscreen fixture exited: '+stderr);const line=stdout.split(/\r?\n/).find(l=>l.startsWith('{'));return line&&JSON.parse(line);});assert.equal(info.exclusive,true,'the test must use real DXGI exclusive fullscreen');evidence.push(info);
  const begin=Date.now();const shown=await until(async()=>{const s=await readState();return s?.visible&&s.labels.map(l=>l.id).join(',')==='3055,1205,1415'?s:null;});
  assert.ok(shown.labels[0].text.includes('13.85%'));
  const painted=await until(async()=>{const w=await widget();return w?.rendered===3?w:null;});evidence.push({mode:'mayhem',latencyMs:Date.now()-begin,state:shown,widget:painted});
  await run(join(output,'capture.exe'),[String(info.hwnd),String(game.pid),join(output,'fullscreen-mayhem.png'),'gamebar']);
  if(process.env.ORACLE_GAMEBAR_HOLD_MS)await delay(Math.min(30000,Math.max(0,Number(process.env.ORACLE_GAMEBAR_HOLD_MS)||0)));
  await action('fixture',{overlayFixtureArena:true});const swap=Date.now();game.stdin.write(arena+'\n');
  const arenaShown=await until(async()=>{const s=await readState();return s?.visible&&s.labels.map(l=>l.id).join(',')==='205,68,97'?s:null;});
  assert.deepEqual(arenaShown.labels.map(l=>l.text),['Pick 11.11%   |   Tier S','Pick 22.22%   |   Tier A','Pick 33.33%   |   Tier B']);
  await until(async()=>{const w=await widget();return w?.rendered===3&&w.documentAt>=swap;});evidence.push({mode:'arena',latencyMs:Date.now()-swap,state:arenaShown});
  await run(join(output,'capture.exe'),[String(info.hwnd),String(game.pid),join(output,'fullscreen-arena.png'),'gamebar']);
  const selection=Date.now();game.stdin.write(selected+'\n');await until(async()=>(await readState())?.visible===false);await until(async()=>(await widget())?.rendered===0);evidence.push({selectionHideMs:Date.now()-selection});
  // Transport/layout acknowledgements are NOT proof that the compositor draws
  // over the exclusive surface. Physical visibility remains a separate gate.
  await copyFile(join(inbox,'widget-preview.png'),join(output,'widget-render.png')).catch(()=>{});
  await writeFile(join(output,'model-verified.json'),JSON.stringify({exclusiveFullscreenCapture:true,automaticRecognition:true,physicalVisibilityVerified:false,evidence},null,2));console.log('Game Bar OCR/transport checks passed; fullscreen display still requires visual acceptance: '+output);
}catch(e){console.error('Evidence: '+output);console.error(JSON.stringify(await readState()));console.error(JSON.stringify(await widget()));throw e;}
finally{
  stopPump=true;await until(()=>!pumping,3000).catch(()=>{});
  await writeFile(join(inbox,'overlay.json'),JSON.stringify({schema:1,at:0,visible:false,labels:[]}));
  if(game?.exitCode===null){game.stdin.write('quit\n');await Promise.race([new Promise(r=>game.once('exit',r)),delay(4000)]);if(game.exitCode===null)game.kill();}
  if(browser)await browser.close();if(oracle?.exitCode===null)await run(exe,['--quit']);clearTimeout(watchdog);
}
