// Native dashboard visual acceptance. Operates only on workspace executables/test profiles.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createServer} from 'node:net';
import {chromium} from '../dependencies/playwright-core/index.mjs';
const root=fileURLToPath(new URL('../',import.meta.url)),exe=resolve(root,process.argv[2]||'target/release/oracle.exe');
assert.ok(['target','.test'].some(dir=>exe.toLowerCase().startsWith(resolve(root,dir).toLowerCase()+'\\')));
const output=join(root,'.test','design-'+Date.now());await mkdir(output,{recursive:true});
const server=createServer();await new Promise(done=>server.listen(0,'127.0.0.1',done));const port=server.address().port;await new Promise(done=>server.close(done));
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
let child,browser,page;const errors=[],observations=[];
async function until(fn,ms=20000){const end=Date.now()+ms;do{const v=await fn();if(v)return v;if(child.exitCode!==null)throw Error('Oracle exited');await delay(60);}while(Date.now()<end);throw Error('Timed out: '+fn.toString());}
async function forward(args=[]){const p=spawn(exe,args,{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>p.once('exit',done));assert.equal(p.exitCode,0);}
const action=(kind,id=null)=>page.evaluate(({kind,id})=>window.__TAURI__.core.invoke('action',{kind,id}),{kind,id});
async function snapshot(name){
  await page.evaluate(()=>document.fonts.ready);await delay(150);
  const layout=await page.evaluate(()=>{
    const main=document.querySelector('main');
    const clipped=[...document.querySelectorAll('button,select,h2,h3,.setting-row strong,.connection')].filter(n=>{const box=n.getBoundingClientRect();if(!box.height)return false;const range=document.createRange();range.selectNodeContents(n);return range.getBoundingClientRect().width>box.width+2;}).map(n=>n.id||n.textContent.trim());
    const escaped=[...document.querySelectorAll('button,select,input,.portrait')].filter(n=>{const r=n.getBoundingClientRect();return r.height&&(r.left<0||r.top<0||r.right>innerWidth+1||r.bottom>innerHeight+1);}).map(n=>n.id||n.textContent.trim());
    return {width:innerWidth,height:innerHeight,horizontal:document.documentElement.scrollWidth>innerWidth,scrolling:main.scrollHeight>main.clientHeight+2,clipped,escaped};
  });
  observations.push({name,...layout});await page.screenshot({path:join(output,name+'.png')});
  assert.equal(layout.horizontal,false,name);assert.equal(layout.scrolling,false,name+' page overflow');assert.deepEqual(layout.clipped,[],name+' clipped labels');assert.deepEqual(layout.escaped,[],name+' escaped controls');
}
const watchdog=setTimeout(()=>child?.kill(),120000);watchdog.unref();
try{
  child=spawn(exe,['--fixture'],{cwd:root,env,windowsHide:true,stdio:'ignore'});await delay(800);await forward();
  await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));
  page.on('pageerror',e=>errors.push(e.message));await page.waitForFunction(()=>document.querySelector('#desk-champion').options.length>100);
  await action('fixture',{target:null});await page.waitForFunction(()=>document.querySelector('#match-champion').textContent==='No active match');
  await snapshot('overview');
  assert.equal(await page.locator('#open-current').isDisabled(),true,'no match cannot open a match guide');
  const artwork=await page.evaluate(async()=>{
    const image=new Image();image.src='backgrounds/oracle-eclipse.webp';await image.decode();
    return {width:image.naturalWidth,height:image.naturalHeight,logo:document.querySelector('#match-icon').getAttribute('src')};
  });assert.equal(artwork.logo,'oracle-portrait.svg');assert.ok(artwork.width>=1400);
  await page.locator('nav [data-tab="settings"]').click();await snapshot('settings');
  await page.locator('#check-updates').click();await page.waitForFunction(()=>document.querySelector('#update-status').textContent.includes('up to date'));
  await page.locator('#start-windows').check();await until(async()=>(await action('fixture',{}))&&page.locator('#start-windows').isChecked());
  await page.locator('#start-windows').uncheck();
  await action('save-settings',{guideHotkey:'Ctrl+Alt+Shift+Super+F24'});
  await page.locator('nav [data-tab="overview"]').click();await page.waitForFunction(()=>document.querySelector('#record-key').textContent.includes('F24'));await snapshot('long-shortcut');
  await page.locator('#reset-key').click();await page.waitForFunction(()=>document.querySelector('#record-key').textContent==='F1');
  await action('fixture',{target:{id:136,mode:'rift'}});await page.waitForFunction(()=>document.querySelector('#match-champion').textContent==='Aurelion Sol');await snapshot('active-match');
  for(const size of [[720,570],[1280,720]]){
    await action('fixture',{dashboardSize:size});await page.waitForFunction(([w,h])=>Math.abs(innerWidth-w)<=1&&Math.abs(innerHeight-h)<=1,size);
    for(const tab of ['overview','settings']){await page.locator(`nav [data-tab="${tab}"]`).click();await snapshot(`${tab}-${size.join('x')}`);}
  }
  await action('fixture',{dashboardSize:[1040,780],target:null});await page.locator('nav [data-tab="overview"]').click();
  assert.deepEqual(errors,[]);await writeFile(join(output,'verified.json'),JSON.stringify({artwork,observations,updates:true,shortcut:true,errors},null,2));
  console.log('Design acceptance passed: '+output);
}finally{clearTimeout(watchdog);if(browser)await browser.close();if(child?.exitCode===null){await forward(['--quit']);if(child.exitCode===null)await new Promise(done=>child.once('exit',done));}}
