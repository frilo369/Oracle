// One-time extraction adjustments, retained to explain exactly what changed from Umbra.
import {readFileSync,writeFileSync} from 'node:fs';
const edit=(path,fn)=>writeFileSync(path,fn(readFileSync(path,'utf8')));
for(const path of ['src-tauri/src/match_guide.rs','src-tauri/src/guide.rs','src-tauri/oracle.manifest','src/ui/guide/mayhem.html','src/ui/guide/mayhem.js','src/ui/guide/metasrc-guide.js','installer/winforms/OracleSetup.cs','installer/winforms/app.manifest']) {
  edit(path,s=>s.replaceAll('Umbra','Oracle'));
}
edit('installer/winforms/OracleSetup.cs',s=>s.replaceAll('umbra','oracle').replaceAll('settings and skins','settings and notes').replaceAll('settings and skin library','settings and notes').replace('ltkNeeded = ProbeValue("ltk", "ok") == "needed";','ltkNeeded = false;'));
edit('src/ui/guide/mayhem.js',s=>s.replace('`http://umbra-icon.localhost/guide/${url.slice(6)}`','`https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/${url.slice(6)}`'));
// Reuse only the Default design in this product; the original design framework stays compatible.
edit('src/ui/appearance.js',s=>s.replace(/,\s*\{id:'eclipse'[^\n]*\}/,''));
