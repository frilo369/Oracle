import {test} from 'node:test';
import assert from 'node:assert/strict';
import {captureController,revisionGate,mergeSnapshot,refreshController} from '../src/ui/async-state.mjs';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
test('cancelling an in-flight capture always reaches native after it starts',async()=>{
  const start=deferred(),calls=[];
  const c=captureController(on=>{calls.push(on);return on?start.promise:Promise.resolve();});
  const first=c.set(true);await Promise.resolve();
  assert.deepEqual(calls,[true]);
  const second=c.set(false);assert.equal(c.active,false);
  start.resolve();await Promise.all([first,second]);
  assert.deepEqual(calls,[true,false]);assert.equal(c.pending,false);
});
test('capture start failure clears the UI and a later attempt can recover',async()=>{
  let fail=true;const c=captureController(async()=>{if(fail)throw Error('offline');});
  await assert.rejects(c.set(true),/offline/);assert.equal(c.active,false);
  fail=false;await c.set(true);assert.equal(c.active,true);await c.set(false);
});
test('a failed cancellation can be retried without re-enabling recording',async()=>{
  let fail=false;const calls=[];const c=captureController(async on=>{calls.push(on);if(fail)throw Error('offline');});
  await c.set(true);fail=true;await assert.rejects(c.set(false),/offline/);
  assert.equal(c.active,false);fail=false;await c.set(false);assert.deepEqual(calls,[true,false,false]);
});
test('polls started before a completed settings save are rejected',()=>{
  const gate=revisionGate(),old=gate.snapshot(),done=gate.begin();
  assert.equal(gate.accepts(old),false);done();assert.equal(gate.accepts(old),false);
  assert.equal(gate.accepts(gate.snapshot()),true);
  const first=gate.begin(),second=gate.begin();first();assert.equal(gate.accepts(gate.snapshot()),false);second();assert.equal(gate.accepts(gate.snapshot()),true);
});

test('compact updates preserve the catalogue and retired data without repainting unchanged state',()=>{
  const state={champions:[{id:1}],settings:{notes:{1:'Keep me'},assetColour:'#ae1236'},guideTarget:null};
  const unchanged={settings:{assetColour:'#ae1236'},guideTarget:null};
  assert.equal(mergeSnapshot(state,unchanged),state);
  const next=mergeSnapshot(state,{...unchanged,guideTarget:{id:103,mode:'aram'}});
  assert.equal(next.champions,state.champions);
  assert.equal(next.settings.notes,state.settings.notes);
  assert.equal(next.guideTarget.id,103);
  assert.equal(state.guideTarget,null);
});

test('background refresh sleeps until shown and never overlaps a pending request',async()=>{
  const timers=new Map();let id=0,calls=0,pending=deferred();
  const clock={setTimeout(fn,ms){timers.set(++id,{fn,ms});return id;},clearTimeout(key){timers.delete(key);}};
  const refresh=refreshController(()=>{calls++;return pending.promise;},1200,clock);
  refresh.setActive(true);assert.equal(calls,1);
  refresh.setActive(false);refresh.setActive(true);assert.equal(calls,1);
  pending.resolve();await Promise.resolve();
  assert.equal(timers.size,1);assert.equal([...timers.values()][0].ms,0);
  const timer=[...timers.values()][0];pending=deferred();timer.fn();assert.equal(calls,2);
  refresh.setActive(false);pending.resolve();await Promise.resolve();assert.equal(timers.size,0);
  refresh.setActive(true);await Promise.resolve();assert.equal(calls,3);
  assert.equal([...timers.values()][0].ms,1200);
  refresh.setActive(false);assert.equal(timers.size,0);
});
