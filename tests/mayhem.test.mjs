import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {parseMayhemBuild,parseMayhemRankings,tencentPayload,sortSkillOrders} from '../src/ui/guide/mayhem-data.mjs';
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
test('skill priorities and detailed upgrades choose highest win rate, then pick rate',()=>{
 const rows=[{id:'popular',winRate:51,pickRate:80},{id:'tie-low',winRate:55,pickRate:10},{id:'best',winRate:55,pickRate:30},{id:'unknown',winRate:null,pickRate:90}];
 assert.deepEqual(sortSkillOrders(rows).map(r=>r.id),['best','tie-low','popular','unknown']);
 assert.equal(rows[0].id,'popular','ranking does not mutate provider data');
 const payload=tencentPayload(build);
 const variant={...payload,skill_json:JSON.stringify({
  low:{qwe:'1&2&3',qwe_w:5000,qwe_s:8000,sks:{a:{sk:'1&2&3',sk_w:5000,sk_s:7000}}},
  high:{qwe:'1&3&2',qwe_w:5500,qwe_s:2000,sks:{a:{sk:'1&3&2',sk_w:5200,sk_s:7000},b:{sk:'3&1&2',sk_w:6000,sk_s:1000},c:{sk:'2&1&3',sk_w:6000,sk_s:2000}}}
 })};
 const sorted=parseMayhemBuild(variant,1).skillOrders;
 assert.deepEqual(sorted[0].priority,['Q','E','W']);
 assert.deepEqual(sorted[0].plans.map(p=>p.order),[['W','Q','E'],['E','Q','W'],['Q','E','W']]);
 assert.equal(sorted[0].winRate,55);
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
