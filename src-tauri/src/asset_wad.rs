//! Read-only League game-asset decoding, extracted from the owner's reference.
use flate2::read::GzDecoder;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
};
use xxhash_rust::xxh64::xxh64;
const HEADER: usize = 272;
const ENTRY: usize = 32;
const MAX_ENTRIES: usize = 500_000;
const MAX_BIN: usize = 128 * 1024 * 1024;
pub fn name_hash(file: &str) -> u64 {
    xxh64(file.to_lowercase().as_bytes(), 0)
}

#[derive(Debug, Clone, Copy)]
pub struct Found {
    pub offset: u32,
    pub compressed: u32,
    pub size: u32,
    pub kind: u8,
}

pub fn unpack(raw: &[u8], kind: u8, size: u32) -> Option<Vec<u8>> {
    let size = size as usize;
    if size > MAX_BIN {
        return None;
    }
    match kind {
        0 => (raw.len() == size).then(|| raw.to_vec()),
        1 => {
            let mut out = Vec::with_capacity(size);
            GzDecoder::new(raw)
                .take(size as u64 + 1)
                .read_to_end(&mut out)
                .ok()?;
            (out.len() == size).then_some(out)
        }
        3 => {
            let out = zstd::bulk::decompress(raw, size).ok()?;
            (out.len() == size).then_some(out)
        }
        4 => chunked(raw, size),
        _ => None,
    }
}

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// Exact byte length of one RFC 8878 zstd frame starting at `start`, or 0 if incomplete.
fn zstd_frame_size(raw: &[u8], start: usize) -> usize {
    if start + 5 > raw.len() || raw[start..start + 4] != ZSTD_MAGIC {
        return 0;
    }
    let fhd = raw[start + 4];
    if fhd & 0x08 != 0 {
        return 0; // Reserved bit must be 0.
    }
    let fcs_flag = fhd >> 6;
    let single = (fhd >> 5) & 1;
    let checksum = (fhd >> 2) & 1;
    let dict_flag = fhd & 3;
    let dict_bytes = [0usize, 1, 2, 4][dict_flag as usize];
    let fcs_bytes = if fcs_flag == 0 {
        if single == 1 {
            1
        } else {
            0
        }
    } else {
        [0usize, 2, 4, 8][fcs_flag as usize]
    };
    let mut at = start + 5 + if single == 1 { 0 } else { 1 } + dict_bytes + fcs_bytes;
    if at > raw.len() {
        return 0;
    }
    for _ in 0..1_000_000 {
        if at + 3 > raw.len() {
            return 0;
        }
        let hdr = raw[at] as usize | (raw[at + 1] as usize) << 8 | (raw[at + 2] as usize) << 16;
        let last = hdr & 1;
        let kind = (hdr >> 1) & 3;
        let block_size = hdr >> 3;
        if kind == 3 {
            return 0;
        }
        at += 3 + if kind == 1 { 1 } else { block_size };
        if at > raw.len() {
            return 0;
        }
        if last == 1 {
            break;
        }
    }
    if checksum == 1 {
        at += 4;
    }
    if at <= raw.len() {
        at - start
    } else {
        0
    }
}

/// A type-4 entry that may interleave zstd frames and raw sub-chunks, decoded in O(N).
fn chunked(raw: &[u8], size: usize) -> Option<Vec<u8>> {
    if let Ok(direct) = zstd::bulk::decompress(raw, size) {
        if direct.len() == size {
            return Some(direct);
        }
    }
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut total = 0usize;
    let mut pos = 0usize;
    while pos < raw.len() && total <= size {
        let magic = raw[pos..]
            .windows(4)
            .position(|window| window == ZSTD_MAGIC)
            .map(|at| pos + at);
        let Some(magic_at) = magic else {
            let span = &raw[pos..];
            if total + span.len() > size {
                return None;
            }
            parts.push(span.to_vec());
            total += span.len();
            break;
        };
        if magic_at > pos {
            let span = &raw[pos..magic_at];
            if total + span.len() > size {
                return None;
            }
            parts.push(span.to_vec());
            total += span.len();
            pos = magic_at;
        }
        let frame_len = zstd_frame_size(raw, pos);
        let decoded = if frame_len > 0 {
            zstd::bulk::decompress(&raw[pos..pos + frame_len], size - total).ok()
        } else {
            None
        };
        if let Some(decoded) = decoded {
            if total + decoded.len() > size {
                return None;
            }
            parts.push(decoded);
            total += parts.last().map_or(0, Vec::len);
            pos += frame_len;
        } else {
            // False magic bytes inside an uncompressed raw sub-chunk: copy 4 literal bytes on.
            if total + 4 > size {
                return None;
            }
            parts.push(raw[pos..pos + 4].to_vec());
            total += 4;
            pos += 4;
        }
    }
    if total != size {
        return None;
    }
    let mut out = Vec::with_capacity(size);
    for part in parts {
        out.extend_from_slice(&part);
    }
    Some(out)
}

pub struct AssetArchive {
    handle: File,
    entries: HashMap<u64, Found>,
    table_end: u64,
    file_size: u64,
}

impl AssetArchive {
    pub fn open(path: &std::path::Path) -> Option<Self> {
        let mut handle = File::open(path).ok()?;
        let file_size = handle.metadata().ok()?.len();
        let mut header = [0u8; HEADER];
        handle.read_exact(&mut header).ok()?;
        if &header[..2] != b"RW" || header[2] != 3 {
            return None;
        }
        let count = u32::from_le_bytes(header[268..272].try_into().ok()?) as usize;
        let table_end = (HEADER + count.saturating_mul(ENTRY)) as u64;
        if count > MAX_ENTRIES || table_end > file_size {
            return None;
        }
        let mut table = vec![0; count * ENTRY];
        handle.read_exact(&mut table).ok()?;
        let mut entries = HashMap::with_capacity(count);
        for row in table.as_chunks::<ENTRY>().0 {
            entries
                .entry(u64::from_le_bytes(row[..8].try_into().ok()?))
                .or_insert(Found {
                    offset: u32::from_le_bytes(row[8..12].try_into().ok()?),
                    compressed: u32::from_le_bytes(row[12..16].try_into().ok()?),
                    size: u32::from_le_bytes(row[16..20].try_into().ok()?),
                    kind: row[20] & 15,
                });
        }
        Some(Self {
            handle,
            entries,
            table_end,
            file_size,
        })
    }

    pub fn read(&mut self, file: &str, max: u64) -> Option<Vec<u8>> {
        let found = self.entries.get(&name_hash(file))?;
        if u64::from(found.size) > max
            || found.compressed as usize > MAX_BIN
            || u64::from(found.offset) < self.table_end
            || u64::from(found.offset) + u64::from(found.compressed) > self.file_size
        {
            return None;
        }
        let mut raw = vec![0; found.compressed as usize];
        self.handle
            .seek(SeekFrom::Start(u64::from(found.offset)))
            .ok()?;
        self.handle.read_exact(&mut raw).ok()?;
        let bytes = unpack(&raw, found.kind, found.size)?;
        (bytes.len() as u64 <= max).then_some(bytes)
    }
}
