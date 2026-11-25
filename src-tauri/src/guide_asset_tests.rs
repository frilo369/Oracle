use crate::{
    asset_wad::{name_hash, AssetArchive},
    guide_assets,
};
use std::io::Write;
fn archive(files: &[(&str, &[u8], u8, u32)]) -> Vec<u8> {
    let mut bytes = vec![0; 272 + 32 * files.len()];
    bytes[..4].copy_from_slice(&[b'R', b'W', 3, 1]);
    bytes[268..272].copy_from_slice(&(files.len() as u32).to_le_bytes());
    for (i, (name, raw, kind, size)) in files.iter().enumerate() {
        let at = 272 + 32 * i;
        let offset = bytes.len() as u32;
        bytes[at..at + 8].copy_from_slice(&name_hash(name).to_le_bytes());
        bytes[at + 8..at + 12].copy_from_slice(&offset.to_le_bytes());
        bytes[at + 12..at + 16].copy_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes[at + 16..at + 20].copy_from_slice(&size.to_le_bytes());
        bytes[at + 20] = *kind;
        bytes.extend_from_slice(raw);
    }
    bytes
}
#[test]
fn asset_reader_decodes_plain_gzip_zstd_and_rejects_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("assets.wad");
    let body = b"game data";
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(body).unwrap();
    let gzip = gzip.finish().unwrap();
    let zstd = zstd::bulk::compress(body, 1).unwrap();
    let chunked = [
        zstd::bulk::compress(b"game", 1).unwrap(),
        b" ".to_vec(),
        zstd::bulk::compress(b"data", 1).unwrap(),
    ]
    .concat();
    std::fs::write(
        &path,
        archive(&[
            ("plain", body, 0, 9),
            ("gzip", &gzip, 1, 9),
            ("zstd", &zstd, 3, 9),
            ("chunked", &chunked, 4, 9),
        ]),
    )
    .unwrap();
    let mut reader = AssetArchive::open(&path).unwrap();
    for name in ["plain", "gzip", "zstd", "chunked"] {
        assert_eq!(reader.read(name, 9), Some(body.to_vec()));
        assert!(reader.read(name, 8).is_none());
    }
    assert!(reader.read("missing", 100).is_none());
    let mut broken = archive(&[("plain", body, 0, 9)]);
    broken[280..284].copy_from_slice(&0u32.to_le_bytes());
    std::fs::write(&path, broken).unwrap();
    assert!(AssetArchive::open(&path)
        .unwrap()
        .read("plain", 100)
        .is_none());
    std::fs::write(&path, b"truncated").unwrap();
    assert!(AssetArchive::open(&path).is_none());
}
#[test]
fn guide_assets_reject_traversal_and_refresh_archive_indexes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let path = root.join("default-assets.wad");
    let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1];
    let name = "plugins/rcp-be-lol-game-data/global/default/assets/icon.png";
    std::fs::write(&path, archive(&[(name, &png, 0, png.len() as u32)])).unwrap();
    let mut cache = guide_assets::AssetCache::default();
    assert_eq!(cache.read(root, &[name.into()], 100), Some(png.to_vec()));
    let mut changed = png.to_vec();
    changed.push(2);
    std::fs::write(&path, archive(&[(name, &changed, 0, changed.len() as u32)])).unwrap();
    assert_eq!(cache.read(root, &[name.into()], 100), Some(changed.clone()));
    assert!(guide_assets::guide_art_at(root, "assets/%2e%2e/icon.png").is_none());
    assert!(guide_assets::guide_art_at(root, "v1/items.json").is_none());
    assert_eq!(
        guide_assets::guide_art_at(root, "assets/icon.png"),
        Some((changed, "image/png".into()))
    );
    std::fs::remove_file(&path).unwrap();
    assert!(cache.read(root, &[name.into()], 100).is_none());
}
