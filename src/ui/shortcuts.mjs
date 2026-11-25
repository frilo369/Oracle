const supported=/^(?:Key[A-Z]|Digit[0-9]|Numpad[0-9]|F(?:[1-9]|1[0-9]|2[0-4])|Space|Tab|Enter|Backspace|Delete|Insert|Home|End|PageUp|PageDown|ArrowUp|ArrowDown|ArrowLeft|ArrowRight|Backquote|Minus|Equal|BracketLeft|BracketRight|Backslash|Semicolon|Quote|Comma|Period|Slash|NumpadAdd|NumpadSubtract|NumpadMultiply|NumpadDivide|NumpadDecimal)$/;
export function shortcutFromEvent(e){
  if(e.metaKey||!supported.test(e.code))return null;
  if(!/^F\d+$/.test(e.code)&&!e.ctrlKey&&!e.altKey)return null;
  if(e.altKey&&['F4','Tab'].includes(e.code))return null;
  return [e.ctrlKey?'Ctrl':null,e.altKey?'Alt':null,e.shiftKey?'Shift':null,e.code].filter(Boolean).join('+');
}
export function shortcutLabel(binding){return String(binding||'').replaceAll('Key','').replaceAll('Digit','').replaceAll('+',' + ');}
