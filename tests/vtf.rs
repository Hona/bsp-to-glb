use bsp_to_glb::{
    MaterialResolver, MaterialResourceProvenance, PakResource, PakResourceKind,
    ResolvedMaterialResource, TextureDecodeStatus, VtfErrorKind, VtfImageSelection,
    build_source_material_package, decode_vtf, inspect_vtf, vtf_format_universe,
};
use sha2::{Digest, Sha256};
use std::cell::RefCell;

const IMAGE_FORMAT_RGBA8888: u32 = 0;
const IMAGE_FORMAT_ABGR8888: u32 = 1;
const IMAGE_FORMAT_RGB888: u32 = 2;
const IMAGE_FORMAT_BGR888: u32 = 3;
const IMAGE_FORMAT_I8: u32 = 5;
const IMAGE_FORMAT_IA88: u32 = 6;
const IMAGE_FORMAT_A8: u32 = 8;
const IMAGE_FORMAT_BGRA8888: u32 = 12;
const IMAGE_FORMAT_DXT1: u32 = 13;
const IMAGE_FORMAT_DXT3: u32 = 14;
const IMAGE_FORMAT_DXT5: u32 = 15;

#[test]
fn inventories_the_complete_tf2_pc_image_format_universe() {
    let formats = vtf_format_universe();
    assert_eq!(formats.len(), 39);
    assert_eq!(
        formats
            .iter()
            .map(|format| format.name.as_str())
            .collect::<Vec<_>>(),
        [
            "RGBA8888",
            "ABGR8888",
            "RGB888",
            "BGR888",
            "RGB565",
            "I8",
            "IA88",
            "P8",
            "A8",
            "RGB888_BLUESCREEN",
            "BGR888_BLUESCREEN",
            "ARGB8888",
            "BGRA8888",
            "DXT1",
            "DXT3",
            "DXT5",
            "BGRX8888",
            "BGR565",
            "BGRX5551",
            "BGRA4444",
            "DXT1_ONEBITALPHA",
            "BGRA5551",
            "UV88",
            "UVWQ8888",
            "RGBA16161616F",
            "RGBA16161616",
            "UVLX8888",
            "R32F",
            "RGB323232F",
            "RGBA32323232F",
            "NV_DST16",
            "NV_DST24",
            "NV_INTZ",
            "NV_RAWZ",
            "ATI_DST16",
            "ATI_DST24",
            "NV_NULL",
            "ATI2N",
            "ATI1N",
        ]
    );
    let supported = formats
        .iter()
        .filter(|format| format.supported)
        .map(|format| format.code)
        .collect::<Vec<_>>();
    assert_eq!(
        supported,
        [
            0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 37, 38,
        ]
    );
}

fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn vtf_72(
    width: u16,
    height: u16,
    format: u32,
    frames: u16,
    mip_count: u8,
    flags: u32,
    high_resolution_data: &[u8],
) -> Vec<u8> {
    let mut data = vec![0; 65];
    data[0..4].copy_from_slice(b"VTF\0");
    put_u32(&mut data, 4, 7);
    put_u32(&mut data, 8, 2);
    put_u32(&mut data, 12, 65);
    put_u16(&mut data, 16, width);
    put_u16(&mut data, 18, height);
    put_u32(&mut data, 20, flags);
    put_u16(&mut data, 24, frames);
    put_u32(&mut data, 52, format);
    data[56] = mip_count;
    put_u32(&mut data, 57, u32::MAX);
    put_u16(&mut data, 63, 1);
    data.extend_from_slice(high_resolution_data);
    data
}

fn vtf_73(width: u16, height: u16, format: u32, high_resolution_data: &[u8]) -> Vec<u8> {
    let header_size = 88_u32;
    let mut data = vec![0; header_size as usize];
    data[0..4].copy_from_slice(b"VTF\0");
    put_u32(&mut data, 4, 7);
    put_u32(&mut data, 8, 3);
    put_u32(&mut data, 12, header_size);
    put_u16(&mut data, 16, width);
    put_u16(&mut data, 18, height);
    put_u16(&mut data, 24, 1);
    put_u32(&mut data, 52, format);
    data[56] = 1;
    put_u32(&mut data, 57, u32::MAX);
    put_u16(&mut data, 63, 1);
    put_u32(&mut data, 68, 1);
    data[80..83].copy_from_slice(&[0x30, 0, 0]);
    put_u32(&mut data, 84, header_size);
    data.extend_from_slice(high_resolution_data);
    data
}

