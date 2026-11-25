import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {parseMayhemBuild,parseMayhemRankings,tencentPayload} from '../src/ui/guide/mayhem-data.mjs';
import {readMayhemGuide,saveMayhemGuide} from '../src/ui/guide/mayhem-cache.mjs';
const build=JSON.parse(readFileSync(new URL('./fixtures/mayhem-annie.json',import.meta.url),'utf8'));
const ranks=JSON.parse(readFileSync(new URL('./fixtures/mayhem-rankings.json',import.meta.url),'utf8'));
test('Mayhem renders the bundled wire response and refuses a different champion',()=>{
 const parsed=parseMayhemBuild(build,1);
 assert.equal(parsed.augments.length,109);
 assert.equal(parsed.sections.length,3);
 assert.ok(parsed.sections.every(section=>section.rows.length>0));
 assert.ok(parsed.skillOrders.length>0);
 assert.throws(()=>parseMayhemBuild(build,103),/different champion/);
});
test('Mayhem refuses empty builds, impossible dates and rankings from another date',()=>{
 assert.throws(()=>parseMayhemBuild({dtstatdate:'20261120',itemone_json:'{}'},1),/no Mayhem build/);
 assert.throws(()=>parseMayhemBuild({dtstatdate:'20260230',itemshoes:'1001$0.1$0.5'},1),/invalid data date/);
 const parsed=parseMayhemBuild(build,1);
 assert.ok(parseMayhemRankings(ranks,parsed.dataDate.replaceAll('-','')).champions.length>100);
 assert.throws(()=>parseMayhemRankings({...tencentPayload(ranks),dtstatdate:'20000101'},'20261120'));
});
test('Mayhem saved guides expire, preserve data during storage errors, and survive reopening',()=>{
 let stored='';const storage={getItem:()=>stored,setItem:(_,value)=>{stored=value;}};
 const now=Date.now();const response={championId:1,build:{value:build,fetchedAt:now/1000},notices:[],rankings:null};
 saveMayhemGuide(storage,response,now);
 assert.equal(readMayhemGuide(storage,1,now).championId,1);
 assert.equal(readMayhemGuide(storage,103,now),null);
 assert.equal(readMayhemGuide(storage,1,now+8*86400000),null);
 saveMayhemGuide({getItem(){throw Error('unavailable');},setItem(){throw Error('unavailable');}},response,now);
 assert.equal(readMayhemGuide({getItem:()=>'{broken'},1,now),null);
});
