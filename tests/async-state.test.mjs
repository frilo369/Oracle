import {test} from 'node:test';
import assert from 'node:assert/strict';
import {captureController,revisionGate} from '../src/ui/async-state.mjs';
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