fn vtf_73_with_inline_resource(
    width: u16,
    height: u16,
    format: u32,
    tag: [u8; 3],
    inline_data: u32,
    high_resolution_data: &[u8],
) -> Vec<u8> {
    let header_size = 96_u32;
    let mut data = vec![0; header_size as usize];
    data[0..4].copy_from_slice(b"VTF\0");
    put_u32(&mut data, 4, 7);
    put_u32(&mut data, 8, 3);
    put_u32(&mut data, 12, header_size);
    put_u16(&mut data, 16, width);
    put_u16(&mut data, 18, height);
    put_u16(&mut data, 24, 1);
    put_u32(&mut data, 52, format);
    data[56] = 1;
    put_u32(&mut data, 57, u32::MAX);
    put_u16(&mut data, 63, 1);
    put_u32(&mut data, 68, 2);
    data[80..83].copy_from_slice(&[0x30, 0, 0]);
    put_u32(&mut data, 84, header_size);
    data[88..91].copy_from_slice(&tag);
    data[91] = 0x02;
    put_u32(&mut data, 92, inline_data);
    data.extend_from_slice(high_resolution_data);
    data
}

fn rgba_vtf(pixel: [u8; 4]) -> Vec<u8> {
    vtf_72(1, 1, IMAGE_FORMAT_RGBA8888, 1, 1, 0, &pixel)
}

#[test]
fn accepts_tf2_vtf_versions_70_through_75_and_versioned_cubemap_faces() {
    for minor in 0..=5 {
        let mut texture = if minor >= 3 {
            vtf_73(1, 1, IMAGE_FORMAT_RGBA8888, &[1, 2, 3, 4])
        } else {
            vtf_72(1, 1, IMAGE_FORMAT_RGBA8888, 1, 1, 0, &[1, 2, 3, 4])
        };
        put_u32(&mut texture, 8, minor);
        let metadata = inspect_vtf(&texture).unwrap();
        assert_eq!((metadata.version_major, metadata.version_minor), (7, minor));
    }

    let mut six_face = vtf_73(1, 1, IMAGE_FORMAT_RGBA8888, &[0; 6 * 4]);
    put_u32(&mut six_face, 8, 4);
    put_u32(&mut six_face, 20, 0x4000);
    put_u16(&mut six_face, 26, u16::MAX);
    assert_eq!(inspect_vtf(&six_face).unwrap().faces, 6);

    let mut sphere_map = vtf_73(1, 1, IMAGE_FORMAT_RGBA8888, &[0; 7 * 4]);
    put_u32(&mut sphere_map, 8, 4);
    put_u32(&mut sphere_map, 20, 0x4000);
    assert_eq!(inspect_vtf(&sphere_map).unwrap().faces, 7);

    let mut current = vtf_73(1, 1, IMAGE_FORMAT_RGBA8888, &[0; 6 * 4]);
    put_u32(&mut current, 8, 5);
    put_u32(&mut current, 20, 0x4000);
    assert_eq!(inspect_vtf(&current).unwrap().faces, 6);
}

#[test]
fn decodes_required_uncompressed_and_luminance_formats_to_rgba() {
    let cases: &[(u32, &[u8], [u8; 4], &str)] = &[
        (
            IMAGE_FORMAT_RGBA8888,
            &[1, 2, 3, 4],
            [1, 2, 3, 4],
            "RGBA8888",
        ),
        (
            IMAGE_FORMAT_ABGR8888,
            &[4, 3, 2, 1],
            [1, 2, 3, 4],
            "ABGR8888",
        ),
        (IMAGE_FORMAT_RGB888, &[1, 2, 3], [1, 2, 3, 255], "RGB888"),
        (IMAGE_FORMAT_BGR888, &[3, 2, 1], [1, 2, 3, 255], "BGR888"),
        (
            IMAGE_FORMAT_BGRA8888,
            &[3, 2, 1, 4],
            [1, 2, 3, 4],
            "BGRA8888",
        ),
        (IMAGE_FORMAT_I8, &[42], [42, 42, 42, 255], "I8"),
        (IMAGE_FORMAT_IA88, &[42, 9], [42, 42, 42, 9], "IA88"),
        (IMAGE_FORMAT_A8, &[9], [9, 9, 9, 9], "A8"),
    ];

    for &(format, encoded, expected, name) in cases {
        let decoded = decode_vtf(
            &vtf_72(1, 1, format, 1, 1, 0, encoded),
            VtfImageSelection::default(),
        )
        .unwrap();
        assert_eq!(decoded.pixels, expected, "format {name}");
        assert_eq!(decoded.metadata.format.name, name);
        assert!(decoded.metadata.format.supported);
    }
}

