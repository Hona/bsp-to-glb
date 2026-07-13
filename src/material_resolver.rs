use crate::materials::{MaterialResolver, MaterialResourceProvenance, ResolvedMaterialResource};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const VPK_SIGNATURE: u32 = 0x55aa_1234;
const VPK_EMBEDDED_ARCHIVE: u16 = 0x7fff;
const VPK_ENTRY_TERMINATOR: u16 = 0xffff;
const VPK_V1_HEADER_BYTES: u64 = 12;
const VPK_V2_HEADER_BYTES: u64 = 28;
const MAX_VMT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_VTF_BYTES: u64 = 256 * 1024 * 1024;

const HARD_MAX_MOUNTS: usize = 64;
const HARD_MAX_ENTRIES: usize = 250_000;
const HARD_MAX_INDEXED_PATH_BYTES: usize = 64 * 1024 * 1024;
const HARD_MAX_SOURCE_TREE_BYTES: usize = 32 * 1024 * 1024;
const HARD_MAX_PATH_BYTES: usize = 1024;
const HARD_MAX_REQUESTS: usize = 16_384;
const HARD_MAX_RETURNED_BYTES: u64 = 512 * 1024 * 1024;
const HARD_MAX_OPEN_CHUNKS: usize = 1;

pub const MATERIAL_MOUNT_PLAN_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialResolverLimits {
    pub max_mounts: usize,
    pub max_entries: usize,
    pub max_indexed_path_bytes: usize,
    pub max_source_tree_bytes: usize,
    pub max_path_bytes: usize,
    pub max_requests: usize,
    pub max_returned_bytes: u64,
    pub max_open_chunks: usize,
}

impl Default for MaterialResolverLimits {
    fn default() -> Self {
        Self {
            max_mounts: HARD_MAX_MOUNTS,
            max_entries: HARD_MAX_ENTRIES,
            max_indexed_path_bytes: HARD_MAX_INDEXED_PATH_BYTES,
            max_source_tree_bytes: HARD_MAX_SOURCE_TREE_BYTES,
            max_path_bytes: HARD_MAX_PATH_BYTES,
            max_requests: HARD_MAX_REQUESTS,
            max_returned_bytes: HARD_MAX_RETURNED_BYTES,
            max_open_chunks: HARD_MAX_OPEN_CHUNKS,
        }
    }
}

