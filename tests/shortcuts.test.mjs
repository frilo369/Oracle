import test from 'node:test';
import assert from 'node:assert/strict';
import {shortcutFromEvent,shortcutLabel} from '../src/ui/shortcuts.mjs';
const key=(code,modifiers={})=>shortcutFromEvent({code,...modifiers});
test('shortcut capture agrees with supported native key codes',()=>{
  assert.equal(key('F1'),'F1');
  assert.equal(key('KeyG',{ctrlKey:true,shiftKey:true}),'Ctrl+Shift+KeyG');
  assert.equal(key('Numpad7',{altKey:true}),'Alt+Numpad7');
  assert.equal(key('KeyG'),null);
  assert.equal(key('F4',{altKey:true}),null);
  assert.equal(key('Tab',{altKey:true}),null);
  assert.equal(key('F25'),null);
  assert.equal(key('ShiftLeft'),null);
  assert.equal(key('F1',{metaKey:true}),null);
  assert.equal(shortcutLabel('Ctrl+Shift+KeyG'),'Ctrl + Shift + G');
});