#[test]
fn decodes_packed_color_and_data_map_formats_with_source_channel_semantics() {
    let cases: &[(u32, &[u8], [u8; 4])] = &[
        (9, &[0, 0, 255], [0, 0, 0, 0]),
        (10, &[255, 0, 0], [0, 0, 0, 0]),
        (11, &[4, 1, 2, 3], [1, 2, 3, 4]),
        (4, &[0x1f, 0x00], [255, 0, 0, 255]),
        (16, &[3, 2, 1, 99], [1, 2, 3, 255]),
        (17, &[0x00, 0xf8], [255, 0, 0, 255]),
        (18, &[0x00, 0x7c], [255, 0, 0, 255]),
        (19, &[0x81, 0xff], [240, 128, 16, 240]),
        (21, &[0x00, 0xfc], [255, 0, 0, 255]),
        (22, &[1, 2], [1, 2, 0, 0]),
        (23, &[1, 2, 3, 4], [1, 2, 3, 4]),
        (
            25,
            &[0xf0, 0x0f, 0x00, 0x08, 0x00, 0x00, 0xff, 0xff],
            [255, 128, 0, 255],
        ),
        (26, &[5, 6, 7, 8], [5, 6, 7, 8]),
        (
            24,
            &[0x00, 0x3c, 0x00, 0x38, 0x00, 0x40, 0x00, 0x00],
            [255, 128, 255, 0],
        ),
    ];
    for &(format, encoded, expected) in cases {
        let decoded = decode_vtf(
            &vtf_72(1, 1, format, 1, 1, 0, encoded),
            VtfImageSelection::default(),
        )
        .unwrap();
        assert_eq!(decoded.pixels, expected, "format {format}");
    }
}

#[test]
fn decodes_ati_normal_data_blocks_without_inventing_color_channels() {
    let red = [10, 20, 0x3e, 0, 0, 0, 0, 0];
    let green = [30, 40, 0, 0, 0, 0, 0, 0];
    let mut ati2 = red.to_vec();
    ati2.extend_from_slice(&green);

    let decoded = decode_vtf(
        &vtf_72(4, 4, 37, 1, 1, 0, &ati2),
        VtfImageSelection::default(),
    )
    .unwrap();
    assert_eq!(&decoded.pixels[0..8], &[0, 30, 0, 0, 255, 30, 0, 0]);

    let decoded = decode_vtf(
        &vtf_72(4, 4, 38, 1, 1, 0, &red),
        VtfImageSelection::default(),
    )
    .unwrap();
    assert_eq!(&decoded.pixels[0..8], &[0, 0, 0, 0, 255, 0, 0, 0]);
}

#[test]
fn inventories_inline_resources_without_treating_their_values_as_offsets() {
    let texture = vtf_73_with_inline_resource(
        1,
        1,
        IMAGE_FORMAT_RGBA8888,
        *b"CRC",
        0xfedc_ba98,
        &[1, 2, 3, 4],
    );

    let metadata = inspect_vtf(&texture).unwrap();

    assert_eq!(metadata.resources.len(), 2);
    assert_eq!(metadata.resources[0].tag, "0x300000");
    assert!(!metadata.resources[0].is_inline);
    assert_eq!(metadata.resources[1].tag, "CRC");
    assert!(metadata.resources[1].is_inline);
    assert_eq!(metadata.resources[1].inline_data, Some(0xfedc_ba98));
}

#[test]
fn selects_individual_volume_slices_in_source_storage_order() {
    let mut texture = vtf_72(
        1,
        1,
        IMAGE_FORMAT_RGBA8888,
        1,
        1,
        0,
        &[1, 0, 0, 255, 2, 0, 0, 255],
    );
    put_u16(&mut texture, 63, 2);

    let decoded = decode_vtf(
        &texture,
        VtfImageSelection {
            slice: 1,
            ..VtfImageSelection::default()
        },
    )
    .unwrap();

    assert_eq!(decoded.pixels, [2, 0, 0, 255]);
}