impl MaterialResolverLimits {
    fn validate(self) -> Result<Self, String> {
        let hard = Self::default();
        let values = [
            ("mount count", self.max_mounts, hard.max_mounts),
            ("entry count", self.max_entries, hard.max_entries),
            (
                "indexed path bytes",
                self.max_indexed_path_bytes,
                hard.max_indexed_path_bytes,
            ),
            (
                "source tree bytes",
                self.max_source_tree_bytes,
                hard.max_source_tree_bytes,
            ),
            ("path bytes", self.max_path_bytes, hard.max_path_bytes),
            ("request count", self.max_requests, hard.max_requests),
            (
                "open chunk count",
                self.max_open_chunks,
                hard.max_open_chunks,
            ),
        ];
        for (name, value, maximum) in values {
            if value == 0 || value > maximum {
                return Err(format!(
                    "material resolver {name} {value} is outside the 1..={maximum} hard limit"
                ));
            }
        }
        if self.max_returned_bytes == 0 || self.max_returned_bytes > hard.max_returned_bytes {
            return Err(format!(
                "material resolver returned-byte limit {} is outside the 1..={} hard limit",
                self.max_returned_bytes, hard.max_returned_bytes
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MaterialMountPlan {
    schema_version: u32,
    mounts: Vec<MaterialMountSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MaterialMountSpec {
    id: String,
    kind: MaterialMountKind,
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum MaterialMountKind {
    Directory,
    Vpk,
}

#[derive(Debug)]
enum IndexedSource {
    Directory {
        path: PathBuf,
    },
    Vpk {
        source_path: PathBuf,
        data_offset: u64,
        data_length: u32,
        preload: Vec<u8>,
        expected_crc32: u32,
    },
}

#[derive(Debug)]
struct IndexedAsset {
    mount_id: String,
    logical_path: String,
    source: IndexedSource,
}

#[derive(Debug, Default)]
struct ResolverState {
    requests: usize,
    returned_bytes: u64,
}

#[derive(Debug)]
pub struct MountedMaterialResolver {
    assets: HashMap<String, IndexedAsset>,
    limits: MaterialResolverLimits,
    state: Mutex<ResolverState>,
}

#[derive(Default)]
struct IndexBudget {
    entries: usize,
    path_bytes: usize,
    source_tree_bytes: usize,
}

impl IndexBudget {
    fn record(&mut self, path: &str, limits: MaterialResolverLimits) -> Result<(), String> {
        self.record_length(path.len(), limits)
    }

    fn record_length(
        &mut self,
        path_bytes: usize,
        limits: MaterialResolverLimits,
    ) -> Result<(), String> {
        if path_bytes > limits.max_path_bytes {
            return Err(format!(
                "material source path is {} bytes; limit is {}",
                path_bytes, limits.max_path_bytes
            ));
        }
        self.entries = self
            .entries
            .checked_add(1)
            .ok_or_else(|| "material source entry count overflows".to_owned())?;
        if self.entries > limits.max_entries {
            return Err(format!(
                "material mount plan has more than {} entries",
                limits.max_entries
            ));
        }
        self.path_bytes = self
            .path_bytes
            .checked_add(path_bytes)
            .ok_or_else(|| "material indexed path byte count overflows".to_owned())?;
        if self.path_bytes > limits.max_indexed_path_bytes {
            return Err(format!(
                "material mount plan exceeds {} indexed path bytes",
                limits.max_indexed_path_bytes
            ));
        }
        Ok(())
    }

    fn record_source_tree(
        &mut self,
        bytes: u64,
        limits: MaterialResolverLimits,
    ) -> Result<(), String> {
        let bytes = usize::try_from(bytes)
            .map_err(|_| "material source tree is too large for this platform".to_owned())?;
        self.source_tree_bytes = self
            .source_tree_bytes
            .checked_add(bytes)
            .ok_or_else(|| "material source tree byte count overflows".to_owned())?;
        if self.source_tree_bytes > limits.max_source_tree_bytes {
            return Err(format!(
                "material mount plan exceeds {} cumulative source tree bytes",
                limits.max_source_tree_bytes
            ));
        }
        Ok(())
    }
}

impl MountedMaterialResolver {
    pub fn from_json(data: &[u8]) -> Result<Self, String> {
        Self::from_json_with_limits(data, MaterialResolverLimits::default())
    }

    pub fn from_json_with_limits(
        data: &[u8],
        limits: MaterialResolverLimits,
    ) -> Result<Self, String> {
        Self::from_json_at(data, Path::new("."), limits)
    }

    pub fn from_json_file(path: &Path) -> Result<Self, String> {
        let limits = MaterialResolverLimits::default();
        let data = read_bounded_file(
            path,
            limits.max_source_tree_bytes as u64,
            "material mount plan",
        )?;
        let base = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        Self::from_json_at(&data, base, limits)
    }

    fn from_json_at(
        data: &[u8],
        base: &Path,
        limits: MaterialResolverLimits,
    ) -> Result<Self, String> {
        let limits = limits.validate()?;
        if data.len() > limits.max_source_tree_bytes {
            return Err(format!(
                "material mount plan is {} bytes; limit is {}",
                data.len(),
                limits.max_source_tree_bytes
            ));
        }
        let plan: MaterialMountPlan = serde_json::from_slice(data)
            .map_err(|error| format!("invalid material mount plan JSON: {error}"))?;
        if plan.schema_version != MATERIAL_MOUNT_PLAN_VERSION {
            return Err(format!(
                "unsupported material mount plan schema version {}",
                plan.schema_version
            ));
        }
        if plan.mounts.len() > limits.max_mounts {
            return Err(format!(
                "material mount plan has {} mounts; limit is {}",
                plan.mounts.len(),
                limits.max_mounts
            ));
        }

        let mut assets = HashMap::new();
        let mut mount_ids = HashSet::new();
        let mut budget = IndexBudget {
            source_tree_bytes: data.len(),
            ..IndexBudget::default()
        };
        for mount in plan.mounts {
            validate_mount_id(&mount.id)?;
            if !mount_ids.insert(mount.id.clone()) {
                return Err(format!("duplicate material mount ID {:?}", mount.id));
            }
            let mut mounted_paths = HashSet::new();
            match mount.kind {
                MaterialMountKind::Directory => index_directory(
                    &resolve_source_path(base, &mount.path),
                    &mount.id,
                    &mut mounted_paths,
                    &mut assets,
                    &mut budget,
                    limits,
                )?,
                MaterialMountKind::Vpk => index_vpk(
                    &resolve_source_path(base, &mount.path),
                    &mount.id,
                    &mut mounted_paths,
                    &mut assets,
                    &mut budget,
                    limits,
                )?,
            }
        }

        Ok(Self {
            assets,
            limits,
            state: Mutex::new(ResolverState::default()),
        })
    }
}

impl MaterialResolver for MountedMaterialResolver {
    fn resolve(&self, lookup_path: &str) -> Result<Option<ResolvedMaterialResource>, String> {
        let logical_path = normalize_request_path(lookup_path, self.limits.max_path_bytes)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "material resolver state lock is poisoned".to_owned())?;
        if state.requests >= self.limits.max_requests {
            return Err(format!(
                "material resolver request limit {} exceeded",
                self.limits.max_requests
            ));
        }
        state.requests += 1;

        let Some(asset) = self.assets.get(&logical_path) else {
            return Ok(None);
        };
        let byte_length = indexed_asset_length(asset)?;
        let returned_bytes = state
            .returned_bytes
            .checked_add(byte_length)
            .ok_or_else(|| "material resolver returned byte count overflows".to_owned())?;
        if returned_bytes > self.limits.max_returned_bytes {
            return Err(format!(
                "material resolver returned bytes would exceed limit {}",
                self.limits.max_returned_bytes
            ));
        }
        let data = match &asset.source {
            IndexedSource::Directory { path } => read_directory_asset(
                path,
                &asset.mount_id,
                &asset.logical_path,
                maximum_asset_bytes(&asset.logical_path),
            )?,
            IndexedSource::Vpk {
                source_path,
                data_offset,
                data_length,
                preload,
                expected_crc32,
            } => read_vpk_asset(
                source_path,
                *data_offset,
                *data_length,
                preload,
                *expected_crc32,
                &asset.mount_id,
                &asset.logical_path,
                maximum_asset_bytes(&asset.logical_path),
            )?,
        };
        if data.len() as u64 != byte_length {
            return Err(format!(
                "material mount {:?} entry {:?} changed length while being read",
                asset.mount_id, asset.logical_path
            ));
        }
        state.returned_bytes = returned_bytes;
        let provenance = resource_provenance(&asset.mount_id, &asset.logical_path, &data);
        Ok(Some(ResolvedMaterialResource { data, provenance }))
    }
}

fn validate_mount_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(format!(
            "invalid material mount ID {id:?}; expected 1-128 ASCII letters, digits, '.', '-' or '_'"
        ));
    }
    Ok(())
}

fn resolve_source_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

fn normalize_path(path: &str, max_path_bytes: usize) -> Result<String, String> {
    if path.len() > max_path_bytes {
        return Err(format!(
            "material path is {} bytes; limit is {max_path_bytes}",
            path.len()
        ));
    }
    if path.contains('\0') || path.starts_with('/') || path.starts_with('\\') {
        return Err(format!("unsafe material path {path:?}"));
    }
    let replaced = path.replace('\\', "/");
    let mut components = Vec::new();
    for component in replaced.split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(format!("unsafe material path {path:?}")),
            component if component.contains(':') => {
                return Err(format!("unsafe material path {path:?}"));
            }
            component => components.push(component.to_ascii_lowercase()),
        }
    }
    let normalized = components.join("/");
    if normalized.is_empty() || normalized.len() > max_path_bytes {
        return Err(format!("unsafe material path {path:?}"));
    }
    Ok(normalized)
}

fn is_material_resource(path: &str) -> bool {
    path.starts_with("materials/") && (path.ends_with(".vmt") || path.ends_with(".vtf"))
}

fn normalize_request_path(path: &str, max_path_bytes: usize) -> Result<String, String> {
    let normalized = normalize_path(path, max_path_bytes)?;
    if !is_material_resource(&normalized) {
        return Err(format!(
            "material resolver only accepts materials/**/*.vmt or materials/**/*.vtf, got {path:?}"
        ));
    }
    Ok(normalized)
}

fn maximum_asset_bytes(path: &str) -> u64 {
    if path.ends_with(".vmt") {
        MAX_VMT_BYTES
    } else {
        MAX_VTF_BYTES
    }
}

fn indexed_asset_length(asset: &IndexedAsset) -> Result<u64, String> {
    match &asset.source {
        IndexedSource::Directory { path } => {
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                format!(
                    "failed to inspect material mount {:?} entry {:?}: {error}",
                    asset.mount_id, asset.logical_path
                )
            })?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "material mount {:?} entry {:?} is no longer a non-symlink file",
                    asset.mount_id, asset.logical_path
                ));
            }
            let length = metadata.len();
            let maximum = maximum_asset_bytes(&asset.logical_path);
            if length > maximum {
                return Err(format!(
                    "material mount {:?} entry {:?} has {length} bytes; limit is {maximum}",
                    asset.mount_id, asset.logical_path
                ));
            }
            Ok(length)
        }
        IndexedSource::Vpk {
            data_length,
            preload,
            ..
        } => (preload.len() as u64)
            .checked_add(u64::from(*data_length))
            .ok_or_else(|| {
                format!(
                    "VPK mount {:?} entry {:?} length overflows",
                    asset.mount_id, asset.logical_path
                )
            }),
    }
}

