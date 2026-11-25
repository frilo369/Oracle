// Extract only the reference's read-only game-asset reader. No WAD writer or skin engine.
import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {join} from 'node:path';
const root=fileURLToPath(new URL('../',import.meta.url));
const reference=process.argv[2]||String.raw`C:\Users\fryil\Documents\Rust-Friloverse-Skins`;
const between=(s,a,b)=>{const start=s.indexOf(a),end=s.indexOf(b,start+a.length);if(start<0||end<0)throw Error('Reference asset reader changed; review extraction.');return s.slice(start,end);};
const wad=readFileSync(join(reference,'src-tauri/src/wad.rs'),'utf8').replaceAll('\r\n','\n');
const reader=`//! Read-only League game-asset decoding, extracted from the owner's reference.
use flate2::read::GzDecoder;
use std::{collections::HashMap,fs::File,io::{Read,Seek,SeekFrom}};
use xxhash_rust::xxh64::xxh64;
const HEADER:usize=272;
const ENTRY:usize=32;
const MAX_ENTRIES:usize=500_000;
const MAX_BIN:usize=128*1024*1024;
${between(wad,'pub fn name_hash(', '/// A table row by hash.')}
${between(wad,'pub fn unpack(', '/// A row\'s file, unpacked.')}
${between(wad,'pub struct AssetArchive {','/// Members of a WAD on disk by hash,')}`;
const icons=readFileSync(join(reference,'src-tauri/src/league_icons.rs'),'utf8').replaceAll('\r\n','\n');
let assets=`//! Cached read-only guide artwork and metadata from League's two asset archives.
use crate::{league_path,asset_wad as wad};
use serde_json::Value;
use std::{collections::HashMap,path::{Path,PathBuf},sync::{Mutex,OnceLock},time::{Duration,Instant,SystemTime}};
${between(icons,'const ARCHIVES:','fn icon_id(')}
${between(icons,'fn image_type(', '/// One summoner\'s profile icon,')}
${between(icons,'pub fn game_file(', '#[cfg(test)]')}`;
assets=assets.replace('an LTK path change','a League path change');
const assetPath=join(root,'src-tauri/src/guide_assets.rs');
if(existsSync(assetPath)){
  const existing=readFileSync(assetPath,'utf8'),marker='// Oracle protocol and fixture adapters.';
  if(existing.includes(marker))assets+='\n'+existing.slice(existing.indexOf(marker));
}
writeFileSync(join(root,'src-tauri/src/asset_wad.rs'),reader);
writeFileSync(assetPath,assets);
console.log('Read-only guide asset modules extracted.');