#[test]
fn decodes_dxt1_four_color_transparency_and_cropped_blocks() {
    let four_color = [
        0x00, 0xf8, 0xe0, 0x07, // red, green
        0xe4, 0xe4, 0xe4, 0xe4, // indices 0, 1, 2, 3 per row
    ];
    let decoded = decode_vtf(
        &vtf_72(4, 4, IMAGE_FORMAT_DXT1, 1, 1, 0, &four_color),
        VtfImageSelection::default(),
    )
    .unwrap();
    assert_eq!(
        &decoded.pixels[0..16],
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 170, 85, 0, 255, 85, 170, 0, 255
        ]
    );

    let transparent = [
        0x00, 0x00, 0xff, 0xff, // black <= white enables one-bit alpha
        0xff, 0xff, 0xff, 0xff, // index 3
    ];
    let decoded = decode_vtf(
        &vtf_72(3, 2, IMAGE_FORMAT_DXT1, 1, 1, 0, &transparent),
        VtfImageSelection::default(),
    )
    .unwrap();
    assert_eq!((decoded.width, decoded.height), (3, 2));
    assert_eq!(decoded.pixels, vec![0; 3 * 2 * 4]);
}

#[test]
fn decodes_dxt3_and_dxt5_alpha_blocks() {
    let dxt3 = [
        0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe, // alpha nibbles 0..15
        0x00, 0xf8, 0x00, 0xf8, 0, 0, 0, 0, // red color
    ];
    let decoded = decode_vtf(
        &vtf_72(4, 4, IMAGE_FORMAT_DXT3, 1, 1, 0, &dxt3),
        VtfImageSelection::default(),
    )
    .unwrap();
    assert_eq!(decoded.pixels[0..8], [255, 0, 0, 0, 255, 0, 0, 17]);
    assert_eq!(decoded.pixels[15 * 4 + 3], 255);

    let dxt5 = [
        10, 20, // six-alpha mode: indices 6 and 7 are 0 and 255
        0x3e, 0, 0, 0, 0, 0, // texel 0 index 6, texel 1 index 7
        0xe0, 0x07, 0xe0, 0x07, 0, 0, 0, 0, // green color
    ];
    let decoded = decode_vtf(
        &vtf_72(4, 4, IMAGE_FORMAT_DXT5, 1, 1, 0, &dxt5),
        VtfImageSelection::default(),
    )
    .unwrap();
    assert_eq!(decoded.pixels[0..8], [0, 255, 0, 0, 0, 255, 0, 255]);
}

#[test]
fn selects_mips_frames_and_cubemap_faces_in_source_storage_order() {
    let mut mip_data = Vec::new();
    mip_data.extend_from_slice(&[1, 2, 3, 4]);
    mip_data.extend_from_slice(&[5, 6, 7, 8]);
    mip_data.extend_from_slice(&[[10, 0, 0, 255]; 4].concat());
    mip_data.extend_from_slice(&[[20, 0, 0, 255]; 4].concat());
    let texture = vtf_72(2, 2, IMAGE_FORMAT_RGBA8888, 2, 2, 0, &mip_data);
    let decoded = decode_vtf(
        &texture,
        VtfImageSelection {
            mip: 0,
            frame: 1,
            face: 0,
            slice: 0,
        },
    )
    .unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 2));
    assert_eq!(decoded.pixels, [[20, 0, 0, 255]; 4].concat());

    let faces = (0_u8..7)
        .flat_map(|face| [face, 0, 0, 255])
        .collect::<Vec<_>>();
    let cubemap = vtf_72(1, 1, IMAGE_FORMAT_RGBA8888, 1, 1, 0x4000, &faces);
    let metadata = inspect_vtf(&cubemap).unwrap();
    assert_eq!(metadata.faces, 7);
    let decoded = decode_vtf(
        &cubemap,
        VtfImageSelection {
            face: 6,
            ..VtfImageSelection::default()
        },
    )
    .unwrap();
    assert_eq!(decoded.pixels, [6, 0, 0, 255]);
}

