use bsp_to_glb::{
    MaterialResolver, MaterialResolverLimits, MountedMaterialResolver, PakResource,
    PakResourceKind, build_source_material_manifest,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const EMBEDDED_ARCHIVE: u16 = 0x7fff;

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("bsp-to-glb-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct VpkFixtureEntry<'a> {
    path: &'a str,
    preload: &'a [u8],
    archive: &'a [u8],
    archive_index: u16,
    corrupt_crc: bool,
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn push_cstring(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(value.as_bytes());
    output.push(0);
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_vpk(root: &Path, stem: &str, version: u32, entries: &[VpkFixtureEntry<'_>]) -> PathBuf {
    #[derive(Clone)]
    struct IndexedFixture<'a> {
        entry: VpkFixtureEntry<'a>,
        name: &'a str,
        offset: u32,
        crc: u32,
    }

    let mut archive_data = BTreeMap::<u16, Vec<u8>>::new();
    let mut grouped = BTreeMap::<&str, BTreeMap<&str, Vec<IndexedFixture<'_>>>>::new();
    for entry in entries {
        let (directory, file) = entry.path.rsplit_once('/').unwrap();
        let (name, extension) = file.rsplit_once('.').unwrap();
        let data = archive_data.entry(entry.archive_index).or_default();
        let offset = data.len() as u32;
        data.extend_from_slice(entry.archive);
        let mut complete = entry.preload.to_vec();
        complete.extend_from_slice(entry.archive);
        grouped
            .entry(extension)
            .or_default()
            .entry(directory)
            .or_default()
            .push(IndexedFixture {
                entry: entry.clone(),
                name,
                offset,
                crc: crc32(&complete),
            });
    }

    let mut tree = Vec::new();
    for (extension, paths) in grouped {
        push_cstring(&mut tree, extension);
        for (path, files) in paths {
            push_cstring(&mut tree, path);
            for file in files {
                push_cstring(&mut tree, file.name);
                tree.extend_from_slice(&file.crc.to_le_bytes());
                tree.extend_from_slice(&(file.entry.preload.len() as u16).to_le_bytes());
                tree.extend_from_slice(&file.entry.archive_index.to_le_bytes());
                tree.extend_from_slice(&file.offset.to_le_bytes());
                tree.extend_from_slice(&(file.entry.archive.len() as u32).to_le_bytes());
                tree.extend_from_slice(&0xffff_u16.to_le_bytes());
                tree.extend_from_slice(file.entry.preload);
            }
            tree.push(0);
        }
        tree.push(0);
    }
    tree.push(0);

    let embedded = archive_data.remove(&EMBEDDED_ARCHIVE).unwrap_or_default();
    let header_size = if version == 1 { 12 } else { 28 };
    let mut directory = vec![0; header_size];
    put_u32(&mut directory, 0, 0x55aa_1234);
    put_u32(&mut directory, 4, version);
    put_u32(&mut directory, 8, tree.len() as u32);
    if version == 2 {
        put_u32(&mut directory, 12, embedded.len() as u32);
    }
    directory.extend_from_slice(&tree);
    directory.extend_from_slice(&embedded);
    let directory_path = root.join(format!("{stem}_dir.vpk"));
    fs::write(&directory_path, directory).unwrap();

    for (archive_index, mut data) in archive_data {
        if entries
            .iter()
            .any(|entry| entry.archive_index == archive_index && entry.corrupt_crc)
            && let Some(byte) = data.first_mut()
        {
            *byte ^= 0xff;
        }
        fs::write(root.join(format!("{stem}_{archive_index:03}.vpk")), data).unwrap();
    }
    directory_path
}

fn plan(mounts: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "mounts": mounts,
    }))
    .unwrap()
}

fn directory_mount(id: &str, path: &Path) -> Value {
    json!({ "id": id, "kind": "directory", "path": path })
}

fn vpk_mount(id: &str, path: &Path) -> Value {
    json!({ "id": id, "kind": "vpk", "path": path })
}