fn insert_asset(
    assets: &mut HashMap<String, IndexedAsset>,
    mounted_paths: &mut HashSet<String>,
    asset: IndexedAsset,
) -> Result<(), String> {
    if !mounted_paths.insert(asset.logical_path.clone()) {
        return Err(format!(
            "material mount {:?} has ambiguous case-insensitive path {:?}",
            asset.mount_id, asset.logical_path
        ));
    }
    assets.entry(asset.logical_path.clone()).or_insert(asset);
    Ok(())
}

fn index_directory(
    root: &Path,
    mount_id: &str,
    mounted_paths: &mut HashSet<String>,
    assets: &mut HashMap<String, IndexedAsset>,
    budget: &mut IndexBudget,
    limits: MaterialResolverLimits,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("failed to inspect directory mount {mount_id:?}: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "material directory mount {mount_id:?} is not a non-symlink directory"
        ));
    }
    let Some(materials) = find_materials_directory(root, mount_id)? else {
        return Ok(());
    };
    let mut pending = vec![materials];
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            format!("failed to enumerate material directory mount {mount_id:?}: {error}")
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("failed to enumerate material directory mount {mount_id:?}: {error}")
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                format!("failed to inspect material directory mount {mount_id:?}: {error}")
            })?;
            if file_type.is_symlink() {
                return Err(format!(
                    "material directory mount {mount_id:?} contains a symlink"
                ));
            }
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("material directory mount {mount_id:?} escaped its root"))?;
            let relative = relative.to_str().ok_or_else(|| {
                format!("material directory mount {mount_id:?} contains a non-UTF-8 path")
            })?;
            let relative = relative.replace('\\', "/");
            budget.record(&relative, limits)?;
            let logical_path = normalize_path(&relative, limits.max_path_bytes)?;
            if !is_material_resource(&logical_path) {
                continue;
            }
            insert_asset(
                assets,
                mounted_paths,
                IndexedAsset {
                    mount_id: mount_id.to_owned(),
                    logical_path,
                    source: IndexedSource::Directory { path },
                },
            )?;
        }
    }
    Ok(())
}