#[test]
fn reads_v73_resource_offsets_and_rejects_invalid_or_unsupported_inputs() {
    let decoded = decode_vtf(
        &vtf_73(1, 1, IMAGE_FORMAT_RGBA8888, &[9, 8, 7, 6]),
        VtfImageSelection::default(),
    )
    .unwrap();
    assert_eq!(decoded.pixels, [9, 8, 7, 6]);

    let mut bad_resource_offset = vtf_73(1, 1, IMAGE_FORMAT_RGBA8888, &[0; 4]);
    put_u32(&mut bad_resource_offset, 84, 1);
    let error = inspect_vtf(&bad_resource_offset).unwrap_err();
    assert_eq!(error.kind, VtfErrorKind::Invalid);
    assert!(error.message.contains("resource 0 offset"), "{error}");

    let truncated = vtf_72(4, 4, IMAGE_FORMAT_RGBA8888, 1, 1, 0, &[0; 63]);
    let error = decode_vtf(&truncated, VtfImageSelection::default()).unwrap_err();
    assert_eq!(error.kind, VtfErrorKind::Invalid);
    assert!(error.message.contains("truncated"), "{error}");

    let unsupported = vtf_72(1, 1, 7, 1, 1, 0, &[0; 1]);
    let metadata = inspect_vtf(&unsupported).unwrap();
    assert_eq!(metadata.format.name, "P8");
    assert!(!metadata.format.supported);
    let error = decode_vtf(&unsupported, VtfImageSelection::default()).unwrap_err();
    assert_eq!(error.kind, VtfErrorKind::Unsupported);
    assert!(error.message.contains("P8"), "{error}");

    let impossible_mips = vtf_72(1, 1, IMAGE_FORMAT_RGBA8888, 1, 2, 0, &[0; 8]);
    let error = inspect_vtf(&impossible_mips).unwrap_err();
    assert_eq!(error.kind, VtfErrorKind::Invalid);
    assert!(error.message.contains("mip count"), "{error}");

    let selection_error = decode_vtf(
        &rgba_vtf([0; 4]),
        VtfImageSelection {
            frame: 1,
            ..VtfImageSelection::default()
        },
    )
    .unwrap_err();
    assert_eq!(selection_error.kind, VtfErrorKind::Invalid);
    assert!(
        selection_error.message.contains("frame"),
        "{selection_error}"
    );
}

struct Resolver {
    requests: RefCell<Vec<String>>,
}

impl MaterialResolver for Resolver {
    fn resolve(&self, lookup_path: &str) -> Result<Option<ResolvedMaterialResource>, String> {
        self.requests.borrow_mut().push(lookup_path.to_owned());
        let data = match lookup_path {
            "materials/shared/external.vtf" => rgba_vtf([7, 8, 9, 255]),
            "materials/shared/unsupported.vtf" => vtf_72(1, 1, 7, 1, 1, 0, &[0; 1]),
            _ => return Ok(None),
        };
        Ok(Some(ResolvedMaterialResource {
            provenance: MaterialResourceProvenance {
                mount_id: "fixture".to_owned(),
                path: lookup_path.to_owned(),
                crc32: format!("{:08x}", crc32fast::hash(&data)),
                content_hash: format!("sha256:{:x}", Sha256::digest(&data)),
            },
            data,
        }))
    }
}

