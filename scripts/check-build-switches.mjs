// Deterministic guide switching in headless Edge (or real WebView2 with --native).
// All pages/data are disposable fixtures;
// there are no provider requests, League access, or commands sent to installed Oracle.
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {createServer} from 'node:net';
import {chromium} from '../dependencies/playwright-core/index.mjs';

const root=fileURLToPath(new URL('../',import.meta.url));
const executable=resolve(root,process.env.ORACLE_TEST_EXE||'target/release/oracle.exe');
assert.ok(['target','.test'].some(dir=>executable.toLowerCase().startsWith(resolve(root,dir).toLowerCase()+'\\')));
const baseline=process.argv.includes('--baseline');
const native=process.argv.includes('--native');
const source=baseline
  ?execFileSync('git',['show','86a4cd6:src/ui/guide/metasrc-guide.js'],{cwd:root,encoding:'utf8'})
  :await readFile(join(root,'src/ui/guide/metasrc-guide.js'),'utf8');
const template=await readFile(join(root,'src/ui/guide/mayhem.html'),'utf8');
const style=(await Promise.all(['appearance.css','guide/mayhem.css','guide/metasrc-guide.css','guide/guide-theme.css','designs/eclipse.css'].map(name=>readFile(join(root,'src/ui',name),'utf8')))).join('\n');
const output=join(root,'.test','build-switches-'+Date.now());await mkdir(output,{recursive:true});
const server=createServer();await new Promise(done=>server.listen(0,'127.0.0.1',done));
const port=server.address().port;await new Promise(done=>server.close(done));
const env={...process.env,ORACLE_DATA_DIR:output,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`};
let child,browser;const errors=[];
async function until(fn,ms=20000){const end=Date.now()+ms;do{const value=await fn();if(value)return value;if(child&&child.exitCode!==null)throw Error('Oracle exited');await delay(60);}while(Date.now()<end);throw Error('Timed out: '+fn.toString());}
async function forward(args=[]){const p=spawn(executable,args,{cwd:root,env,windowsHide:true,stdio:'ignore'});await new Promise(done=>p.once('exit',done));assert.equal(p.exitCode,0);}
const watchdog=setTimeout(()=>{child?.kill();void browser?.close();},120000);watchdog.unref();
try {
  let page;
  if(native){
    child=spawn(executable,['--fixture'],{cwd:root,env,windowsHide:true,stdio:'ignore'});
    await delay(800);await forward();
    await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});
    browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));
    await page.waitForFunction(()=>document.querySelector('#desk-champion')?.options.length>100);
  }else{
    browser=await chromium.launch({channel:'msedge',headless:true});
    page=await browser.newPage({viewport:{width:1600,height:900}});
    await page.goto('about:blank');
  }
  page.on('pageerror',e=>errors.push(e.message));
  // Block fixture artwork: decoded-image/network speed must not skew the DOM comparison.
  await page.route('https://cdn.metasrc.com/**',route=>route.fulfill({contentType:'image/svg+xml',body:'<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#b4975a"/></svg>'}));
  const results=[];
  for(const mode of ['aram','rift','arena']) {
    await page.evaluate(async({mode})=>{
      document.querySelector('#switch-fixture')?.remove();
      const frame=document.createElement('iframe');frame.id='switch-fixture';
      frame.style='position:fixed;inset:0;width:100%;height:100%;border:0;z-index:2147483647';
      const ready=new Promise(done=>frame.onload=done);frame.srcdoc='<html><head></head><body><main id="main"></main></body></html>';
      document.body.append(frame);await ready;
      const win=frame.contentWindow,doc=win.document;
      const base=`https://www.metasrc.com/lol/${mode==='rift'?'':mode+'/'}champions/ahri/build`;
      const urls=[base,base+'?build=ap',base+'?build=ad'];
      const image=(id,name='Item '+id)=>`<img src="https://cdn.metasrc.com/static/item-${id}.png" alt="${name}">`;
      const group=(id,multi=false)=>`<div class="group">${image(id)}${multi?image(id+1):''}<span>54%</span><span>12%</span></div>`;
      const section=(title,body)=>`<section><h2>${title}</h2>${body}</section>`;
      const state=win.__switch={base,urls,mode,href:base,requests:[],runeClicks:[],aborts:[],patch:'16.23'};
      doc.addEventListener('htmx:abort',event=>state.aborts.push(event.target.id));
      state.html=(index,rune='0')=>{
        const stats=['Win %','Pick %','Tier','Top 3 %','Placement','Games'].map(label=>`<span><span>${label}:</span><strong>${label==='Games'?'12345':'54%'}</strong></span>`).join('');
        const tabs=urls.map((url,i)=>`<a id="style-${i}" href="${url}" class="${i===index?'active':''}"><span data-tooltip-select="#tab-${i}">${image(3000+i)}<span>${['Recommended','AP','AD'][i]}</span></span></a>`).join('');
        const runes=section('Ahri Runes',`<div data-tab-url-value="unused" data-tab-current-value="${rune}">${['0','1'].map(key=>`<button id="rune-${index}-${key}" data-tab-key="${key}" class="${key===rune?'active':''}">${image(8000+Number(key))}<span>55%</span><span>10%</span></button>`).join('')}<div data-tab-target="content">${image(8100+Number(rune))}</div></div>`);
        const arena=mode==='arena'?`<script type="application/json" id="ArenaWidget_row_data">${JSON.stringify(Array.from({length:317},(_,i)=>({key:(i<227?'augment-':'item-')+(10000+i),title:'Choice '+i,image:`https://cdn.metasrc.com/static/choice-${i}.png`,displays:{rarity:i<227?['Silver','Gold','Prismatic'][i%3]:'Item'},values:{tier:i%5===0?['S','A','B'][index]:'S'}})))}</script>`:'';
        return `<header id="shell-header"><h1>Ahri Patch ${state.patch}</h1>${image(103,'Ahri')}<div data-build-stats="full">${stats}</div>${tabs}</header><div id="build-section-content">${section('Ahri Items',group(3100+index*10,true)+Array.from({length:50},(_,i)=>group(4000+i)).join(''))}${section('Ahri Boots',group(3006))}${section('Ahri Skill Order','Priority Q > W > E Win % 54% Pick % 12%')}${runes}${section('Starting Items',group(1056))}${section('Summoner Spells',group(4,true))}${section('Slot 1 Item Options',Array.from({length:15},(_,i)=>group(5000+i)).join(''))}${arena}</div>`;
      };
      state.deliver=(index,rune='0')=>{state.href=urls[index];doc.querySelector('#main').innerHTML=state.html(index,rune);};
      state.deliver(0);
      doc.addEventListener('click',event=>{
        const link=event.target.closest('[id^="style-"]');
        if(link){event.preventDefault();state.requests.push(link.href);}
        const button=event.target.closest('[data-tab-key]');
        if(button){const index=urls.indexOf(state.href);state.runeClicks.push({index,key:button.dataset.tabKey});state.deliver(index,button.dataset.tabKey);}
      });
    },{mode});
    const frame=page.frames().find(f=>f!==page.mainFrame());
    assert.ok(frame,'fixture frame exists');
    await frame.evaluate(source);
    await frame.evaluate(({template,style})=>{
      const state=__switch;
      const values=new Map(),storage={getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)};
      state.guide=globalThis.metasrcGuide.install({template,style,appearance:{theme:'umbra',colour:'#ae1236'},champions:Array.from({length:172},(_,i)=>({slug:i===0?'ahri':'champion-'+i,name:i===0?'Ahri':'Champion '+i}))},{window,document,storage,url:()=>state.href,changeTimeoutMs:1000});
      state.root=document.querySelector('fryiloco-guide').shadowRoot;
      state.click=index=>state.root.querySelector(`[data-build-style="${CSS.escape(state.urls[index])}"]`).click();
      state.selected=()=>state.root.querySelector('[data-build-style][aria-pressed="true"]')?.dataset.buildStyle;
    },{template,style});
    for(const index of [1,2,0]) {
      await frame.evaluate(index=>{__switch.click(index);__switch.deliver(index);},index);
      await frame.waitForFunction(index=>__switch.selected()===__switch.urls[index]&&!__switch.root.querySelector('[aria-busy="true"]'),index);
    }
    await frame.evaluate(async()=>{await document.fonts.ready;await new Promise(done=>setTimeout(done,250));});
    const timing=await frame.evaluate(async()=>{
      const s=__switch,root=s.root;
      const nav=root.querySelector('[data-build-style]'),select=root.querySelector('#champion-select'),layout=root.querySelector('.meta-standard,.meta-arena-layout');
      const samples=[],paint=[];let added=0,correctData=true;
      const observer=new MutationObserver(records=>{for(const r of records)added+=r.addedNodes.length;});
      observer.observe(root,{subtree:true,childList:true});
      for(let i=0;i<60;i++){
        const index=(i+1)%3,start=performance.now();s.click(index);samples.push(performance.now()-start);
        correctData&&=s.mode==='arena'
          ?root.querySelector('[data-entry="augment-10000"] .grade').textContent===['S','A','B'][index]
          :root.querySelector('.meta-core img').getAttribute('src').endsWith(`item-${3100+index*10}.png`);
        await new Promise(done=>requestAnimationFrame(()=>{paint.push(performance.now()-start);done();}));
      }
      observer.disconnect();
      const summary=values=>{values.sort((a,b)=>a-b);return{medianMs:values[Math.floor(values.length/2)],p95Ms:values[Math.floor(values.length*.95)],maxMs:values.at(-1)};};
      return {handler:summary(samples),nextFrame:summary(paint),addedNodes:added,correctData,buttonsReused:nav===root.querySelector('[data-build-style]'),championSelectReused:select===root.querySelector('#champion-select'),layoutReused:layout===root.querySelector('.meta-standard,.meta-arena-layout')};
    });
    // A successful latest response must remain visible even if an older request finishes later.
    const race=await frame.evaluate(async()=>{
      const s=__switch;s.click(1);s.click(2);s.click(1);s.deliver(1);await new Promise(done=>setTimeout(done,200));
      s.deliver(2);await new Promise(done=>setTimeout(done,200));
      return {latestRemainsSelected:s.selected()===s.urls[1],obsoleteRequestsAborted:s.aborts.length>0};
    });
    const behavior=await frame.evaluate(async()=>{
      const s=__switch;
      // Return to the known provider state before exercising a cached rune choice.
      s.href=s.urls[0];s.deliver(0);dispatchEvent(new PopStateEvent('popstate'));
      s.click(1);
      const rune=s.root.querySelector('[data-rune-page="1"]');rune?.click();
      const clicksBefore=s.runeClicks.length;
      s.deliver(1);await new Promise(done=>setTimeout(done,250));
      const runeDeferred=s.mode==='arena'?null:clicksBefore===0&&s.runeClicks.at(-1)?.index===1&&s.runeClicks.at(-1)?.key==='1';
      const runeSelected=s.mode==='arena'?null:s.root.querySelector('[data-rune-page="1"]')?.getAttribute('aria-pressed')==='true';
      s.click(2);await new Promise(done=>setTimeout(done,1150));
      const cachedFailureKeepsSelection=s.selected()===s.urls[2];
      // A patch refresh must rebuild a cached layout instead of reusing stale recommendations.
      s.patch='16.24';s.deliver(0);dispatchEvent(new PopStateEvent('popstate'));
      const patchRefreshed=s.root.querySelector('#data-date').textContent==='Patch 16.24';
      s.click(1);s.deliver(1);await new Promise(done=>setTimeout(done,200));
      const actualNow=Date.now;Date.now=()=>actualNow()+16*60000;
      s.click(0);const expiredCacheIgnored=s.selected()===s.urls[1];
      Date.now=actualNow;s.deliver(0);await new Promise(done=>setTimeout(done,200));
      const target=s.root.querySelector('.meta-art[tabindex="0"]');
      target.dispatchEvent(new MouseEvent('mouseover',{bubbles:true}));
      const tooltipRestored=!s.root.querySelector('#item-tooltip').hidden&&s.root.querySelector('#item-tooltip').textContent===target.getAttribute('aria-label');
      s.root.querySelector('#meta-item-names').click();s.click(1);
      const itemNamesPreserved=document.querySelector('fryiloco-guide').classList.contains('named-items');
      const sourceNode=document.querySelector('#style-1'),oldXhr={},newXhr={};
      sourceNode.dispatchEvent(new CustomEvent('htmx:beforeRequest',{bubbles:true,detail:{elt:sourceNode,xhr:oldXhr}}));
      s.click(2);s.click(1);
      sourceNode.dispatchEvent(new CustomEvent('htmx:beforeRequest',{bubbles:true,detail:{elt:sourceNode,xhr:newXhr}}));
      sourceNode.dispatchEvent(new CustomEvent('htmx:sendError',{bubbles:true,detail:{elt:sourceNode,xhr:oldXhr}}));
      const oldErrorsIgnored=s.root.querySelector(`[data-build-style="${CSS.escape(s.urls[1])}"]`).getAttribute('aria-busy')==='true';
      s.deliver(1);await new Promise(done=>setTimeout(done,200));
      s.click(0);document.documentElement.classList.add('dynamic-challenge');
      const beforeChallenge=s.aborts.length;s.click(2);
      await new Promise(done=>setTimeout(done,1100));
      const humanCheckPreserved=beforeChallenge===s.aborts.length&&s.root.querySelector('#notice').textContent.includes('human check')&&s.root.querySelector('[aria-busy="true"]')!==null;
      document.documentElement.classList.remove('dynamic-challenge');s.deliver(2);
      await new Promise(done=>setTimeout(done,200));
      return {runeDeferred,runeSelected,cachedFailureKeepsSelection,patchRefreshed,expiredCacheIgnored,tooltipRestored,itemNamesPreserved,oldErrorsIgnored,humanCheckPreserved};
    });
    results.push({mode,...timing,...race,...behavior});
    console.log(JSON.stringify(results.at(-1)));
    if(!baseline){
      assert.equal(timing.buttonsReused,true);assert.equal(timing.championSelectReused,true);
      assert.equal(timing.correctData,true);
      assert.equal(timing.layoutReused,true);
      assert.equal(race.latestRemainsSelected,true);
      assert.equal(race.obsoleteRequestsAborted,true);
      if(mode!=='arena'){assert.equal(behavior.runeDeferred,true);assert.equal(behavior.runeSelected,true);}
      assert.equal(behavior.cachedFailureKeepsSelection,true);assert.equal(behavior.patchRefreshed,true);
      assert.equal(behavior.expiredCacheIgnored,true);assert.equal(behavior.tooltipRestored,true);assert.equal(behavior.itemNamesPreserved,true);
      assert.equal(behavior.oldErrorsIgnored,true);assert.equal(behavior.humanCheckPreserved,true);
      assert.ok(timing.handler.p95Ms<16,'cached click handling must fit within a 60 Hz frame');
    }
    await page.screenshot({path:join(output,mode+'.png')});
    await frame.evaluate(()=>dispatchEvent(new Event('beforeunload')));
  }
  assert.deepEqual(errors,[]);
  await writeFile(join(output,'verified.json'),JSON.stringify({baseline,native,fixtures:true,results,errors},null,2));
  console.log('Build switching evidence: '+output);
} finally {
  clearTimeout(watchdog);if(browser)await browser.close();
  if(child?.exitCode===null)await forward(['--quit']);
}