fn find_materials_directory(root: &Path, mount_id: &str) -> Result<Option<PathBuf>, String> {
    let mut materials = None;
    for entry in fs::read_dir(root).map_err(|error| {
        format!("failed to enumerate material directory mount {mount_id:?}: {error}")
    })? {
        let entry = entry.map_err(|error| {
            format!("failed to enumerate material directory mount {mount_id:?}: {error}")
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.eq_ignore_ascii_case("materials") {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            format!("failed to inspect material directory mount {mount_id:?}: {error}")
        })?;
        if !file_type.is_dir() || file_type.is_symlink() {
            return Err(format!(
                "material directory mount {mount_id:?} has a non-directory materials entry"
            ));
        }
        if materials.replace(entry.path()).is_some() {
            return Err(format!(
                "material directory mount {mount_id:?} has ambiguous case-insensitive materials directories"
            ));
        }
    }
    Ok(materials)
}

#[derive(Clone, Copy)]
struct VpkHeader {
    header_bytes: u64,
    tree_bytes: u64,
    embedded_bytes: Option<u64>,
}

fn index_vpk(
    path: &Path,
    mount_id: &str,
    mounted_paths: &mut HashSet<String>,
    assets: &mut HashMap<String, IndexedAsset>,
    budget: &mut IndexBudget,
    limits: MaterialResolverLimits,
) -> Result<(), String> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open VPK mount {mount_id:?}: {error}"))?;
    let file_bytes = file
        .metadata()
        .map_err(|error| format!("failed to inspect VPK mount {mount_id:?}: {error}"))?
        .len();
    let header = read_vpk_header(&mut file, file_bytes, mount_id, limits)?;
    budget.record_source_tree(header.tree_bytes, limits)?;
    let tree_size = usize::try_from(header.tree_bytes)
        .map_err(|_| format!("VPK mount {mount_id:?} tree is too large for this platform"))?;
    let mut tree = vec![0; tree_size];
    file.seek(SeekFrom::Start(header.header_bytes))
        .and_then(|_| file.read_exact(&mut tree))
        .map_err(|error| format!("failed to read VPK mount {mount_id:?} tree: {error}"))?;
    let embedded_base = header
        .header_bytes
        .checked_add(header.tree_bytes)
        .ok_or_else(|| format!("VPK mount {mount_id:?} embedded offset overflows"))?;
    let embedded_bytes = header
        .embedded_bytes
        .unwrap_or_else(|| file_bytes.saturating_sub(embedded_base));

    let mut cursor = 0;
    loop {
        let extension = read_tree_string(&tree, &mut cursor, mount_id)?;
        if extension.is_empty() {
            break;
        }
        loop {
            let directory = read_tree_string(&tree, &mut cursor, mount_id)?;
            if directory.is_empty() {
                break;
            }
            loop {
                let name = read_tree_string(&tree, &mut cursor, mount_id)?;
                if name.is_empty() {
                    break;
                }
                let entry = tree.get(cursor..cursor + 18).ok_or_else(|| {
                    format!("VPK mount {mount_id:?} has a truncated directory entry")
                })?;
                cursor += 18;
                let expected_crc32 = u32::from_le_bytes(entry[0..4].try_into().unwrap());
                let preload_bytes = u16::from_le_bytes(entry[4..6].try_into().unwrap()) as usize;
                let archive_index = u16::from_le_bytes(entry[6..8].try_into().unwrap());
                let archive_offset = u32::from_le_bytes(entry[8..12].try_into().unwrap());
                let archive_length = u32::from_le_bytes(entry[12..16].try_into().unwrap());
                let terminator = u16::from_le_bytes(entry[16..18].try_into().unwrap());
                if terminator != VPK_ENTRY_TERMINATOR {
                    return Err(format!(
                        "VPK mount {mount_id:?} directory entry has invalid terminator {terminator:#06x}"
                    ));
                }
                let preload = tree
                    .get(cursor..cursor + preload_bytes)
                    .ok_or_else(|| format!("VPK mount {mount_id:?} has truncated preload data"))?;
                cursor += preload_bytes;

                let directory = if directory == " " { "" } else { directory };
                let raw_path_bytes = directory
                    .len()
                    .checked_add(usize::from(!directory.is_empty()))
                    .and_then(|length| length.checked_add(name.len()))
                    .and_then(|length| length.checked_add(1))
                    .and_then(|length| length.checked_add(extension.len()))
                    .ok_or_else(|| format!("VPK mount {mount_id:?} path length overflows"))?;
                budget.record_length(raw_path_bytes, limits)?;
                if !extension.eq_ignore_ascii_case("vmt") && !extension.eq_ignore_ascii_case("vtf")
                {
                    continue;
                }
                let raw_path = if directory.is_empty() {
                    format!("{name}.{extension}")
                } else {
                    format!("{directory}/{name}.{extension}")
                };
                let logical_path = normalize_path(&raw_path, limits.max_path_bytes)?;
                if !is_material_resource(&logical_path) {
                    continue;
                }
                let total_length = (preload.len() as u64)
                    .checked_add(u64::from(archive_length))
                    .ok_or_else(|| format!("VPK material {logical_path:?} length overflows"))?;
                let maximum = maximum_asset_bytes(&logical_path);
                if total_length > maximum {
                    return Err(format!(
                        "VPK material {logical_path:?} declares {total_length} bytes; limit is {maximum}"
                    ));
                }
                let (source_path, data_offset) = if archive_index == VPK_EMBEDDED_ARCHIVE {
                    let end = u64::from(archive_offset)
                        .checked_add(u64::from(archive_length))
                        .ok_or_else(|| {
                            format!("VPK material {logical_path:?} embedded range overflows")
                        })?;
                    if end > embedded_bytes {
                        return Err(format!(
                            "VPK material {logical_path:?} embedded range exceeds its data section"
                        ));
                    }
                    (path.to_owned(), embedded_base + u64::from(archive_offset))
                } else {
                    (
                        vpk_archive_path(path, archive_index, mount_id)?,
                        u64::from(archive_offset),
                    )
                };
                insert_asset(
                    assets,
                    mounted_paths,
                    IndexedAsset {
                        mount_id: mount_id.to_owned(),
                        logical_path,
                        source: IndexedSource::Vpk {
                            source_path,
                            data_offset,
                            data_length: archive_length,
                            preload: preload.to_vec(),
                            expected_crc32,
                        },
                    },
                )?;
            }
        }
    }
    if cursor != tree.len() {
        return Err(format!(
            "VPK mount {mount_id:?} has {} trailing tree bytes",
            tree.len() - cursor
        ));
    }
    Ok(())
}