#[test]
fn material_package_is_pak_first_content_addressed_and_preserves_failures() {
    let vmt = br#"LightmappedGeneric {
        "$basetexture" "shared/pak"
        "$detail" "shared/external"
        "$bumpmap" "shared/unsupported"
        Proxies { TextureScroll { texturescrollvar "$basetexturetransform" } }
    }"#;
    let resources = vec![
        PakResource {
            path: "materials/shared/test.vmt".to_owned(),
            kind: PakResourceKind::Vmt,
            data: vmt.to_vec(),
        },
        PakResource {
            path: "materials/shared/pak.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: rgba_vtf([7, 8, 9, 255]),
        },
    ];
    let resolver = Resolver {
        requests: RefCell::new(Vec::new()),
    };

    let package = build_source_material_package(
        &["shared/test".to_owned()],
        &resources,
        Some(&resolver),
        VtfImageSelection::default(),
    )
    .unwrap();

    assert_eq!(
        package.artifacts.len(),
        1,
        "identical decoded pixels deduplicate"
    );
    let artifact = &package.artifacts[0];
    assert!(artifact.content_id.starts_with("sha256:"));
    assert!(artifact.file_name.starts_with("sha256-"));
    assert!(artifact.file_name.ends_with(".png"));
    assert!(artifact.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(
        artifact.content_id,
        format!("sha256:{:x}", Sha256::digest(&artifact.png))
    );
    assert_eq!(package.manifest.sources.len(), 3);
    let pak = package
        .manifest
        .sources
        .iter()
        .find(|source| source.lookup_path == "materials/shared/pak.vtf")
        .unwrap();
    let external = package
        .manifest
        .sources
        .iter()
        .find(|source| source.lookup_path == "materials/shared/external.vtf")
        .unwrap();
    let unsupported = package
        .manifest
        .sources
        .iter()
        .find(|source| source.lookup_path == "materials/shared/unsupported.vtf")
        .unwrap();
    assert_eq!(pak.status, TextureDecodeStatus::Decoded);
    assert_eq!(external.status, TextureDecodeStatus::Decoded);
    assert_eq!(
        pak.output.as_ref().unwrap().content_id,
        external.output.as_ref().unwrap().content_id
    );
    assert_eq!(unsupported.status, TextureDecodeStatus::Unsupported);
    assert!(unsupported.error.as_deref().unwrap().contains("P8"));
    assert_eq!(package.material_manifest.schema_version, 3);
    assert_eq!(
        package.material_manifest.materials[0]
            .metadata
            .as_ref()
            .unwrap()
            .unsupported
            .proxies,
        ["TextureScroll"]
    );
    assert_eq!(
        package.material_manifest.materials[0].textures[0].package_source_index,
        Some(0)
    );
    assert_eq!(
        resolver.requests.into_inner(),
        [
            "materials/shared/unsupported.vtf",
            "materials/shared/external.vtf"
        ]
    );
}

#[test]
fn material_package_emits_every_animated_frame_from_the_canonical_decoder() {
    let vmt = br#"LightmappedGeneric { "$basetexture" "shared/animated" }"#;
    let texture = vtf_72(
        1,
        1,
        IMAGE_FORMAT_RGBA8888,
        2,
        1,
        0,
        &[1, 2, 3, 255, 4, 5, 6, 255],
    );
    let resources = vec![
        PakResource {
            path: "materials/shared/test.vmt".to_owned(),
            kind: PakResourceKind::Vmt,
            data: vmt.to_vec(),
        },
        PakResource {
            path: "materials/shared/animated.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: texture,
        },
    ];

    let package = build_source_material_package(
        &["shared/test".to_owned()],
        &resources,
        None,
        VtfImageSelection::default(),
    )
    .unwrap();

    assert_eq!(package.manifest.sources[0].frame_outputs.len(), 2);
    assert_eq!(
        package.manifest.sources[0].strict_subresource_outputs.len(),
        2
    );
    assert_eq!(
        package.manifest.sources[0].strict_subresource_outputs[0].frame,
        0
    );
    assert_eq!(
        package.manifest.sources[0].strict_subresource_outputs[1].frame,
        1
    );
    assert_eq!(package.artifacts.len(), 2);
    assert_eq!(
        package.manifest.sources[0].output,
        Some(package.manifest.sources[0].frame_outputs[0].clone())
    );
}

