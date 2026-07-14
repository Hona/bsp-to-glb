use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use zip::CompressionMethod;

const MAX_PAK_BYTES: usize = 512 * 1024 * 1024;
const MAX_PAK_ENTRIES: usize = 65_535;
const MAX_PAK_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PAK_DECODED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PAK_PATH_BYTES: usize = 1024;
const MAX_COMPRESSION_RATIO: u64 = 20_000;
const COMPRESSION_RATIO_GRACE_BYTES: u64 = 1024 * 1024;

pub const BSP_PAK_MANIFEST_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BspPakMethodInventory {
    pub code: u16,
    pub name: String,
    pub status: String,
    pub entries: usize,
    pub compressed_bytes: u64,
    pub decoded_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BspPakEntryMetadata {
    pub path: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub compression_method: u16,
    pub compression_name: String,
    pub compressed_bytes: u64,
    pub decoded_bytes: u64,
    pub crc32: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BspPakManifest {
    pub schema: String,
    pub version: u32,
    pub archive_bytes: usize,
    pub archive_content_hash: String,
    pub entry_count: usize,
    pub compressed_bytes: u64,
    pub decoded_bytes: u64,
    pub coverage: BspPakCoverage,
    pub methods: Vec<BspPakMethodInventory>,
    pub entries: Vec<BspPakEntryMetadata>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BspPakCoverage {
    pub handled: usize,
    pub inert: usize,
    pub unsupported: usize,
    pub malformed: usize,
    pub unknown: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BspPakEntry {
    pub metadata: BspPakEntryMetadata,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BspPakArchive {
    pub manifest: BspPakManifest,
    pub entries: Vec<BspPakEntry>,
}

pub(crate) fn normalize_archive_path(path: &str) -> Result<String, String> {
    if path.len() > MAX_PAK_PATH_BYTES
        || path.contains('\0')
        || path.starts_with('/')
        || path.starts_with('\\')
    {
        return Err(format!("unsafe PAK path {path:?}"));
    }
    let replaced = path.replace('\\', "/");
    let mut output = Vec::new();
    for part in replaced.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(format!("unsafe PAK path {path:?}")),
            part if part.contains(':') => return Err(format!("unsafe PAK path {path:?}")),
            part => output.push(part),
        }
    }
    let normalized = output.join("/");
    if normalized.is_empty() || normalized.len() > MAX_PAK_PATH_BYTES {
        return Err(format!("unsafe PAK path {path:?}"));
    }
    Ok(normalized)
}

#[allow(deprecated)]
fn method_code(method: CompressionMethod) -> u16 {
    method.to_u16()
}

fn source_method(method: CompressionMethod) -> Result<(u16, &'static str), String> {
    let code = method_code(method);
    match code {
        0 => Ok((code, "stored")),
        14 => Ok((code, "lzma")),
        1..=99 => Err(format!(
            "BSP PAK ZIP compression method {code} is unsupported by the TF2 archive contract"
        )),
        _ => Err(format!(
            "BSP PAK ZIP compression method {code} is unknown to the TF2 archive contract"
        )),
    }
}

fn sha256_content_id(data: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(data))
}

pub fn read_pak_archive(data: &[u8]) -> Result<BspPakArchive, String> {
    if data.is_empty() {
        return Ok(BspPakArchive {
            manifest: BspPakManifest {
                schema: "bsp-to-glb/bsp-pak".to_owned(),
                version: BSP_PAK_MANIFEST_VERSION,
                archive_bytes: 0,
                archive_content_hash: sha256_content_id(data),
                entry_count: 0,
                compressed_bytes: 0,
                decoded_bytes: 0,
                coverage: BspPakCoverage::default(),
                methods: Vec::new(),
                entries: Vec::new(),
            },
            entries: Vec::new(),
        });
    }
    if data.len() > MAX_PAK_BYTES {
        return Err(format!(
            "BSP PAK has {} bytes; limit is {MAX_PAK_BYTES}",
            data.len()
        ));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(data))
        .map_err(|error| format!("invalid BSP PAK ZIP: {error}"))?;
    if archive.len() > MAX_PAK_ENTRIES {
        return Err(format!(
            "BSP PAK has {} entries; limit is {MAX_PAK_ENTRIES}",
            archive.len()
        ));
    }

    let mut entries = Vec::new();
    let mut paths = HashMap::new();
    let mut method_totals = BTreeMap::<u16, (String, usize, u64, u64)>::new();
    let mut compressed_bytes = 0_u64;
    let mut decoded_bytes = 0_u64;
    let mut coverage = BspPakCoverage::default();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("failed to read BSP PAK entry {index}: {error}"))?;
        if file.encrypted() {
            return Err(format!("BSP PAK entry {index} is encrypted"));
        }
        let raw_path = file.name().replace('\\', "/");
        if raw_path.len() > MAX_PAK_PATH_BYTES || raw_path.contains('\0') {
            return Err(format!("unsafe PAK path {raw_path:?}"));
        }
        let (path, status, reason) = match normalize_archive_path(&raw_path) {
            Ok(path) => (path, "handled".to_owned(), None),
            Err(_) => (
                raw_path,
                "inert".to_owned(),
                Some("unsafeArchivePath".to_owned()),
            ),
        };
        let lookup = path.to_ascii_lowercase();
        if let Some(existing) = paths.insert(lookup, path.clone()) {
            return Err(format!(
                "BSP PAK has ambiguous duplicate paths {existing:?} and {path:?}"
            ));
        }
        if file.is_dir() {
            continue;
        }
        let (method, method_name) = source_method(file.compression())?;
        if file.size() > MAX_PAK_ENTRY_BYTES {
            return Err(format!(
                "BSP PAK entry {path:?} declares {} bytes; limit is {MAX_PAK_ENTRY_BYTES}",
                file.size()
            ));
        }
        compressed_bytes = compressed_bytes
            .checked_add(file.compressed_size())
            .ok_or_else(|| "BSP PAK compressed byte count overflows".to_owned())?;
        decoded_bytes = decoded_bytes
            .checked_add(file.size())
            .ok_or_else(|| "BSP PAK decoded byte count overflows".to_owned())?;
        if decoded_bytes > MAX_PAK_DECODED_BYTES {
            return Err(format!(
                "BSP PAK decoded bytes exceed the {MAX_PAK_DECODED_BYTES}-byte limit"
            ));
        }
        let ratio_limit = file
            .compressed_size()
            .checked_mul(MAX_COMPRESSION_RATIO)
            .and_then(|value| value.checked_add(COMPRESSION_RATIO_GRACE_BYTES))
            .unwrap_or(u64::MAX);
        if file.size() > ratio_limit {
            return Err(format!(
                "BSP PAK entry {path:?} exceeds the bounded compression ratio"
            ));
        }
        let capacity = usize::try_from(file.size())
            .map_err(|_| format!("BSP PAK entry {path:?} is too large for this platform"))?;
        let mut output = Vec::with_capacity(capacity);
        file.by_ref()
            .take(MAX_PAK_ENTRY_BYTES + 1)
            .read_to_end(&mut output)
            .map_err(|error| format!("failed to decode BSP PAK entry {path:?}: {error}"))?;
        if output.len() as u64 != file.size() {
            return Err(format!(
                "BSP PAK entry {path:?} decoded to {} bytes, expected {}",
                output.len(),
                file.size()
            ));
        }
        let actual_crc = crc32fast::hash(&output);
        if actual_crc != file.crc32() {
            return Err(format!(
                "BSP PAK entry {path:?} CRC mismatch: expected {:08x}, got {actual_crc:08x}",
                file.crc32()
            ));
        }
        let metadata = BspPakEntryMetadata {
            path,
            status: status.clone(),
            reason,
            compression_method: method,
            compression_name: method_name.to_owned(),
            compressed_bytes: file.compressed_size(),
            decoded_bytes: file.size(),
            crc32: format!("{actual_crc:08x}"),
            content_hash: sha256_content_id(&output),
        };
        let totals = method_totals
            .entry(method)
            .or_insert_with(|| (method_name.to_owned(), 0, 0, 0));
        totals.1 += 1;
        totals.2 += file.compressed_size();
        totals.3 += file.size();
        if status == "handled" {
            coverage.handled += 1;
        } else {
            coverage.inert += 1;
        }
        entries.push(BspPakEntry {
            metadata,
            data: output,
        });
    }
    entries.sort_by(|left, right| {
        left.metadata
            .path
            .to_ascii_lowercase()
            .cmp(&right.metadata.path.to_ascii_lowercase())
            .then_with(|| left.metadata.path.cmp(&right.metadata.path))
    });
    let manifest_entries = entries.iter().map(|entry| entry.metadata.clone()).collect();
    let methods = method_totals
        .into_iter()
        .map(
            |(code, (name, entries, compressed_bytes, decoded_bytes))| BspPakMethodInventory {
                code,
                name,
                status: "handled".to_owned(),
                entries,
                compressed_bytes,
                decoded_bytes,
            },
        )
        .collect();
    Ok(BspPakArchive {
        manifest: BspPakManifest {
            schema: "bsp-to-glb/bsp-pak".to_owned(),
            version: BSP_PAK_MANIFEST_VERSION,
            archive_bytes: data.len(),
            archive_content_hash: sha256_content_id(data),
            entry_count: entries.len(),
            compressed_bytes,
            decoded_bytes,
            coverage,
            methods,
            entries: manifest_entries,
        },
        entries,
    })
}

pub fn read_bsp_pak_archive(data: &[u8]) -> Result<BspPakArchive, String> {
    let bsp = super::parse_bsp(data)?;
    read_pak_archive(&bsp.lumps[super::LUMP_PAKFILE])
}