fn read_vpk_header(
    file: &mut File,
    file_bytes: u64,
    mount_id: &str,
    limits: MaterialResolverLimits,
) -> Result<VpkHeader, String> {
    let mut common = [0_u8; VPK_V1_HEADER_BYTES as usize];
    file.read_exact(&mut common)
        .map_err(|error| format!("failed to read VPK mount {mount_id:?} header: {error}"))?;
    let signature = u32::from_le_bytes(common[0..4].try_into().unwrap());
    if signature != VPK_SIGNATURE {
        return Err(format!(
            "VPK mount {mount_id:?} has invalid signature {signature:#010x}"
        ));
    }
    let version = u32::from_le_bytes(common[4..8].try_into().unwrap());
    let tree_bytes = u64::from(u32::from_le_bytes(common[8..12].try_into().unwrap()));
    if tree_bytes > limits.max_source_tree_bytes as u64 {
        return Err(format!(
            "VPK mount {mount_id:?} tree has {tree_bytes} bytes; limit is {}",
            limits.max_source_tree_bytes
        ));
    }
    let (header_bytes, embedded_bytes, trailing_sections) = match version {
        1 => (VPK_V1_HEADER_BYTES, None, 0_u64),
        2 => {
            let mut extended = [0_u8; (VPK_V2_HEADER_BYTES - VPK_V1_HEADER_BYTES) as usize];
            file.read_exact(&mut extended).map_err(|error| {
                format!("failed to read VPK mount {mount_id:?} v2 header: {error}")
            })?;
            let embedded = u64::from(u32::from_le_bytes(extended[0..4].try_into().unwrap()));
            let trailing = [4, 8, 12]
                .into_iter()
                .map(|offset| {
                    u64::from(u32::from_le_bytes(
                        extended[offset..offset + 4].try_into().unwrap(),
                    ))
                })
                .try_fold(0_u64, |sum, value| sum.checked_add(value))
                .ok_or_else(|| format!("VPK mount {mount_id:?} section sizes overflow"))?;
            (VPK_V2_HEADER_BYTES, Some(embedded), trailing)
        }
        _ => {
            return Err(format!(
                "VPK mount {mount_id:?} has unsupported version {version}"
            ));
        }
    };
    let required = header_bytes
        .checked_add(tree_bytes)
        .and_then(|value| value.checked_add(embedded_bytes.unwrap_or(0)))
        .and_then(|value| value.checked_add(trailing_sections))
        .ok_or_else(|| format!("VPK mount {mount_id:?} section sizes overflow"))?;
    if required > file_bytes {
        return Err(format!(
            "VPK mount {mount_id:?} declares {required} bytes but file has {file_bytes}"
        ));
    }
    Ok(VpkHeader {
        header_bytes,
        tree_bytes,
        embedded_bytes,
    })
}

