//! Cached read-only guide artwork and metadata from League's two asset archives.
use crate::{asset_wad as wad, league_path};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime},
};
const ARCHIVES: [&str; 2] = ["default-assets.wad", "default-assets2.wad"];
const GAME_DATA: &str = "plugins/rcp-be-lol-game-data/global/default/";
const MAX_ICON: u64 = 2 * 1024 * 1024;

fn root() -> Option<PathBuf> {
    // Folder discovery enumerates Windows processes. Do it once per burst of artwork,
    // while still noticing an LTK path change or another League install promptly.
    static ROOT: OnceLock<Mutex<Option<(Instant, PathBuf)>>> = OnceLock::new();
    let mut cached = ROOT.get_or_init(|| Mutex::new(None)).lock().ok()?;
    if let Some((at, path)) = cached.as_ref() {
        if at.elapsed() < Duration::from_secs(2) {
            return Some(path.clone());
        }
    }
    let path = league_path::folder().join("Plugins/rcp-be-lol-game-data");
    *cached = Some((Instant::now(), path.clone()));
    Some(path)
}
fn archive_path(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}
struct CachedArchive {
    stamp: (u64, Option<SystemTime>, Option<SystemTime>),
    archive: wad::AssetArchive,
}
#[derive(Default)]
struct AssetCache {
    root: PathBuf,
    archives: HashMap<&'static str, CachedArchive>,
}
impl AssetCache {
    fn read(&mut self, root: &Path, paths: &[String], max: u64) -> Option<Vec<u8>> {
        if self.root != root {
            self.root = root.to_path_buf();
            self.archives.clear();
        }
        for archive in ARCHIVES {
            let file = archive_path(root, archive);
            let Ok(meta) = std::fs::metadata(&file) else {
                self.archives.remove(archive);
                continue;
            };
            let stamp = (meta.len(), meta.modified().ok(), meta.created().ok());
            if !self
                .archives
                .get(archive)
                .is_some_and(|saved| saved.stamp == stamp)
            {
                self.archives.remove(archive);
                let Some(indexed) = wad::AssetArchive::open(&file) else {
                    continue;
                };
                self.archives.insert(
                    archive,
                    CachedArchive {
                        stamp,
                        archive: indexed,
                    },
                );
            }
            let indexed = &mut self.archives.get_mut(archive)?.archive;
            for path in paths {
                if let Some(bytes) = indexed.read(path, max) {
                    return Some(bytes);
                }
            }
        }
        None
    }
}
fn read_asset(root: &Path, paths: &[String], max: u64) -> Option<Vec<u8>> {
    // Only two archive indices/handles live here; decoded pictures belong to WebView's
    // bounded image cache. Do not retain the whole catalogue's image bytes in Rust.
    static ASSETS: OnceLock<Mutex<AssetCache>> = OnceLock::new();
    ASSETS
        .get_or_init(|| Mutex::new(AssetCache::default()))
        .lock()
        .ok()?
        .read(root, paths, max)
}
fn image(root: &Path, paths: &[String]) -> Option<(Vec<u8>, String)> {
    let bytes = read_asset(root, paths, MAX_ICON)?;
    let mime = image_type(&bytes)?;
    Some((bytes, mime.to_string()))
}

pub fn game_file(relative: &str) -> Option<(Vec<u8>, String)> {
    game_file_at(&root()?, relative)
}

fn game_file_at(root: &Path, relative: &str) -> Option<(Vec<u8>, String)> {
    let relative = relative.to_lowercase();
    if relative
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
        || !(relative.starts_with("assets/") || relative.starts_with("v1/"))
    {
        return None;
    }
    let path = format!("{GAME_DATA}{relative}");
    if relative.ends_with(".json") {
        let bytes = read_asset(root, &[path], 8 * 1024 * 1024)?;
        serde_json::from_slice::<Value>(&bytes).ok()?;
        return Some((bytes, "application/json; charset=utf-8".into()));
    }
    image(root, &[path])
}

pub fn guide_art(path: &str) -> Option<(Vec<u8>, String)> {
    guide_art_at(&root()?, path)
}

fn guide_art_at(root: &Path, path: &str) -> Option<(Vec<u8>, String)> {
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .ok()?;
    let valid = path.rsplit_once('.').is_some_and(|(_, ext)| {
        ["jpg", "png", "webp", "jpeg"]
            .iter()
            .any(|allowed| ext.eq_ignore_ascii_case(allowed))
    });
    if !valid {
        return None;
    }
    game_file_at(root, &path)
}

// Oracle protocol and fixture adapters.
pub fn response(
    app: &tauri::AppHandle,
    label: &str,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    use tauri::Manager;
    let found = (label == "guide"
        && request.method() == tauri::http::Method::GET
        && request.uri().host() == Some("localhost"))
    .then(|| request.uri().path().strip_prefix("/guide/"))
    .flatten()
    .and_then(|path| {
        let state = app.try_state::<crate::shell::Desktop>()?;
        if state.preview {
            None
        } else {
            guide_art(path)
        }
    });
    match found {
        Some((bytes, mime)) => tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", mime)
            .header("X-Content-Type-Options", "nosniff")
            .header("Cache-Control", "private, max-age=60")
            .body(std::borrow::Cow::Owned(bytes))
            .unwrap(),
        None => tauri::http::Response::builder()
            .status(404)
            .header("Cache-Control", "no-store")
            .body(std::borrow::Cow::Borrowed(&b"Not found"[..]))
            .unwrap(),
    }
}

#[cfg(test)]
#[path = "guide_asset_tests.rs"]
mod tests;

#[test]
#[ignore = "Explicit read-only validation against this PC's League installation"]
fn live_league_guide_assets() {
    assert!(game_file("v1/champion-summary.json").is_some());
    assert!(guide_art("v1/champion-icons/1.png").is_some());
}