#[test]
fn material_package_emits_every_mip_face_and_slice_with_exact_semantics() {
    let vmt = br#"WorldVertexTransition {
        "$basetexture" "shared/color"
        "$basetexture2" "shared/color2"
        "$bumpmap" "shared/normal"
        "$bumpmap2" "shared/normal2"
        "$blendmodulatetexture" "shared/blend"
        "$envmapmask" "shared/mask"
        "$flowmap" "shared/flow"
    }"#;
    let mut cubemap = vtf_72(
        2,
        2,
        IMAGE_FORMAT_RGBA8888,
        1,
        2,
        0x4000,
        &[0; (6 + 6 * 4) * 4],
    );
    put_u16(&mut cubemap, 26, u16::MAX);
    let resources = [
        PakResource {
            path: "materials/shared/test.vmt".to_owned(),
            kind: PakResourceKind::Vmt,
            data: vmt.to_vec(),
        },
        PakResource {
            path: "materials/shared/color.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: cubemap,
        },
        PakResource {
            path: "materials/shared/color2.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: rgba_vtf([1, 2, 3, 255]),
        },
        PakResource {
            path: "materials/shared/normal.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: rgba_vtf([1, 2, 3, 255]),
        },
        PakResource {
            path: "materials/shared/normal2.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: rgba_vtf([1, 2, 3, 255]),
        },
        PakResource {
            path: "materials/shared/blend.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: rgba_vtf([1, 2, 3, 255]),
        },
        PakResource {
            path: "materials/shared/mask.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: rgba_vtf([1, 2, 3, 255]),
        },
        PakResource {
            path: "materials/shared/flow.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: rgba_vtf([1, 2, 3, 255]),
        },
    ];

    let package = build_source_material_package(
        &["shared/test".to_owned()],
        &resources,
        None,
        VtfImageSelection::default(),
    )
    .unwrap();

    let material = &package.material_manifest.materials[0];
    assert_eq!(
        material
            .textures
            .iter()
            .map(|texture| (
                texture.role.as_str(),
                texture.parameter.as_str(),
                texture.semantic
            ))
            .collect::<Vec<_>>(),
        [
            (
                "baseTexture",
                "$basetexture",
                bsp_to_glb::TextureSemantic::Color
            ),
            (
                "baseTexture2",
                "$basetexture2",
                bsp_to_glb::TextureSemantic::Color
            ),
            ("bumpMap", "$bumpmap", bsp_to_glb::TextureSemantic::Normal),
            ("bumpMap2", "$bumpmap2", bsp_to_glb::TextureSemantic::Normal),
            (
                "blendModulateTexture",
                "$blendmodulatetexture",
                bsp_to_glb::TextureSemantic::Mask,
            ),
            (
                "envMapMask",
                "$envmapmask",
                bsp_to_glb::TextureSemantic::Mask
            ),
            ("flowMap", "$flowmap", bsp_to_glb::TextureSemantic::Flow),
        ]
    );
    let cubemap = &package.manifest.sources[0];
    assert_eq!(cubemap.metadata.as_ref().unwrap().faces, 6);
    assert_eq!(cubemap.strict_subresource_outputs.len(), 12);
    assert!(
        cubemap
            .strict_subresource_outputs
            .iter()
            .any(|entry| entry.mip == 1)
    );
    assert!(
        cubemap
            .strict_subresource_outputs
            .iter()
            .any(|entry| entry.face == 5)
    );
}

#[test]
fn material_package_reports_animation_budgets_without_partial_outputs() {
    let vmt = br#"LightmappedGeneric { "$basetexture" "shared/too_many" }"#;
    let texture = vtf_72(1, 1, IMAGE_FORMAT_RGBA8888, 257, 1, 0, &vec![0; 257 * 4]);
    let resources = vec![
        PakResource {
            path: "materials/shared/test.vmt".to_owned(),
            kind: PakResourceKind::Vmt,
            data: vmt.to_vec(),
        },
        PakResource {
            path: "materials/shared/too_many.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: texture,
        },
    ];

    let package = build_source_material_package(
        &["shared/test".to_owned()],
        &resources,
        None,
        VtfImageSelection::default(),
    )
    .unwrap();

    assert_eq!(
        package.manifest.sources[0].status,
        TextureDecodeStatus::Unsupported
    );
    assert!(package.manifest.sources[0].frame_outputs.is_empty());
    assert!(package.manifest.sources[0].output.is_none());
    assert!(package.artifacts.is_empty());
}

#[test]
fn material_package_rejects_an_invalid_selected_frame_without_partial_outputs() {
    let resources = vec![
        PakResource {
            path: "materials/shared/test.vmt".to_owned(),
            kind: PakResourceKind::Vmt,
            data: br#"LightmappedGeneric { "$basetexture" "shared/animated" }"#.to_vec(),
        },
        PakResource {
            path: "materials/shared/animated.vtf".to_owned(),
            kind: PakResourceKind::Vtf,
            data: vtf_72(
                1,
                1,
                IMAGE_FORMAT_RGBA8888,
                2,
                1,
                0,
                &[1, 2, 3, 255, 4, 5, 6, 255],
            ),
        },
    ];

    let package = build_source_material_package(
        &["shared/test".to_owned()],
        &resources,
        None,
        VtfImageSelection {
            frame: 2,
            ..VtfImageSelection::default()
        },
    )
    .unwrap();

    assert_eq!(
        package.manifest.sources[0].status,
        TextureDecodeStatus::Invalid
    );
    assert!(package.manifest.sources[0].frame_outputs.is_empty());
    assert!(package.manifest.outputs.is_empty());
    assert!(package.artifacts.is_empty());
}