fn read_tree_string<'a>(
    tree: &'a [u8],
    cursor: &mut usize,
    mount_id: &str,
) -> Result<&'a str, String> {
    let remaining = tree
        .get(*cursor..)
        .ok_or_else(|| format!("VPK mount {mount_id:?} tree cursor is out of bounds"))?;
    let length = remaining
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("VPK mount {mount_id:?} has an unterminated tree string"))?;
    let value = std::str::from_utf8(&remaining[..length])
        .map_err(|error| format!("VPK mount {mount_id:?} tree path is not UTF-8: {error}"))?;
    *cursor += length + 1;
    Ok(value)
}

fn vpk_archive_path(path: &Path, archive_index: u16, mount_id: &str) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("VPK mount {mount_id:?} directory path has no UTF-8 file name"))?;
    let suffix_bytes = "_dir.vpk".len();
    if name.len() < suffix_bytes
        || !name[name.len() - suffix_bytes..].eq_ignore_ascii_case("_dir.vpk")
    {
        return Err(format!(
            "VPK mount {mount_id:?} must end in _dir.vpk to address chunk {archive_index}"
        ));
    }
    let stem = &name[..name.len() - suffix_bytes];
    Ok(path.with_file_name(format!("{stem}_{archive_index:03}.vpk")))
}

