// Read-only installed League assets through Oracle's actual WebView protocol, in a .test profile.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url)),exe=join(root,'target/release/oracle.exe');
const output=join(root,'.test','live-assets-'+Date.now());await mkdir(output,{recursive:true});
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=0'};
const child=spawn(exe,[],{cwd:root,env,windowsHide:true,stdio:'ignore'});let browser;
async function until(fn){const end=Date.now()+25000;do{const v=await fn();if(v)return v;if(child.exitCode!==null)throw Error('Oracle exited');await delay(80);}while(Date.now()<end);throw Error('Live image route timed out');}
const watchdog=setTimeout(()=>child.kill(),45000);watchdog.unref();
try {
  await delay(850);const show=spawn(exe,[],{env,windowsHide:true,stdio:'ignore'});await new Promise(done=>show.once('exit',done));
  const port=await until(async()=>{try{return Number((await readFile(join(output,'Dashboard-WebView2/EBWebView/DevToolsActivePort'),'utf8')).split(/\r?\n/)[0]);}catch{return 0;}});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const dashboard=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));
  await dashboard.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  await dashboard.evaluate(()=>window.__TAURI__.core.invoke('action',{kind:'open-guide',id:{champion:1,mode:'mayhem'}}));
  const page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('/guide/mayhem.html')));
  await page.waitForFunction(()=>typeof window.umbraShowChampion==='function');
  const path='http://umbra-icon.localhost/guide/v1/champion-icons/1.png?oracle-read-check='+Date.now();
  const response=page.waitForResponse(r=>r.url()===path);
  const decoded=await page.evaluate(src=>new Promise((done,fail)=>{const image=new Image();image.onload=()=>done({width:image.naturalWidth,height:image.naturalHeight});image.onerror=()=>fail(Error('Local League image failed to decode'));image.src=src;}),path);
  const received=await response;assert.equal(received.status(),200);assert.ok(decoded.width>0&&decoded.height>0);
  assert.equal(received.headers()['cache-control'],'private, max-age=60');assert.equal(received.headers()['content-type'],'image/png');
  const result={nativeProtocol:true,readOnly:true,installedLeagueImage:true,decoded,cacheControl:received.headers()['cache-control'],contentType:received.headers()['content-type']};
  await writeFile(join(output,'verified.json'),JSON.stringify(result,null,2));console.log('Live local image route passed: '+output);
}finally{clearTimeout(watchdog);if(browser)await browser.close();if(child.exitCode===null){const quit=spawn(exe,['--quit'],{env,windowsHide:true,stdio:'ignore'});await new Promise(done=>quit.once('exit',done));}}