#[test]
fn resolves_vpk_v1_preload_and_chunk_case_insensitively() {
    let temp = TempDirectory::new("vpk-v1");
    let vpk = write_vpk(
        temp.path(),
        "fixture",
        1,
        &[VpkFixtureEntry {
            path: "Materials/Brick/Wall.VMT",
            preload: b"preload-",
            archive: b"chunk",
            archive_index: 0,
            corrupt_crc: false,
        }],
    );
    let resolver =
        MountedMaterialResolver::from_json(&plan(vec![vpk_mount("stock", &vpk)])).unwrap();

    let resource = resolver
        .resolve(r"MATERIALS\brick\.\WALL.vmt")
        .unwrap()
        .unwrap();

    assert_eq!(resource.data, b"preload-chunk");
    assert_eq!(resource.provenance.mount_id, "stock");
    assert_eq!(resource.provenance.path, "materials/brick/wall.vmt");
    assert_eq!(
        resource.provenance.crc32,
        format!("{:08x}", crc32(&resource.data))
    );
    assert!(resource.provenance.content_hash.starts_with("sha256:"));
    assert!(
        !resource
            .provenance
            .content_hash
            .contains(&temp.path().display().to_string())
    );
}

#[test]
fn resolves_vpk_v2_preload_and_embedded_file_data() {
    let temp = TempDirectory::new("vpk-v2");
    let vpk = write_vpk(
        temp.path(),
        "fixture",
        2,
        &[VpkFixtureEntry {
            path: "materials/detail/noise.vtf",
            preload: b"abc",
            archive: b"def",
            archive_index: EMBEDDED_ARCHIVE,
            corrupt_crc: false,
        }],
    );
    let resolver =
        MountedMaterialResolver::from_json(&plan(vec![vpk_mount("textures", &vpk)])).unwrap();

    let resource = resolver
        .resolve("materials/detail/noise.vtf")
        .unwrap()
        .unwrap();

    assert_eq!(resource.data, b"abcdef");
    assert_eq!(resource.provenance.mount_id, "textures");
}

#[test]
fn first_mount_wins_across_directory_and_vpk_sources() {
    let temp = TempDirectory::new("order");
    let loose = temp.path().join("loose");
    fs::create_dir_all(loose.join("materials/brick")).unwrap();
    fs::write(loose.join("materials/brick/wall.vmt"), b"loose-first").unwrap();
    let vpk = write_vpk(
        temp.path(),
        "fixture",
        2,
        &[VpkFixtureEntry {
            path: "materials/brick/wall.vmt",
            preload: b"vpk-second",
            archive: b"",
            archive_index: EMBEDDED_ARCHIVE,
            corrupt_crc: false,
        }],
    );
    let resolver = MountedMaterialResolver::from_json(&plan(vec![
        directory_mount("override", &loose),
        vpk_mount("stock", &vpk),
    ]))
    .unwrap();

    let resource = resolver
        .resolve("materials/brick/wall.vmt")
        .unwrap()
        .unwrap();

    assert_eq!(resource.data, b"loose-first");
    assert_eq!(resource.provenance.mount_id, "override");
}

#[test]
fn corrupt_high_priority_entry_fails_without_falling_through() {
    let temp = TempDirectory::new("corrupt-order");
    let vpk = write_vpk(
        temp.path(),
        "fixture",
        2,
        &[VpkFixtureEntry {
            path: "materials/brick/wall.vmt",
            preload: b"",
            archive: b"corrupt-me",
            archive_index: 0,
            corrupt_crc: true,
        }],
    );
    let fallback = temp.path().join("fallback");
    fs::create_dir_all(fallback.join("materials/brick")).unwrap();
    fs::write(
        fallback.join("materials/brick/wall.vmt"),
        b"must-not-be-returned",
    )
    .unwrap();
    let resolver = MountedMaterialResolver::from_json(&plan(vec![
        vpk_mount("broken", &vpk),
        directory_mount("fallback", &fallback),
    ]))
    .unwrap();

    let error = resolver.resolve("materials/brick/wall.vmt").unwrap_err();

    assert!(error.contains("CRC"), "unexpected error: {error}");
    assert!(error.contains("broken"), "unexpected error: {error}");
}