fn read_directory_asset(
    path: &Path,
    mount_id: &str,
    logical_path: &str,
    maximum: u64,
) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "material mount {mount_id:?} entry {logical_path:?} can no longer be opened: {error}"
        )
    })?;
    let length = file
        .metadata()
        .map_err(|error| {
            format!("failed to inspect material mount {mount_id:?} entry {logical_path:?}: {error}")
        })?
        .len();
    if length > maximum {
        return Err(format!(
            "material mount {mount_id:?} entry {logical_path:?} has {length} bytes; limit is {maximum}"
        ));
    }
    let mut data = Vec::with_capacity(length as usize);
    file.take(maximum + 1)
        .read_to_end(&mut data)
        .map_err(|error| {
            format!("failed to read material mount {mount_id:?} entry {logical_path:?}: {error}")
        })?;
    if data.len() as u64 != length {
        return Err(format!(
            "material mount {mount_id:?} entry {logical_path:?} changed while being read"
        ));
    }
    Ok(data)
}

#[allow(clippy::too_many_arguments)]
fn read_vpk_asset(
    source_path: &Path,
    data_offset: u64,
    data_length: u32,
    preload: &[u8],
    expected_crc32: u32,
    mount_id: &str,
    logical_path: &str,
    maximum: u64,
) -> Result<Vec<u8>, String> {
    let total_length = (preload.len() as u64)
        .checked_add(u64::from(data_length))
        .ok_or_else(|| format!("VPK mount {mount_id:?} entry {logical_path:?} length overflows"))?;
    if total_length > maximum {
        return Err(format!(
            "VPK mount {mount_id:?} entry {logical_path:?} has {total_length} bytes; limit is {maximum}"
        ));
    }
    let mut data = Vec::with_capacity(total_length as usize);
    data.extend_from_slice(preload);
    if data_length != 0 {
        let mut file = File::open(source_path).map_err(|error| {
            format!("VPK mount {mount_id:?} chunk for {logical_path:?} cannot be opened: {error}")
        })?;
        file.seek(SeekFrom::Start(data_offset)).map_err(|error| {
            format!("failed to seek VPK mount {mount_id:?} entry {logical_path:?}: {error}")
        })?;
        let start = data.len();
        data.resize(start + data_length as usize, 0);
        file.read_exact(&mut data[start..]).map_err(|error| {
            format!("failed to read VPK mount {mount_id:?} entry {logical_path:?}: {error}")
        })?;
    }
    let actual_crc32 = crc32fast::hash(&data);
    if actual_crc32 != expected_crc32 {
        return Err(format!(
            "VPK mount {mount_id:?} entry {logical_path:?} CRC mismatch: expected {expected_crc32:08x}, got {actual_crc32:08x}"
        ));
    }
    Ok(data)
}

fn read_bounded_file(path: &Path, maximum: u64, label: &str) -> Result<Vec<u8>, String> {
    let file = File::open(path)
        .map_err(|error| format!("failed to open {label} {}: {error}", path.display()))?;
    let length = file
        .metadata()
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?
        .len();
    if length > maximum {
        return Err(format!(
            "{label} {} has {length} bytes; limit is {maximum}",
            path.display()
        ));
    }
    let mut data = Vec::with_capacity(length as usize);
    file.take(maximum + 1)
        .read_to_end(&mut data)
        .map_err(|error| format!("failed to read {label} {}: {error}", path.display()))?;
    if data.len() as u64 != length {
        return Err(format!(
            "{label} {} changed while being read",
            path.display()
        ));
    }
    Ok(data)
}

fn resource_provenance(
    mount_id: &str,
    logical_path: &str,
    data: &[u8],
) -> MaterialResourceProvenance {
    MaterialResourceProvenance {
        mount_id: mount_id.to_owned(),
        path: logical_path.to_owned(),
        crc32: format!("{:08x}", crc32fast::hash(data)),
        content_hash: format!("sha256:{:x}", Sha256::digest(data)),
    }
}
