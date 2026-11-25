// Verify the inherited live provider layouts. Human verification is observed, never automated.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url));
const output=join(root,'.test','meta-'+Date.now());await mkdir(output,{recursive:true});
const exe=join(root,'target/release/oracle.exe');
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=0'};
const child=spawn(exe,['--fixture'],{cwd:root,windowsHide:true,stdio:'ignore',env});
let dashboardBrowser,guideBrowser;const results=[];
async function until(fn,ms=25000){const end=Date.now()+ms;do{const v=await fn();if(v)return v;if(child.exitCode!==null)throw Error('Oracle exited');await delay(100);}while(Date.now()<end);throw Error('Timed out waiting for '+fn.toString().slice(0,120));}
async function port(folder){return until(async()=>{try{return Number((await readFile(join(output,folder,'EBWebView/DevToolsActivePort'),'utf8')).split(/\r?\n/)[0]);}catch{return 0;}});}
const watchdog=setTimeout(()=>child.kill(),160000);watchdog.unref();
try {
  await delay(800);const show=spawn(exe,[],{env,windowsHide:true,stdio:'ignore'});await new Promise(done=>show.once('exit',done));
  dashboardBrowser=await chromium.connectOverCDP(`http://127.0.0.1:${await port('Dashboard-WebView2')}`);
  const dashboard=await until(()=>dashboardBrowser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));
  await dashboard.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  for(const mode of ['arena','aram','rift']) {
    await dashboard.evaluate(mode=>window.__TAURI__.core.invoke('action',{kind:'open-guide',id:{champion:103,mode}}),mode);
    if(!guideBrowser)guideBrowser=await chromium.connectOverCDP(`http://127.0.0.1:${await port('Guide-WebView2')}`);
    const path=mode==='rift'?'/lol/champions/ahri/build':`/lol/${mode}/champions/ahri/build`;
    const page=await until(()=>guideBrowser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes(path)));
    let outcome;
    try {outcome=await until(async()=>page.evaluate(()=>{
      const r=document.querySelector('fryiloco-guide')?.shadowRoot;
      if(r?.querySelector('#guide-content')&&!r.querySelector('#guide-content').hidden)return {rendered:true,champion:r.querySelector('#champion-name')?.textContent,cards:r.querySelectorAll('.meta-card').length,augments:r.querySelectorAll('.augment-row').length};
      if(/security verification|just a moment|verify you are human/i.test(document.body?.innerText||'')||document.querySelector('iframe[src*="challenges.cloudflare.com"]'))return {skipped:'Provider human verification'};
      return null;
    }),30000);}catch(e){await page.screenshot({path:join(output,mode+'-failed.png')});throw e;}
    await page.screenshot({path:join(output,mode+'.png')});
    if(outcome.rendered){assert.equal(outcome.champion,'Ahri');assert.ok(outcome.cards>=4||outcome.augments>50);const skill=await page.evaluate(()=>[...document.querySelectorAll('#build-section-content h2')].find(h=>h.textContent.includes('Skill Order'))?.closest('section')?.outerHTML||'');await writeFile(join(output,mode+'-skill-source.html'),skill);}
    const denied=await page.evaluate(async()=>{try{await window.__TAURI__.core.invoke('action',{kind:'save-settings',id:{guideHotkey:'F9'}});return false;}catch{return true;}});
    assert.equal(denied,true,'remote pages cannot invoke Oracle commands');
    results.push({mode,...outcome,remoteCommandsDenied:denied});console.log(mode+': '+JSON.stringify(outcome));
  }
  await writeFile(join(output,'verified.json'),JSON.stringify({live:true,results},null,2));console.log('Live provider checks: '+output);
}finally{clearTimeout(watchdog);if(guideBrowser)await guideBrowser.close();if(dashboardBrowser)await dashboardBrowser.close();if(child.exitCode===null){const q=spawn(exe,['--quit'],{env,windowsHide:true,stdio:'ignore'});await new Promise(done=>q.once('exit',done));}}