#[test]
fn bsp_pak_stays_above_the_mount_plan() {
    let temp = TempDirectory::new("pak-first");
    let vpk = write_vpk(
        temp.path(),
        "fixture",
        1,
        &[VpkFixtureEntry {
            path: "materials/brick/wall.vmt",
            preload: b"invalid external VMT",
            archive: b"",
            archive_index: EMBEDDED_ARCHIVE,
            corrupt_crc: false,
        }],
    );
    let resolver =
        MountedMaterialResolver::from_json(&plan(vec![vpk_mount("stock", &vpk)])).unwrap();
    let pak = [PakResource {
        path: "materials/brick/wall.vmt".to_owned(),
        kind: PakResourceKind::Vmt,
        data: br#"LightmappedGeneric { "$basetexture" "brick/pak" }"#.to_vec(),
    }];

    let manifest =
        build_source_material_manifest(&["brick/wall".to_owned()], &pak, Some(&resolver)).unwrap();

    assert_eq!(manifest.lookup_policy, "pakFirst");
    assert_eq!(
        manifest.materials[0]
            .metadata
            .as_ref()
            .unwrap()
            .textures
            .base_texture
            .as_deref(),
        Some("brick/pak")
    );
    let provenance = serde_json::to_value(&manifest.materials[0].vmt.provenance).unwrap();
    assert_eq!(provenance["kind"], "pak");
    assert_eq!(provenance["mountId"], "bspPak");
    assert_eq!(provenance["path"], "materials/brick/wall.vmt");
    assert!(provenance["crc32"].as_str().unwrap().len() == 8);
    assert!(
        provenance["contentHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

#[test]
fn rejects_unsafe_requests_and_case_ambiguous_vpk_entries() {
    let temp = TempDirectory::new("paths");
    let empty = temp.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    let resolver =
        MountedMaterialResolver::from_json(&plan(vec![directory_mount("empty", &empty)])).unwrap();

    for path in [
        "../materials/brick/wall.vmt",
        "/materials/brick/wall.vmt",
        "C:/materials/brick/wall.vmt",
        "materials/brick/wall.txt",
        "scripts/items.txt",
    ] {
        assert!(
            resolver.resolve(path).is_err(),
            "accepted unsafe path {path:?}"
        );
    }

    let vpk = write_vpk(
        temp.path(),
        "duplicates",
        2,
        &[
            VpkFixtureEntry {
                path: "materials/brick/wall.vmt",
                preload: b"a",
                archive: b"",
                archive_index: EMBEDDED_ARCHIVE,
                corrupt_crc: false,
            },
            VpkFixtureEntry {
                path: "Materials/Brick/WALL.VMT",
                preload: b"b",
                archive: b"",
                archive_index: EMBEDDED_ARCHIVE,
                corrupt_crc: false,
            },
        ],
    );
    let error =
        MountedMaterialResolver::from_json(&plan(vec![vpk_mount("duplicate", &vpk)])).unwrap_err();
    assert!(error.contains("ambiguous"), "unexpected error: {error}");
}

#[test]
fn enforces_mount_index_path_request_and_returned_byte_budgets() {
    let temp = TempDirectory::new("budgets");
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    fs::create_dir_all(first.join("materials/a")).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(first.join("materials/a/one.vmt"), b"one").unwrap();
    fs::write(first.join("materials/a/two.vmt"), b"two").unwrap();

    let limits = MaterialResolverLimits {
        max_mounts: 1,
        ..MaterialResolverLimits::default()
    };
    let error = MountedMaterialResolver::from_json_with_limits(
        &plan(vec![
            directory_mount("first", &first),
            directory_mount("second", &second),
        ]),
        limits,
    )
    .unwrap_err();
    assert!(error.contains("mount"), "unexpected error: {error}");

    let limits = MaterialResolverLimits {
        max_entries: 1,
        ..MaterialResolverLimits::default()
    };
    let error = MountedMaterialResolver::from_json_with_limits(
        &plan(vec![directory_mount("first", &first)]),
        limits,
    )
    .unwrap_err();
    assert!(error.contains("entries"), "unexpected error: {error}");

    let limits = MaterialResolverLimits {
        max_indexed_path_bytes: 1,
        ..MaterialResolverLimits::default()
    };
    let error = MountedMaterialResolver::from_json_with_limits(
        &plan(vec![directory_mount("first", &first)]),
        limits,
    )
    .unwrap_err();
    assert!(error.contains("path bytes"), "unexpected error: {error}");

    let limits = MaterialResolverLimits {
        max_requests: 1,
        ..MaterialResolverLimits::default()
    };
    let resolver = MountedMaterialResolver::from_json_with_limits(
        &plan(vec![directory_mount("first", &first)]),
        limits,
    )
    .unwrap();
    assert!(resolver.resolve("materials/a/one.vmt").unwrap().is_some());
    let error = resolver.resolve("materials/a/missing.vmt").unwrap_err();
    assert!(error.contains("request"), "unexpected error: {error}");

    let limits = MaterialResolverLimits {
        max_returned_bytes: 5,
        ..MaterialResolverLimits::default()
    };
    let resolver = MountedMaterialResolver::from_json_with_limits(
        &plan(vec![directory_mount("first", &first)]),
        limits,
    )
    .unwrap();
    assert!(resolver.resolve("materials/a/one.vmt").unwrap().is_some());
    let error = resolver.resolve("materials/a/two.vmt").unwrap_err();
    assert!(
        error.contains("returned bytes"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_a_read_that_cannot_fit_the_remaining_budget_before_asset_io() {
    let temp = TempDirectory::new("preflight-read-budget");
    let vpk = write_vpk(
        temp.path(),
        "fixture",
        2,
        &[VpkFixtureEntry {
            path: "materials/brick/wall.vmt",
            preload: b"",
            archive: b"too-large",
            archive_index: 0,
            corrupt_crc: true,
        }],
    );
    let limits = MaterialResolverLimits {
        max_returned_bytes: 4,
        ..MaterialResolverLimits::default()
    };
    let resolver = MountedMaterialResolver::from_json_with_limits(
        &plan(vec![vpk_mount("stock", &vpk)]),
        limits,
    )
    .unwrap();

    let error = resolver.resolve("materials/brick/wall.vmt").unwrap_err();

    assert!(
        error.contains("returned bytes"),
        "read was attempted before its budget was checked: {error}"
    );
}

#[test]
fn enforces_the_source_tree_budget_across_all_mounts() {
    let temp = TempDirectory::new("cumulative-tree-budget");
    let preload = vec![b'x'; 600];
    let first = write_vpk(
        temp.path(),
        "first",
        2,
        &[VpkFixtureEntry {
            path: "materials/brick/first.vmt",
            preload: &preload,
            archive: b"",
            archive_index: EMBEDDED_ARCHIVE,
            corrupt_crc: false,
        }],
    );
    let second = write_vpk(
        temp.path(),
        "second",
        2,
        &[VpkFixtureEntry {
            path: "materials/brick/second.vmt",
            preload: &preload,
            archive: b"",
            archive_index: EMBEDDED_ARCHIVE,
            corrupt_crc: false,
        }],
    );
    let limits = MaterialResolverLimits {
        max_source_tree_bytes: 1_024,
        ..MaterialResolverLimits::default()
    };

    let error = MountedMaterialResolver::from_json_with_limits(
        &plan(vec![
            vpk_mount("first", &first),
            vpk_mount("second", &second),
        ]),
        limits,
    )
    .unwrap_err();

    assert!(
        error.contains("source tree bytes"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_limits_above_the_audited_hard_bounds() {
    assert_eq!(
        MaterialResolverLimits::default(),
        MaterialResolverLimits {
            max_mounts: 64,
            max_entries: 250_000,
            max_indexed_path_bytes: 64 * 1024 * 1024,
            max_source_tree_bytes: 32 * 1024 * 1024,
            max_path_bytes: 1024,
            max_requests: 16_384,
            max_returned_bytes: 512 * 1024 * 1024,
            max_open_chunks: 1,
        }
    );
    let limits = MaterialResolverLimits {
        max_mounts: 65,
        ..MaterialResolverLimits::default()
    };
    let error =
        MountedMaterialResolver::from_json_with_limits(&plan(Vec::new()), limits).unwrap_err();
    assert!(error.contains("hard limit"), "unexpected error: {error}");

    let mut overlong = String::from("materials/");
    overlong.push_str(&"a".repeat(1_100));
    overlong.push_str(".vmt");
    let resolver = MountedMaterialResolver::from_json(&plan(Vec::new())).unwrap();
    let error = resolver.resolve(&overlong).unwrap_err();
    assert!(error.contains("1024"), "unexpected error: {error}");
}
