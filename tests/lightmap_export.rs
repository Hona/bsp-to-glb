use bsp_to_glb::{ExportOptions, LightmapSet, encode_lightmap_png, export_bsp_with_options};
use serde_json::Value;
use std::fs;
use std::process::Command;

const HEADER_SIZE: usize = 4 + 4 + 64 * 16 + 4;

fn put_i16(data: &mut [u8], offset: usize, value: i16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_i32(data: &mut [u8], offset: usize, value: i32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_f32(data: &mut [u8], offset: usize, value: f32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn sample(r: u8, g: u8, b: u8, exponent: i8) -> [u8; 4] {
    [r, g, b, exponent as u8]
}

fn lighting(seed: u8, bumped: bool, styles: usize) -> Vec<u8> {
    let maps = if bumped { 4 } else { 1 };
    let mut data = Vec::new();
    for style in 0..styles {
        for map in 0..maps {
            for luxel in 0..4 {
                data.extend_from_slice(&sample(
                    seed + style as u8 * 40 + map as u8 * 10 + luxel,
                    2,
                    3,
                    -1,
                ));
            }
        }
    }
    data
}

fn synthetic_bsp(
    ldr: Option<Vec<u8>>,
    hdr: Option<Vec<u8>>,
    bumped: bool,
    styles: [u8; 4],
    light_offset: i32,
    extra_flags: i32,
) -> Vec<u8> {
    let mut lumps = vec![Vec::<u8>::new(); 64];
    lumps[0] = b"{\n\"classname\" \"worldspawn\"\n}\n".to_vec();

    let mut planes = vec![0; 20];
    put_f32(&mut planes, 8, 1.0);
    lumps[1] = planes;

    let mut texdata = vec![0; 32];
    put_i32(&mut texdata, 12, 0);
    put_i32(&mut texdata, 16, 16);
    put_i32(&mut texdata, 20, 16);
    lumps[2] = texdata;

    let positions = [
        [0.0, 0.0, 0.0],
        [16.0, 0.0, 0.0],
        [16.0, 16.0, 0.0],
        [0.0, 16.0, 0.0],
    ];
    let mut vertices = vec![0; positions.len() * 12];
    for (index, position) in positions.iter().enumerate() {
        for (axis, value) in position.iter().enumerate() {
            put_f32(&mut vertices, index * 12 + axis * 4, *value);
        }
    }
    lumps[3] = vertices;

    let mut texinfo = vec![0; 72];
    put_f32(&mut texinfo, 0, 1.0);
    put_f32(&mut texinfo, 20, 1.0);
    put_f32(&mut texinfo, 32, 1.0 / 16.0);
    put_f32(&mut texinfo, 52, 1.0 / 16.0);
    put_i32(
        &mut texinfo,
        64,
        extra_flags | if bumped { 0x0800 } else { 0 },
    );
    put_i32(&mut texinfo, 68, 0);
    lumps[6] = texinfo;

    let mut face = vec![0; 56];
    put_u16(&mut face, 0, 0);
    put_i32(&mut face, 4, 0);
    put_i16(&mut face, 8, 4);
    put_i16(&mut face, 10, 0);
    put_i16(&mut face, 12, -1);
    face[16..20].copy_from_slice(&styles);
    put_i32(&mut face, 20, light_offset);
    put_i32(&mut face, 28, 0);
    put_i32(&mut face, 32, 0);
    put_i32(&mut face, 36, 1);
    put_i32(&mut face, 40, 1);
    if ldr.is_some() {
        lumps[7] = face.clone();
    }
    if hdr.is_some() {
        lumps[58] = face;
    }
    if let Some(ldr) = ldr {
        lumps[8] = ldr;
    }
    if let Some(hdr) = hdr {
        lumps[53] = hdr;
    }

    let mut edges = vec![0; 4 * 4];
    for edge in 0..4 {
        put_u16(&mut edges, edge * 4, edge as u16);
        put_u16(&mut edges, edge * 4 + 2, ((edge + 1) % 4) as u16);
    }
    lumps[12] = edges;

    let mut surfedges = vec![0; 4 * 4];
    for edge in 0..4 {
        put_i32(&mut surfedges, edge * 4, edge as i32);
    }
    lumps[13] = surfedges;

    let mut model = vec![0; 48];
    put_i32(&mut model, 40, 0);
    put_i32(&mut model, 44, 1);
    lumps[14] = model;
    lumps[43] = b"brick/test\0".to_vec();
    lumps[44] = 0_i32.to_le_bytes().to_vec();

    let mut bsp = vec![0; HEADER_SIZE];
    bsp[0..4].copy_from_slice(b"VBSP");
    put_i32(&mut bsp, 4, 20);
    for (index, lump) in lumps.into_iter().enumerate() {
        if lump.is_empty() {
            continue;
        }
        let offset = bsp.len();
        bsp.extend_from_slice(&lump);
        let header = 8 + index * 16;
        put_i32(&mut bsp, header, offset as i32);
        put_i32(&mut bsp, header + 4, lump.len() as i32);
        if matches!(index, 7 | 8 | 53 | 58) {
            put_i32(&mut bsp, header + 8, 1);
        }
    }
    bsp
}

fn glb_json(glb: &[u8]) -> Value {
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + json_length]).unwrap()
}

fn read_f32_accessor(glb: &[u8], gltf: &Value, accessor_index: usize) -> Vec<f32> {
    let accessor = &gltf["accessors"][accessor_index];
    let view = &gltf["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let binary = &glb[20 + json_length + 8..];
    let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize;
    let count = accessor["count"].as_u64().unwrap() as usize * 2;
    (0..count)
        .map(|index| {
            let start = offset + index * 4;
            f32::from_le_bytes(binary[start..start + 4].try_into().unwrap())
        })
        .collect()
}

#[test]
fn owns_exact_face_uvs_and_all_bump_channels() {
    let ldr = lighting(10, true, 1);
    let bsp = synthetic_bsp(Some(ldr), None, true, [0, 255, 255, 255], 0, 0);
    let result = export_bsp_with_options(
        &bsp,
        &ExportOptions {
            lightmap_set: LightmapSet::Ldr,
            atlas_width: 64,
            ..ExportOptions::default()
        },
    )
    .unwrap();
    let artifacts = result.lightmaps.as_ref().unwrap();
    let manifest = serde_json::to_value(&artifacts.manifest).unwrap();

    assert_eq!(result.stats.lightmapped_faces, 1);
    assert_eq!(result.stats.bumped_lightmapped_faces, 1);
    assert_eq!((artifacts.flat.width, artifacts.flat.height), (2, 2));
    assert_eq!(&artifacts.flat.pixels[0..4], &sample(10, 2, 3, -1));
    assert_eq!(
        &artifacts.directional[0].pixels[0..4],
        &sample(20, 2, 3, -1)
    );
    assert_eq!(
        &artifacts.directional[1].pixels[0..4],
        &sample(30, 2, 3, -1)
    );
    assert_eq!(
        &artifacts.directional[2].pixels[0..4],
        &sample(40, 2, 3, -1)
    );
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["source"]["lightingSet"], "ldr");
    assert_eq!(manifest["atlas"]["encoding"], "color-rgb-exp-32");
    assert_eq!(manifest["atlas"]["colorSpace"], "linear");
    assert_eq!(manifest["faces"][0]["faceIndex"], 0);
    assert_eq!(manifest["faces"][0]["styles"], serde_json::json!([0]));
    assert_eq!(manifest["faces"][0]["bumpLight"], true);

    let gltf = glb_json(&result.glb);
    let attributes = &gltf["meshes"][0]["primitives"][0]["attributes"];
    let uv1 = read_f32_accessor(
        &result.glb,
        &gltf,
        attributes["TEXCOORD_1"].as_u64().unwrap() as usize,
    );
    assert_eq!(&uv1[0..2], &[0.25, 0.25]);
    assert_eq!(&uv1[2..4], &[0.75, 0.25]);
    assert!(uv1.iter().all(|value| (0.0..=1.0).contains(value)));

    let png = encode_lightmap_png(&artifacts.flat).unwrap();
    assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn auto_prefers_complete_hdr_pair_and_explicit_ldr_is_respected() {
    let bsp = synthetic_bsp(
        Some(lighting(10, false, 1)),
        Some(lighting(80, false, 1)),
        false,
        [0, 255, 255, 255],
        0,
        0,
    );

    let automatic = export_bsp_with_options(&bsp, &ExportOptions::default()).unwrap();
    let automatic_artifacts = automatic.lightmaps.unwrap();
    assert_eq!(
        &automatic_artifacts.flat.pixels[0..4],
        &sample(80, 2, 3, -1)
    );
    assert_eq!(
        serde_json::to_value(automatic_artifacts.manifest).unwrap()["source"]["lightingSet"],
        "hdr"
    );

    let ldr = export_bsp_with_options(
        &bsp,
        &ExportOptions {
            lightmap_set: LightmapSet::Ldr,
            ..ExportOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        &ldr.lightmaps.unwrap().flat.pixels[0..4],
        &sample(10, 2, 3, -1)
    );
}

#[test]
fn rejects_incomplete_requested_pair_and_unsupported_multi_style_faces() {
    let no_hdr_faces = synthetic_bsp(
        Some(lighting(10, false, 1)),
        None,
        false,
        [0, 255, 255, 255],
        0,
        0,
    );
    let hdr_error = export_bsp_with_options(
        &no_hdr_faces,
        &ExportOptions {
            lightmap_set: LightmapSet::Hdr,
            ..ExportOptions::default()
        },
    )
    .unwrap_err();
    assert!(hdr_error.contains("complete HDR face/lighting pair"));

    let mut wrong_version = no_hdr_faces.clone();
    put_i32(&mut wrong_version, 8 + 8 * 16 + 8, 0);
    let version_error = export_bsp_with_options(
        &wrong_version,
        &ExportOptions {
            lightmap_set: LightmapSet::Ldr,
            ..ExportOptions::default()
        },
    )
    .unwrap_err();
    assert!(version_error.contains("unsupported LDR lightmap pair versions"));

    let multi_style = synthetic_bsp(
        Some(lighting(10, false, 2)),
        None,
        false,
        [0, 32, 255, 255],
        0,
        0,
    );
    let style_error = export_bsp_with_options(
        &multi_style,
        &ExportOptions {
            lightmap_set: LightmapSet::Ldr,
            ..ExportOptions::default()
        },
    )
    .unwrap_err();
    assert!(style_error.contains("face 0"));
    assert!(style_error.contains("multiple light styles"));
}

#[test]
fn validates_sample_ranges_and_does_not_invent_lightmaps() {
    let truncated = synthetic_bsp(
        Some(lighting(10, true, 1)[..63].to_vec()),
        None,
        true,
        [0, 255, 255, 255],
        0,
        0,
    );
    let range_error = export_bsp_with_options(
        &truncated,
        &ExportOptions {
            lightmap_set: LightmapSet::Ldr,
            ..ExportOptions::default()
        },
    )
    .unwrap_err();
    assert!(range_error.contains("face 0"));
    assert!(range_error.contains("lighting lump"));

    let no_light = synthetic_bsp(
        Some(lighting(10, false, 1)),
        None,
        false,
        [0, 255, 255, 255],
        -1,
        0,
    );
    let result = export_bsp_with_options(
        &no_light,
        &ExportOptions {
            lightmap_set: LightmapSet::Ldr,
            ..ExportOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.stats.lightmapped_faces, 0);
    assert!(result.lightmaps.is_none());

    let no_light_flag = synthetic_bsp(
        Some(lighting(10, false, 1)),
        None,
        false,
        [0, 255, 255, 255],
        0,
        0x0400,
    );
    let result = export_bsp_with_options(
        &no_light_flag,
        &ExportOptions {
            lightmap_set: LightmapSet::Ldr,
            ..ExportOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.stats.lightmapped_faces, 0);
    assert!(result.lightmaps.is_none());
}

#[test]
fn cli_writes_flat_directional_atlases_and_versioned_manifest() {
    let directory =
        std::env::temp_dir().join(format!("bsp-to-glb-lightmap-output-{}", std::process::id()));
    if directory.exists() {
        fs::remove_dir_all(&directory).unwrap();
    }
    fs::create_dir_all(&directory).unwrap();
    let bsp_path = directory.join("synthetic.bsp");
    let glb_path = directory.join("synthetic.glb");
    let atlas_path = directory.join("lighting.png");
    let manifest_path = directory.join("lighting.json");
    fs::write(
        &bsp_path,
        synthetic_bsp(
            Some(lighting(10, true, 1)),
            None,
            true,
            [0, 255, 255, 255],
            0,
            0,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .args([
            "--bsp",
            bsp_path.to_str().unwrap(),
            "--out",
            glb_path.to_str().unwrap(),
            "--lightmap-set",
            "ldr",
            "--lightmap-atlas",
            atlas_path.to_str().unwrap(),
            "--lightmap-manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(glb_path.exists());
    assert!(atlas_path.exists());
    assert!(directory.join("lighting.bump-0.png").exists());
    assert!(directory.join("lighting.bump-1.png").exists());
    assert!(directory.join("lighting.bump-2.png").exists());
    let manifest: Value = serde_json::from_slice(&fs::read(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["atlas"]["channels"][0]["uri"], "lighting.png");
    assert_eq!(
        manifest["atlas"]["channels"][3]["uri"],
        "lighting.bump-2.png"
    );

    fs::remove_dir_all(directory).unwrap();
}
