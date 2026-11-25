import {parseMayhemBuild} from '../src/ui/guide/mayhem-data.mjs';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const results=[];
for(const champion of [1,103]) {
  const response=await fetch(`https://mlol.qt.qq.com/go/battle_info/odp_proxy/fuwen_hero_rank?championid=${champion}`,{signal:AbortSignal.timeout(15000)});
  if(!response.ok)throw Error('Mayhem provider HTTP '+response.status);
  const build=parseMayhemBuild(await response.json(),champion);
  if(!build.augments.length||!build.sections.length)throw Error('Incomplete Mayhem recommendations');
  results.push({champion,date:build.dataDate,augments:build.augments.length,sections:build.sections.length});
}
const output=fileURLToPath(new URL('../.test/mayhem-live.json',import.meta.url));
await mkdir(fileURLToPath(new URL('../.test/',import.meta.url)),{recursive:true});
await writeFile(output,JSON.stringify(results,null,2));console.log(JSON.stringify(results));
