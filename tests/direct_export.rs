use bsp_to_glb::{
    ExportOptions, decode_compressed_pvs_row, export_bsp, export_bsp_with_options_and_visibility,
    export_bsp_with_visibility, static_prop_collision_inputs,
};
use serde_json::{Value, json};
use std::fs;
use std::io::{Cursor, Write};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

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

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_f32(data: &mut [u8], offset: usize, value: f32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn synthetic_bsp(displacement: bool) -> Vec<u8> {
    let mut lumps = vec![Vec::<u8>::new(); 64];
    lumps[0] = br#"
{
"classname" "worldspawn"
}
{
"classname" "func_brush"
"targetname" "moving_door"
"model" "*1"
"origin" "128 32 16"
"angles" "0 90 0"
"StartDisabled" "1"
"solid" "0"
"rendermode" "0"
}
"#
    .to_vec();

    let mut planes = vec![0; 3 * 20];
    put_f32(&mut planes, 8, 1.0);
    put_i32(&mut planes, 16, 2);
    put_f32(&mut planes, 20, 1.0);
    put_i32(&mut planes, 20 + 16, 0);
    put_f32(&mut planes, 40 + 4, 1.0);
    put_i32(&mut planes, 40 + 16, 1);
    lumps[1] = planes;

    let mut texdata = vec![0; 32];
    put_i32(&mut texdata, 12, 0);
    put_i32(&mut texdata, 16, 64);
    put_i32(&mut texdata, 20, 64);
    lumps[2] = texdata;

    let positions = [
        [0.0, 0.0, 0.0],
        [64.0, 0.0, 0.0],
        [64.0, 64.0, 0.0],
        [0.0, 64.0, 0.0],
        [128.0, 0.0, 0.0],
        [192.0, 0.0, 0.0],
        [192.0, 64.0, 0.0],
        [128.0, 64.0, 0.0],
    ];
    let mut vertices = vec![0; positions.len() * 12];
    for (index, position) in positions.iter().enumerate() {
        for (axis, value) in position.iter().enumerate() {
            put_f32(&mut vertices, index * 12 + axis * 4, *value);
        }
    }
    lumps[3] = vertices;

    let mut visibility = vec![0; 4 + 3 * 8];
    put_i32(&mut visibility, 0, 3);
    for (cluster, row) in [0b101_u8, 0b010, 0b111].into_iter().enumerate() {
        let offset = visibility.len();
        put_i32(&mut visibility, 4 + cluster * 8, offset as i32);
        put_i32(&mut visibility, 8 + cluster * 8, -1);
        visibility.push(row);
    }
    lumps[4] = visibility;

    let mut nodes = vec![0; 2 * 32];
    put_i32(&mut nodes, 0, 1);
    put_i32(&mut nodes, 4, 1);
    put_i32(&mut nodes, 8, -4);
    put_i32(&mut nodes, 32, 2);
    put_i32(&mut nodes, 36, -2);
    put_i32(&mut nodes, 40, -3);
    lumps[5] = nodes;

    let mut texinfo = vec![0; 72];
    put_f32(&mut texinfo, 0, 1.0);
    put_f32(&mut texinfo, 20, 1.0);
    put_f32(&mut texinfo, 32, 1.0 / 16.0);
    put_f32(&mut texinfo, 52, 1.0 / 16.0);
    put_i32(&mut texinfo, 68, 0);
    lumps[6] = texinfo;

    let mut faces = vec![0; 2 * 56];
    for face in 0..2 {
        let offset = face * 56;
        put_u16(&mut faces, offset, 0);
        faces[offset + 2] = u8::from(face == 1);
        put_i32(&mut faces, offset + 4, (face * 4) as i32);
        put_i16(&mut faces, offset + 8, 4);
        put_i16(&mut faces, offset + 10, 0);
        put_i16(
            &mut faces,
            offset + 12,
            if displacement && face == 0 { 0 } else { -1 },
        );
        faces[offset + 16..offset + 20].fill(255);
        if face == 0 {
            faces[offset + 16] = 0;
        }
        put_i32(&mut faces, offset + 20, if face == 0 { 0 } else { -1 });
        put_i32(&mut faces, offset + 36, 4);
        put_i32(&mut faces, offset + 40, 4);
    }
    put_u16(&mut faces, 48, 1);
    put_u16(&mut faces, 50, 0);
    lumps[7] = faces;
    lumps[8] = vec![0; 5 * 5 * 4];

    let mut leaves = vec![0; 4 * 32];
    for (leaf, (cluster, first_face, face_count)) in
        [(-1_i16, 0_u16, 0_u16), (0, 0, 1), (1, 1, 1), (2, 2, 1)]
            .into_iter()
            .enumerate()
    {
        let offset = leaf * 32;
        put_i16(&mut leaves, offset + 4, cluster);
        put_i16(&mut leaves, offset + 8, -16);
        put_i16(&mut leaves, offset + 10, -16);
        put_i16(&mut leaves, offset + 12, -16);
        put_i16(&mut leaves, offset + 14, 16);
        put_i16(&mut leaves, offset + 16, 16);
        put_i16(&mut leaves, offset + 18, 16);
        put_u16(&mut leaves, offset + 20, first_face);
        put_u16(&mut leaves, offset + 22, face_count);
    }
    lumps[10] = leaves;

    let mut edges = vec![0; 8 * 4];
    for face in 0..2 {
        for edge in 0..4 {
            let base = (face * 4 + edge) * 4;
            put_u16(&mut edges, base, (face * 4 + edge) as u16);
            put_u16(&mut edges, base + 2, (face * 4 + (edge + 1) % 4) as u16);
        }
    }
    lumps[12] = edges;

    let mut surfedges = vec![0; 8 * 4];
    for index in 0..8 {
        put_i32(&mut surfedges, index * 4, index as i32);
    }
    lumps[13] = surfedges;

    let mut models = vec![0; 2 * 48];
    for model in 0..2 {
        let offset = model * 48;
        put_i32(&mut models, offset + 40, model as i32);
        put_i32(&mut models, offset + 44, 1);
    }
    put_f32(&mut models, 48 + 24, 128.0);
    put_f32(&mut models, 48 + 28, 32.0);
    put_f32(&mut models, 48 + 32, 16.0);
    lumps[14] = models;

    let mut leaf_faces = vec![0; 3 * 2];
    for (index, face) in [0_u16, 0, 1].into_iter().enumerate() {
        put_u16(&mut leaf_faces, index * 2, face);
    }
    lumps[16] = leaf_faces;

    let compiled_normals = [
        [0.0, 0.0, 1.0],
        [0.0, 0.6, 0.8],
        [0.0, 0.0, 1.0],
        [0.0, -0.6, 0.8],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    let mut normals = vec![0; compiled_normals.len() * 12];
    for (index, normal) in compiled_normals.iter().enumerate() {
        for (axis, value) in normal.iter().enumerate() {
            put_f32(&mut normals, index * 12 + axis * 4, *value);
        }
    }
    lumps[30] = normals;
    let mut normal_indices = vec![0; compiled_normals.len() * 2];
    for index in 0..compiled_normals.len() {
        put_u16(&mut normal_indices, index * 2, index as u16);
    }
    lumps[31] = normal_indices;

    let mut primitive = vec![0; 10];
    primitive[0] = 0;
    put_u16(&mut primitive, 2, 0);
    put_u16(&mut primitive, 4, 6);
    lumps[37] = primitive;
    let mut primitive_indices = vec![0; 12];
    for (index, value) in [0, 1, 3, 1, 2, 3].iter().enumerate() {
        put_u16(&mut primitive_indices, index * 2, *value);
    }
    lumps[39] = primitive_indices;
    lumps[43] = b"brick/test\0".to_vec();
    lumps[44] = 0_i32.to_le_bytes().to_vec();

    if displacement {
        let mut dispinfo = vec![0; 176];
        put_i32(&mut dispinfo, 12, 0);
        put_i32(&mut dispinfo, 16, 0);
        put_i32(&mut dispinfo, 20, 2);
        put_u16(&mut dispinfo, 36, 0);
        lumps[26] = dispinfo;

        let mut dispverts = vec![0; 25 * 20];
        for index in 0..25 {
            put_f32(&mut dispverts, index * 20 + 8, 1.0);
            put_f32(&mut dispverts, index * 20 + 16, index as f32 * 10.0);
        }
        put_f32(&mut dispverts, 6 * 20, 1.0);
        put_f32(&mut dispverts, 6 * 20 + 8, 0.0);
        put_f32(&mut dispverts, 6 * 20 + 12, 4.0);
        put_f32(&mut dispverts, 12 * 20 + 12, 16.0);
        lumps[33] = dispverts;

        let mut disptris = vec![0; 32 * 2];
        for index in 0_usize..32 {
            let tags = 1 | if index.is_multiple_of(2) { 2 } else { 4 };
            put_u16(&mut disptris, index * 2, tags);
        }
        lumps[48] = disptris;

        let mut cubemap = vec![0; 16];
        put_i32(&mut cubemap, 0, 32);
        put_i32(&mut cubemap, 4, 48);
        put_i32(&mut cubemap, 8, 64);
        cubemap[12] = 5;
        lumps[42] = cubemap;

        let mut overlay = vec![0; 352];
        put_i32(&mut overlay, 0, 7);
        put_i16(&mut overlay, 4, 0);
        put_u16(&mut overlay, 6, 1);
        put_i32(&mut overlay, 8, 0);
        lumps[45] = overlay;

        let mut water_overlay = vec![0; 1120];
        put_i32(&mut water_overlay, 0, 9);
        put_i16(&mut water_overlay, 4, 0);
        put_u16(&mut water_overlay, 6, 1);
        put_i32(&mut water_overlay, 8, 0);
        lumps[50] = water_overlay;
    }

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
        if matches!(index, 7 | 8 | 10) {
            put_i32(&mut bsp, header + 8, 1);
        }
    }
    bsp
}

fn append_pak(bsp: &mut Vec<u8>, entries: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (path, data) in entries {
        writer.start_file(path, options).unwrap();
        writer.write_all(data).unwrap();
    }
    let pak = writer.finish().unwrap().into_inner();
    let offset = bsp.len();
    bsp.extend_from_slice(&pak);
    let header = 8 + 40 * 16;
    put_i32(bsp, header, offset as i32);
    put_i32(bsp, header + 4, pak.len() as i32);
}

fn rgba_vtf(pixel: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0; 65];
    data[0..4].copy_from_slice(b"VTF\0");
    put_u32(&mut data, 4, 7);
    put_u32(&mut data, 8, 2);
    put_u32(&mut data, 12, 65);
    put_u16(&mut data, 16, 1);
    put_u16(&mut data, 18, 1);
    put_u16(&mut data, 24, 1);
    data[56] = 1;
    put_u32(&mut data, 57, u32::MAX);
    put_u16(&mut data, 63, 1);
    data.extend_from_slice(&pixel);
    data
}

fn synthetic_bsp_with_tf2_props() -> Vec<u8> {
    let mut bsp = synthetic_bsp(false);
    let mut entities = bsp[lump_offset(&bsp, 0)..].to_vec();
    let entity_length = i32::from_le_bytes(bsp[12..16].try_into().unwrap()) as usize;
    entities.truncate(entity_length);
    entities.extend_from_slice(
        br#"
{
"classname" "prop_dynamic"
"targetname" "animated_crate"
"model" "models/props_test/crate.mdl"
"origin" "10 20 30"
"angles" "0 45 0"
"skin" "2"
"solid" "6"
"StartDisabled" "1"
"DefaultAnim" "idle"
}
"#,
    );
    let entities_offset = bsp.len();
    bsp.extend_from_slice(&entities);
    put_i32(&mut bsp, 8, entities_offset as i32);
    put_i32(&mut bsp, 12, entities.len() as i32);

    let mut static_props = Vec::new();
    static_props.extend_from_slice(&1_u32.to_le_bytes());
    let mut model_name = [0_u8; 128];
    let model_path = b"models/props_test/crate.mdl";
    model_name[..model_path.len()].copy_from_slice(model_path);
    static_props.extend_from_slice(&model_name);
    static_props.extend_from_slice(&3_u32.to_le_bytes());
    static_props.extend_from_slice(&7_u16.to_le_bytes());
    static_props.extend_from_slice(&9_u16.to_le_bytes());
    static_props.extend_from_slice(&12_u16.to_le_bytes());
    static_props.extend_from_slice(&1_u32.to_le_bytes());
    let record_offset = static_props.len();
    static_props.resize(record_offset + 72, 0);
    for (axis, value) in [1.0, 2.0, 3.0].iter().enumerate() {
        put_f32(&mut static_props, record_offset + axis * 4, *value);
    }
    for (axis, value) in [10.0, 20.0, 30.0].iter().enumerate() {
        put_f32(&mut static_props, record_offset + 12 + axis * 4, *value);
    }
    put_u16(&mut static_props, record_offset + 24, 0);
    put_u16(&mut static_props, record_offset + 26, 0);
    put_u16(&mut static_props, record_offset + 28, 2);
    static_props[record_offset + 30] = 6;
    put_i32(&mut static_props, record_offset + 32, 4);
    put_f32(&mut static_props, record_offset + 36, 128.0);
    put_f32(&mut static_props, record_offset + 40, 512.0);
    for (axis, value) in [4.0, 5.0, 6.0].iter().enumerate() {
        put_f32(&mut static_props, record_offset + 44 + axis * 4, *value);
    }
    put_f32(&mut static_props, record_offset + 56, 1.25);
    put_u16(&mut static_props, record_offset + 60, 80);
    put_u16(&mut static_props, record_offset + 62, 95);
    put_u32(&mut static_props, record_offset + 64, 0x101);
    put_u16(&mut static_props, record_offset + 68, 16);
    put_u16(&mut static_props, record_offset + 70, 32);

    let game_lump_offset = bsp.len();
    let static_props_offset = game_lump_offset + 20;
    let mut game_lump = vec![0; 20];
    put_i32(&mut game_lump, 0, 1);
    // Source writes the multi-character constant 'sprp' as little-endian bytes.
    game_lump[4..8].copy_from_slice(b"prps");
    put_u16(&mut game_lump, 8, 0);
    put_u16(&mut game_lump, 10, 10);
    put_i32(&mut game_lump, 12, static_props_offset as i32);
    put_i32(&mut game_lump, 16, static_props.len() as i32);
    game_lump.extend_from_slice(&static_props);
    bsp.extend_from_slice(&game_lump);
    let game_lump_header = 8 + 35 * 16;
    put_i32(&mut bsp, game_lump_header, game_lump_offset as i32);
    put_i32(&mut bsp, game_lump_header + 4, game_lump.len() as i32);
    bsp
}

fn replace_static_prop_game_lump(
    bsp: &mut Vec<u8>,
    payload: &[u8],
    version: u16,
    compressed: bool,
) {
    let child_data = if compressed {
        let mut alone = Vec::new();
        lzma_rs::lzma_compress(&mut Cursor::new(payload), &mut alone).unwrap();
        let compressed_size = alone.len() - 13;
        let mut source_lzma = Vec::with_capacity(17 + compressed_size);
        source_lzma.extend_from_slice(b"LZMA");
        source_lzma.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        source_lzma.extend_from_slice(&(compressed_size as u32).to_le_bytes());
        source_lzma.extend_from_slice(&alone[..5]);
        source_lzma.extend_from_slice(&alone[13..]);
        source_lzma
    } else {
        payload.to_vec()
    };
    let game_lump_offset = bsp.len();
    let child_offset = game_lump_offset + 20;
    let mut game_lump = vec![0; 20];
    put_i32(&mut game_lump, 0, 1);
    game_lump[4..8].copy_from_slice(b"prps");
    put_u16(&mut game_lump, 8, u16::from(compressed));
    put_u16(&mut game_lump, 10, version);
    put_i32(&mut game_lump, 12, child_offset as i32);
    put_i32(&mut game_lump, 16, payload.len() as i32);
    game_lump.extend_from_slice(&child_data);
    bsp.extend_from_slice(&game_lump);
    let game_lump_header = 8 + 35 * 16;
    put_i32(bsp, game_lump_header, game_lump_offset as i32);
    put_i32(bsp, game_lump_header + 4, game_lump.len() as i32);
}

fn static_prop_payload(bsp: &[u8]) -> Vec<u8> {
    let game_lump = lump_offset(bsp, 35);
    let child_offset =
        i32::from_le_bytes(bsp[game_lump + 12..game_lump + 16].try_into().unwrap()) as usize;
    let child_length =
        i32::from_le_bytes(bsp[game_lump + 16..game_lump + 20].try_into().unwrap()) as usize;
    bsp[child_offset..child_offset + child_length].to_vec()
}

fn sdk_v11_static_prop_payload() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&1_u32.to_le_bytes());
    let mut model_name = [0_u8; 128];
    let model_path = b"models/props_test/scaled.mdl";
    model_name[..model_path.len()].copy_from_slice(model_path);
    data.extend_from_slice(&model_name);
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.extend_from_slice(&42_u16.to_le_bytes());
    data.extend_from_slice(&1_u32.to_le_bytes());
    let record = data.len();
    data.resize(record + 76, 0);
    put_f32(&mut data, record, 8.0);
    put_f32(&mut data, record + 4, 16.0);
    put_f32(&mut data, record + 8, 24.0);
    put_u16(&mut data, record + 24, 0);
    put_u16(&mut data, record + 26, 0);
    put_u16(&mut data, record + 28, 1);
    data[record + 30] = 2;
    data[record + 31] = 0x10;
    put_i32(&mut data, record + 32, 3);
    put_f32(&mut data, record + 56, 0.75);
    data[record + 60..record + 64].copy_from_slice(&[1, 2, 3, 4]);
    data[record + 64..record + 68].copy_from_slice(&[128, 192, 255, 255]);
    put_u32(&mut data, record + 68, 0x200);
    put_f32(&mut data, record + 72, 2.5);
    data
}

#[test]
fn decodes_compressed_pvs_rows_exactly() {
    let words = decode_compressed_pvs_row(&[0b0000_0101, 0, 2, 0b1000_0000], 0, 32).unwrap();
    assert_eq!(words, [0x8000_0005]);

    assert!(decode_compressed_pvs_row(&[0, 0], 0, 8).is_err());
    assert!(decode_compressed_pvs_row(&[0, 2], 0, 8).is_err());
    assert!(decode_compressed_pvs_row(&[0], 0, 8).is_err());
}

#[test]
fn exports_exact_versioned_visibility_memberships() {
    let result = export_bsp_with_visibility(&synthetic_bsp(false), None).unwrap();
    let sidecar = result.visibility.as_ref().unwrap();

    assert_eq!(sidecar.format, "bsp-to-glb.visibility");
    assert_eq!(sidecar.version, 2);
    assert_eq!(sidecar.world_head_node, 0);
    assert_eq!(sidecar.planes.len(), 3);
    assert_eq!(sidecar.planes[1].normal, [1.0, 0.0, 0.0]);
    assert_eq!(sidecar.planes[1].distance, 0.0);
    assert_eq!(sidecar.nodes.len(), 2);
    assert_eq!(sidecar.nodes[0].plane_index, 1);
    assert_eq!(sidecar.nodes[0].children, [1, -4]);
    assert_eq!(sidecar.nodes[1].plane_index, 2);
    assert_eq!(sidecar.nodes[1].children, [-2, -3]);
    assert_eq!(sidecar.locate_world_leaf([1.0, -1.0, 0.0]).unwrap(), 2);
    assert_eq!(sidecar.locate_world_leaf([1.0, 0.0, 0.0]).unwrap(), 1);
    assert_eq!(sidecar.locate_world_leaf([-1.0, 1.0, 0.0]).unwrap(), 3);
    assert!(sidecar.locate_world_leaf([f32::NAN, 0.0, 0.0]).is_err());
    assert_eq!(sidecar.cluster_count, 3);
    assert_eq!(sidecar.leaves.len(), 4);
    assert_eq!(sidecar.pvs_words, [0b101, 0b010, 0b111]);
    assert_eq!(sidecar.world_face_indices, [0]);
    assert_eq!(sidecar.world_face_leaf_offsets, [0, 2]);
    assert_eq!(sidecar.world_face_leaf_indices, [1, 2]);
    assert_eq!(sidecar.world_face_cluster_words, [0b011]);
    assert_eq!(sidecar.face_model_indices, [0, 1]);
    assert_eq!(sidecar.dynamic_model_indices, [1]);
    assert_eq!(sidecar.relevant_cluster_count, 2);
    assert_eq!(sidecar.covered_cluster_count, 2);

    assert_eq!(sidecar.chunks.len(), 2);
    assert!(sidecar.chunks[0].static_pvs);
    assert!(!sidecar.chunks[1].static_pvs);
    assert_eq!(sidecar.chunk_leaf_offsets, [0, 2, 2]);
    assert_eq!(sidecar.chunk_leaf_indices, [1, 2]);
    assert_eq!(sidecar.chunk_cluster_words, [0b011, 0]);
    assert_eq!(sidecar.chunk_face_offsets, [0, 1, 2]);
    assert_eq!(sidecar.chunk_face_indices, [0, 1]);

    let encoded = sidecar.to_json().unwrap();
    assert_eq!(encoded, sidecar.to_json().unwrap());
    let decoded: bsp_to_glb::VisibilitySidecar = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(sidecar, &decoded);
    assert_eq!(encoded, decoded.to_json().unwrap());

    let gltf = glb_json(&result.glb);
    assert_eq!(
        gltf["meshes"][0]["primitives"][0]["extras"]["visibilityChunkIndex"],
        0
    );
    assert_eq!(
        gltf["meshes"][1]["primitives"][0]["extras"]["visibilityChunkIndex"],
        1
    );
}

#[test]
fn rejects_malformed_visibility_tree_references_and_cycles() {
    let cases = [
        (5, 0, 3, "node 0 references missing plane 3"),
        (5, 4, 2, "node 0 references missing node 2"),
        (5, 4, -5, "node 0 references missing leaf 4"),
        (14, 36, 2, "model 0 references missing head node 2"),
        (5, 36, 0, "visibility node graph contains a cycle"),
    ];

    for (lump, relative_offset, value, expected) in cases {
        let mut bsp = synthetic_bsp(false);
        let offset = lump_offset(&bsp, lump) + relative_offset;
        put_i32(&mut bsp, offset, value);
        let error = export_bsp_with_visibility(&bsp, None).unwrap_err();
        assert!(
            error.contains(expected),
            "expected {expected:?}, got {error:?}"
        );
    }
}

#[test]
fn rejects_non_finite_visibility_planes() {
    let mut bsp = synthetic_bsp(false);
    let distance = lump_offset(&bsp, 1) + 12;
    put_f32(&mut bsp, distance, f32::NAN);

    let error = export_bsp_with_visibility(&bsp, None).unwrap_err();
    assert!(
        error.contains("plane 0 contains a non-finite value"),
        "{error}"
    );
}

#[test]
fn rejects_inverted_visibility_leaf_bounds() {
    let mut bsp = synthetic_bsp(false);
    let leaf_minimum = lump_offset(&bsp, 10) + 8;
    put_i16(&mut bsp, leaf_minimum, 17);

    let error = export_bsp_with_visibility(&bsp, None).unwrap_err();
    assert!(error.contains("leaf 0 has inverted bounds"), "{error}");
}

#[test]
fn rejects_visibility_lumps_above_public_record_bounds() {
    let bsp = synthetic_bsp(false);
    let too_many_planes = replace_lump(&bsp, 1, &vec![0; 65_537 * 20]);
    let error = export_bsp_with_visibility(&too_many_planes, None).unwrap_err();
    assert!(error.contains("plane count 65537 exceeds 65536"), "{error}");

    let too_many_nodes = replace_lump(&bsp, 5, &vec![0; 65_537 * 32]);
    let error = export_bsp_with_visibility(&too_many_nodes, None).unwrap_err();
    assert!(error.contains("node count 65537 exceeds 65536"), "{error}");
}

#[test]
fn rejects_visibility_trees_above_the_depth_bound() {
    const EXCESSIVE_DEPTH: usize = 4_097;
    let bsp = synthetic_bsp(false);
    let mut nodes = vec![0; EXCESSIVE_DEPTH * 32];
    for index in 0..EXCESSIVE_DEPTH {
        let offset = index * 32;
        put_i32(&mut nodes, offset, 0);
        put_i32(
            &mut nodes,
            offset + 4,
            if index + 1 == EXCESSIVE_DEPTH {
                -1
            } else {
                (index + 1) as i32
            },
        );
        put_i32(&mut nodes, offset + 8, -1);
    }
    let bsp = replace_lump(&bsp, 5, &nodes);

    let error = export_bsp_with_visibility(&bsp, None).unwrap_err();
    assert!(
        error.contains("visibility tree depth 4097 exceeds 4096"),
        "{error}"
    );
}

#[test]
fn composes_displacements_direct_lightmaps_materials_props_and_visibility() {
    let result =
        export_bsp_with_options_and_visibility(&synthetic_bsp(true), &ExportOptions::default())
            .unwrap();
    let gltf = glb_json(&result.glb);
    let displacement = gltf["meshes"][0]["primitives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|primitive| primitive["extras"]["geometry"] == "displacement")
        .unwrap();

    assert!(result.lightmaps.is_some());
    assert!(result.visibility.is_some());
    assert_eq!(result.material_manifest.materials.len(), 1);
    assert_eq!(result.props["schema"], "bsp-to-glb.props");
    assert_eq!(result.stats.displacement_faces, 1);
    assert!(displacement["attributes"].get("TEXCOORD_1").is_some());
    assert!(displacement["extras"]["visibilityChunkIndex"].is_u64());
}

#[test]
fn cli_writes_visibility_sidecar() {
    let directory = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("visibility-cli-test-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let bsp_path = directory.join("synthetic.bsp");
    let glb_path = directory.join("synthetic.glb");
    let visibility_path = directory.join("synthetic.visibility.json");
    let material_manifest_path = directory.join("synthetic.materials.json");
    let mount_plan_path = directory.join("material-mounts.json");
    let content_path = directory.join("content");
    fs::create_dir_all(content_path.join("materials/brick")).unwrap();
    fs::write(&bsp_path, synthetic_bsp(false)).unwrap();
    fs::write(
        content_path.join("materials/brick/test.vmt"),
        br#"LightmappedGeneric {}"#,
    )
    .unwrap();
    fs::write(
        &mount_plan_path,
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "mounts": [
                { "id": "fixture", "kind": "directory", "path": content_path }
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .args([
            "--bsp",
            bsp_path.to_str().unwrap(),
            "--out",
            glb_path.to_str().unwrap(),
            "--visibility-out",
            visibility_path.to_str().unwrap(),
            "--material-mount-plan",
            mount_plan_path.to_str().unwrap(),
            "--material-manifest",
            material_manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sidecar: bsp_to_glb::VisibilitySidecar =
        serde_json::from_slice(&fs::read(&visibility_path).unwrap()).unwrap();
    assert_eq!(sidecar.cluster_count, 3);
    let material_bytes = fs::read(&material_manifest_path).unwrap();
    assert!(!String::from_utf8_lossy(&material_bytes).contains(directory.to_str().unwrap()));
    let materials: Value = serde_json::from_slice(&material_bytes).unwrap();
    let provenance = &materials["materials"][0]["vmt"]["provenance"];
    assert_eq!(provenance["kind"], "external");
    assert_eq!(provenance["mountId"], "fixture");
    assert_eq!(provenance["path"], "materials/brick/test.vmt");
    assert_eq!(provenance.as_object().unwrap().len(), 5);
    assert!(materials["unresolvedAssets"].as_array().unwrap().is_empty());
    assert!(glb_path.is_file());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_rejects_a_material_mount_plan_without_render_output() {
    let directory = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!(
            "resolver-collision-only-test-{}",
            std::process::id()
        ));
    fs::create_dir_all(&directory).unwrap();
    let bsp_path = directory.join("synthetic.bsp");
    let collision_path = directory.join("synthetic.collision.json");
    let mount_plan_path = directory.join("material-mounts.json");
    fs::write(&bsp_path, synthetic_bsp(false)).unwrap();
    fs::write(
        &mount_plan_path,
        serde_json::to_vec(&json!({ "schemaVersion": 1, "mounts": [] })).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .args([
            "--bsp",
            bsp_path.to_str().unwrap(),
            "--collision-out",
            collision_path.to_str().unwrap(),
            "--material-mount-plan",
            mount_plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--material-mount-plan requires --out")
    );
    assert!(!collision_path.exists());

    fs::remove_dir_all(directory).unwrap();
}

fn glb_json(glb: &[u8]) -> Value {
    assert_eq!(&glb[0..4], b"glTF");
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + json_length]).unwrap()
}

fn read_f32_accessor(glb: &[u8], gltf: &Value, accessor_index: usize) -> Vec<f32> {
    let accessor = &gltf["accessors"][accessor_index];
    let view = &gltf["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let binary_header = 20 + json_length;
    let binary = &glb[binary_header + 8..];
    let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize
        + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    let width = match accessor["type"].as_str().unwrap() {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        value => panic!("unsupported test accessor type {value}"),
    };
    let count = accessor["count"].as_u64().unwrap() as usize * width;
    (0..count)
        .map(|index| {
            let start = offset + index * 4;
            f32::from_le_bytes(binary[start..start + 4].try_into().unwrap())
        })
        .collect()
}

fn read_u32_accessor(glb: &[u8], gltf: &Value, accessor_index: usize) -> Vec<u32> {
    let accessor = &gltf["accessors"][accessor_index];
    let view = &gltf["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let binary_header = 20 + json_length;
    let binary = &glb[binary_header + 8..];
    let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize
        + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    (0..accessor["count"].as_u64().unwrap() as usize)
        .map(|index| {
            let start = offset + index * 4;
            u32::from_le_bytes(binary[start..start + 4].try_into().unwrap())
        })
        .collect()
}

fn lump_offset(bsp: &[u8], lump: usize) -> usize {
    let header = 8 + lump * 16;
    i32::from_le_bytes(bsp[header..header + 4].try_into().unwrap()) as usize
}

fn set_lump_version(bsp: &mut [u8], lump: usize, version: i32) {
    put_i32(bsp, 8 + lump * 16 + 8, version);
}

fn replace_lump(bsp: &[u8], target_lump: usize, replacement: &[u8]) -> Vec<u8> {
    let mut rebuilt = bsp[..HEADER_SIZE].to_vec();
    for lump in 0..64 {
        let header = 8 + lump * 16;
        let old_offset = i32::from_le_bytes(bsp[header..header + 4].try_into().unwrap()) as usize;
        let old_length =
            i32::from_le_bytes(bsp[header + 4..header + 8].try_into().unwrap()) as usize;
        let data = if lump == target_lump {
            replacement
        } else {
            &bsp[old_offset..old_offset + old_length]
        };
        if data.is_empty() {
            put_i32(&mut rebuilt, header, 0);
            put_i32(&mut rebuilt, header + 4, 0);
            continue;
        }
        let offset = rebuilt.len();
        rebuilt.extend_from_slice(data);
        put_i32(&mut rebuilt, header, offset as i32);
        put_i32(&mut rebuilt, header + 4, data.len() as i32);
    }
    rebuilt
}

fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn exports_tf2_static_and_dynamic_props_as_unresolved_model_references() {
    let result = export_bsp(&synthetic_bsp_with_tf2_props(), None).unwrap();
    let gltf = glb_json(&result.glb);
    let props = &gltf["asset"]["extras"]["props"];

    assert_eq!(props["schema"], "bsp-to-glb.props");
    assert_eq!(props["schemaVersion"], 1);
    assert_eq!(props["staticPropLump"]["version"], 10);
    assert_eq!(props["staticPropLump"]["layout"], "tf2-v10");
    assert_eq!(props["staticPropLump"]["dictionaryCount"], 1);
    assert_eq!(props["staticPropLump"]["instanceCount"], 1);
    assert_eq!(props["staticPropLump"]["solidInstanceCount"], 1);
    assert_eq!(props["modelAssets"].as_array().unwrap().len(), 1);
    assert_eq!(
        props["modelAssets"][0]["sourcePath"],
        "models/props_test/crate.mdl"
    );
    assert_eq!(props["modelAssets"][0]["resolutionStatus"], "unsupported");

    let nodes = gltf["nodes"].as_array().unwrap();
    let static_prop = nodes
        .iter()
        .find(|node| node["extras"]["sourceType"] == "staticProp")
        .unwrap();
    assert!(static_prop.get("mesh").is_none());
    assert_eq!(static_prop["extras"]["staticPropIndex"], 0);
    assert_eq!(static_prop["extras"]["dictionaryIndex"], 0);
    assert_eq!(static_prop["extras"]["modelAssetIndex"], 0);
    assert_eq!(
        static_prop["extras"]["sourceOrigin"],
        json!([1.0, 2.0, 3.0])
    );
    assert_eq!(
        static_prop["extras"]["sourceAngles"],
        json!([10.0, 20.0, 30.0])
    );
    assert_eq!(static_prop["extras"]["firstLeaf"], 0);
    assert_eq!(static_prop["extras"]["leafCount"], 2);
    assert_eq!(static_prop["extras"]["leaves"], json!([7, 9]));
    assert_eq!(static_prop["extras"]["skin"], 4);
    assert_eq!(static_prop["extras"]["solidity"], 6);
    assert_eq!(static_prop["extras"]["solid"], true);
    assert_eq!(static_prop["extras"]["flags"], 0x101);
    assert_eq!(static_prop["extras"]["fadeMinDistance"], 128.0);
    assert_eq!(static_prop["extras"]["fadeMaxDistance"], 512.0);
    assert_eq!(
        static_prop["extras"]["lightingOrigin"],
        json!([4.0, 5.0, 6.0])
    );
    assert_eq!(static_prop["extras"]["forcedFadeScale"], 1.25);
    assert_eq!(static_prop["extras"]["minDxLevel"], 80);
    assert_eq!(static_prop["extras"]["maxDxLevel"], 95);
    assert_eq!(static_prop["extras"]["lightmapResolution"], json!([16, 32]));

    let dynamic_prop = nodes
        .iter()
        .find(|node| node["extras"]["sourceType"] == "dynamicPropEntity")
        .unwrap();
    assert!(dynamic_prop.get("mesh").is_none());
    assert_eq!(dynamic_prop["extras"]["entityIndex"], 2);
    assert_eq!(dynamic_prop["extras"]["targetname"], "animated_crate");
    assert_eq!(dynamic_prop["extras"]["modelAssetIndex"], 0);
    assert_eq!(dynamic_prop["extras"]["initialState"]["startDisabled"], "1");
    assert_eq!(
        dynamic_prop["extras"]["initialState"]["defaultAnim"],
        "idle"
    );
    assert_eq!(
        dynamic_prop["extras"]["keyValues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|property| property["key"] == "model")
            .count(),
        1
    );
}

#[test]
fn shares_static_prop_identity_with_collision_export() {
    let props = static_prop_collision_inputs(&synthetic_bsp_with_tf2_props())
        .unwrap()
        .unwrap();

    assert_eq!(props.len(), 1);
    assert_eq!(props[0].prop_index, 0);
    assert_eq!(props[0].model_name, "models/props_test/crate.mdl");
    assert_eq!(props[0].solid_mode, 6);
    assert!(
        static_prop_collision_inputs(&synthetic_bsp(false))
            .unwrap()
            .is_none()
    );
}

#[test]
fn parses_compressed_tf2_static_prop_child_lump() {
    let original = synthetic_bsp_with_tf2_props();
    let payload = static_prop_payload(&original);
    let mut bsp = synthetic_bsp(false);
    replace_static_prop_game_lump(&mut bsp, &payload, 10, true);

    let result = export_bsp(&bsp, None).unwrap();
    let gltf = glb_json(&result.glb);
    assert_eq!(result.stats.static_props, 1);
    assert_eq!(result.stats.solid_static_props, 1);
    assert_eq!(
        gltf["asset"]["extras"]["props"]["staticPropLump"]["layout"],
        "tf2-v10"
    );
}

#[test]
fn exports_uniform_scale_from_supported_v11_layout() {
    let mut bsp = synthetic_bsp(false);
    replace_static_prop_game_lump(&mut bsp, &sdk_v11_static_prop_payload(), 11, false);

    let result = export_bsp(&bsp, None).unwrap();
    let gltf = glb_json(&result.glb);
    let node = gltf["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["extras"]["sourceType"] == "staticProp")
        .unwrap();
    assert_eq!(
        gltf["asset"]["extras"]["props"]["staticPropLump"]["layout"],
        "sdk2013-v11"
    );
    assert_eq!(node["extras"]["uniformScale"], 2.5);
    assert_eq!(node["extras"]["flagsEx"], 0x200);
    assert_eq!(node["matrix"][0], 2.5);
}

#[test]
fn rejects_static_prop_with_missing_dictionary_identity() {
    let mut bsp = synthetic_bsp_with_tf2_props();
    let game_lump = lump_offset(&bsp, 35);
    let child =
        i32::from_le_bytes(bsp[game_lump + 12..game_lump + 16].try_into().unwrap()) as usize;
    let record = child + 4 + 128 + 4 + 3 * 2 + 4;
    put_u16(&mut bsp, record + 24, 1);

    let error = export_bsp(&bsp, None).unwrap_err();
    assert!(error.contains("static prop 0"), "unexpected error: {error}");
    assert!(
        error.contains("dictionary entry 1"),
        "unexpected error: {error}"
    );
}

#[test]
fn exports_compiled_faces_with_model_boundaries_and_uvs() {
    let bsp = synthetic_bsp(false);
    let lightmaps = json!({
        "atlasWidth": 128,
        "atlasHeight": 64,
        "faces": [{
            "faceIndex": 0,
            "w": 5,
            "h": 5,
            "atlasX": 8,
            "atlasY": 4,
            "lmVecs": [[0.0625, 0.0, 0.0, 0.0], [0.0, 0.0625, 0.0, 0.0]],
            "lmMinsS": 0,
            "lmMinsT": 0,
            "verts": [[0, 0, 0], [64, 0, 0], [64, 64, 0], [0, 64, 0]]
        }]
    });

    let result = export_bsp(&bsp, Some(lightmaps.to_string().as_bytes())).unwrap();
    let gltf = glb_json(&result.glb);

    assert_eq!(result.stats.models, 2);
    assert_eq!(result.stats.faces, 2);
    assert_eq!(result.stats.triangles, 4);
    assert_eq!(gltf["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(gltf["meshes"].as_array().unwrap().len(), 2);
    assert_eq!(gltf["materials"][0]["name"], "brick/test");
    assert_eq!(gltf["nodes"][1]["extras"]["classname"], "func_brush");
    assert_eq!(gltf["nodes"][1]["extras"]["targetname"], "moving_door");
    assert_eq!(gltf["nodes"][1]["extras"]["startDisabled"], "1");
    assert_eq!(gltf["nodes"][1]["extras"]["solid"], "0");
    assert_eq!(gltf["nodes"][1]["extras"]["rendermode"], "0");
    assert_eq!(gltf["nodes"][1]["extras"]["model"], "*1");
    assert_eq!(gltf["nodes"][1]["extras"]["initiallyRendered"], false);
    let matrix = gltf["nodes"][1]["matrix"].as_array().unwrap();
    assert_eq!(matrix[12], 128.0);
    assert_eq!(matrix[13], 16.0);
    assert_eq!(matrix[14], -32.0);
    assert!(matrix[2].as_f64().unwrap() < -0.99999);
    assert!(matrix[8].as_f64().unwrap() > 0.99999);

    let world_attributes = &gltf["meshes"][0]["primitives"][0]["attributes"];
    assert!(world_attributes.get("POSITION").is_some());
    assert!(world_attributes.get("NORMAL").is_some());
    assert!(world_attributes.get("TEXCOORD_0").is_some());
    assert!(world_attributes.get("TEXCOORD_1").is_some());
    let uv1 = read_f32_accessor(
        &result.glb,
        &gltf,
        world_attributes["TEXCOORD_1"].as_u64().unwrap() as usize,
    );
    assert_eq!(&uv1[0..2], &[8.5 / 128.0, 4.5 / 64.0]);
    let normals = read_f32_accessor(
        &result.glb,
        &gltf,
        world_attributes["NORMAL"].as_u64().unwrap() as usize,
    );
    assert_eq!(&normals[3..6], &[0.0, 0.8, -0.6]);
    let world_primitive = &gltf["meshes"][0]["primitives"][0];
    let indices = read_u32_accessor(
        &result.glb,
        &gltf,
        world_primitive["indices"].as_u64().unwrap() as usize,
    );
    assert_eq!(indices, [0, 1, 3, 1, 2, 3]);
    assert_eq!(world_primitive["extras"]["triangulation"], "compiled");
    assert_eq!(world_primitive["extras"]["initiallyRendered"], true);
    let entity_primitive = &gltf["meshes"][1]["primitives"][0];
    let entity_attributes = &entity_primitive["attributes"];
    let entity_normals = read_f32_accessor(
        &result.glb,
        &gltf,
        entity_attributes["NORMAL"].as_u64().unwrap() as usize,
    );
    assert_eq!(&entity_normals[0..3], &[0.0, -1.0, 0.0]);
    let entity_indices = read_u32_accessor(
        &result.glb,
        &gltf,
        entity_primitive["indices"].as_u64().unwrap() as usize,
    );
    assert_eq!(entity_indices, [0, 2, 1, 0, 3, 2]);
    assert!(
        gltf["meshes"][1]["primitives"][0]["attributes"]
            .get("TEXCOORD_1")
            .is_none()
    );
}

#[test]
fn rejects_lightmap_metadata_when_compiled_face_has_no_lightmap() {
    let mut bsp = synthetic_bsp(false);
    let faces = lump_offset(&bsp, 7);
    bsp[faces + 56 + 16] = 255;
    put_i32(&mut bsp, faces + 56 + 20, -1);
    let lightmaps = json!({
        "atlasWidth": 128,
        "atlasHeight": 64,
        "faces": [{
            "faceIndex": 1,
            "w": 5,
            "h": 5,
            "atlasX": 8,
            "atlasY": 4,
            "lmVecs": [[0.0625, 0.0, 0.0, 0.0], [0.0, 0.0625, 0.0, 0.0]],
            "lmMinsS": 0,
            "lmMinsT": 0,
            "verts": [[128, 0, 0], [192, 0, 0], [192, 64, 0], [128, 64, 0]]
        }]
    });

    let result = export_bsp(&bsp, Some(lightmaps.to_string().as_bytes())).unwrap();
    assert_eq!(result.stats.lightmapped_faces, 0);
}

#[test]
fn preserves_trigger_model_but_marks_it_initially_hidden() {
    let mut bsp = synthetic_bsp(false);
    let entities = lump_offset(&bsp, 0);
    let entity_end = entities + i32::from_le_bytes(bsp[12..16].try_into().unwrap()) as usize;
    let offset = bsp[entities..entity_end]
        .windows(b"func_brush".len())
        .position(|window| window == b"func_brush")
        .unwrap()
        + entities;
    bsp[offset..offset + b"func_brush".len()].copy_from_slice(b"trigger_hu");

    let result = export_bsp(&bsp, None).unwrap();
    let gltf = glb_json(&result.glb);
    assert_eq!(gltf["nodes"][1]["extras"]["classname"], "trigger_hu");
    assert_eq!(gltf["nodes"][1]["extras"]["initiallyRendered"], false);
    assert_eq!(gltf["meshes"].as_array().unwrap().len(), 2);
}

#[test]
fn marks_sky_surfaces_hidden_without_removing_them() {
    let mut bsp = synthetic_bsp(false);
    let texinfo = lump_offset(&bsp, 6);
    put_i32(&mut bsp, texinfo + 64, 0x0004);

    let result = export_bsp(&bsp, None).unwrap();
    let gltf = glb_json(&result.glb);
    assert_eq!(result.stats.initially_rendered_faces, 0);
    assert_eq!(gltf["meshes"].as_array().unwrap().len(), 2);
    assert_eq!(
        gltf["meshes"][0]["primitives"][0]["extras"]["initiallyRendered"],
        false
    );
}

#[test]
fn texinfo_nolight_flag_prevents_lightmap_false_positive() {
    let mut bsp = synthetic_bsp(false);
    let texinfo = lump_offset(&bsp, 6);
    put_i32(&mut bsp, texinfo + 64, 0x0400);

    let lightmaps = json!({
        "atlasWidth": 128,
        "atlasHeight": 64,
        "faces": [{
            "faceIndex": 0,
            "w": 5,
            "h": 5,
            "atlasX": 8,
            "atlasY": 4,
            "lmVecs": [[0.0625, 0.0, 0.0, 0.0], [0.0, 0.0625, 0.0, 0.0]],
            "lmMinsS": 0,
            "lmMinsT": 0,
            "verts": [[0, 0, 0], [64, 0, 0], [64, 64, 0], [0, 64, 0]]
        }]
    });

    let result = export_bsp(&bsp, Some(lightmaps.to_string().as_bytes())).unwrap();
    assert_eq!(result.stats.lightmapped_faces, 0);
}

#[test]
fn exports_compiled_displacement_geometry_and_parent_face_mapping() {
    let bsp = synthetic_bsp(true);
    let lightmaps = json!({
        "atlasWidth": 128,
        "atlasHeight": 64,
        "faces": [{
            "faceIndex": 0,
            "w": 5,
            "h": 5,
            "atlasX": 8,
            "atlasY": 4,
            "lmVecs": [[0.0625, 0.0, 0.0, 0.0], [0.0, 0.0625, 0.0, 0.0]],
            "lmMinsS": 0,
            "lmMinsT": 0,
            "verts": [[0, 0, 0], [64, 0, 0], [64, 64, 0], [0, 64, 0]]
        }]
    });

    let result = export_bsp(&bsp, Some(lightmaps.to_string().as_bytes())).unwrap();
    let gltf = glb_json(&result.glb);
    let displacement = gltf["meshes"][0]["primitives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|primitive| primitive["extras"]["geometry"] == "displacement")
        .unwrap();
    let attributes = &displacement["attributes"];
    let positions = read_f32_accessor(
        &result.glb,
        &gltf,
        attributes["POSITION"].as_u64().unwrap() as usize,
    );
    let normals = read_f32_accessor(
        &result.glb,
        &gltf,
        attributes["NORMAL"].as_u64().unwrap() as usize,
    );
    let uv0 = read_f32_accessor(
        &result.glb,
        &gltf,
        attributes["TEXCOORD_0"].as_u64().unwrap() as usize,
    );
    let uv1 = read_f32_accessor(
        &result.glb,
        &gltf,
        attributes["TEXCOORD_1"].as_u64().unwrap() as usize,
    );
    let alpha = read_f32_accessor(
        &result.glb,
        &gltf,
        attributes["_DISPLACEMENT_ALPHA"].as_u64().unwrap() as usize,
    );
    let indices = read_u32_accessor(
        &result.glb,
        &gltf,
        displacement["indices"].as_u64().unwrap() as usize,
    );

    assert_eq!(result.stats.displacement_faces, 1);
    assert_eq!(result.stats.faces, 2);
    assert_eq!(result.stats.vertices, 29);
    assert_eq!(result.stats.triangles, 34);
    assert_eq!(&positions[6 * 3..6 * 3 + 3], &[20.0, 0.0, -16.0]);
    assert_eq!(&positions[12 * 3..12 * 3 + 3], &[32.0, 16.0, -32.0]);
    assert_eq!(&uv0[6 * 2..6 * 2 + 2], &[0.25, 0.25]);
    assert_eq!(&uv1[0..2], &[8.5 / 128.0, 4.5 / 64.0]);
    assert_eq!(
        alpha,
        (0..25).map(|index| index as f32 * 10.0).collect::<Vec<_>>()
    );
    assert_eq!(indices.len(), 32 * 3);
    assert_eq!(
        &indices[0..24],
        &[
            0, 5, 6, 1, 0, 6, 2, 1, 6, 7, 2, 6, 12, 7, 6, 11, 12, 6, 10, 11, 6, 5, 10, 6
        ]
    );
    assert_eq!(displacement["extras"]["bspFaceIndices"], json!([0]));
    assert_eq!(displacement["extras"]["bspDispInfoIndices"], json!([0]));
    assert_eq!(
        displacement["extras"]["bspDisplacementTriangleTags"]
            .as_array()
            .unwrap()[0],
        json!(
            (0_usize..32)
                .map(|index| 1 | if index.is_multiple_of(2) { 2 } else { 4 })
                .collect::<Vec<_>>()
        )
    );
    for normal in normals.chunks_exact(3) {
        let length = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 1e-5, "non-unit normal: {normal:?}");
        assert!(normal[1] > 0.0, "inward normal: {normal:?}");
    }
}

#[test]
fn omits_removed_displacement_triangles_but_retains_their_source_tags() {
    let mut bsp = synthetic_bsp(true);
    let disptris = lump_offset(&bsp, 48);
    put_u16(&mut bsp, disptris, 1 | 32);

    let result = export_bsp(&bsp, None).unwrap();
    let gltf = glb_json(&result.glb);
    let displacement = gltf["meshes"][0]["primitives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|primitive| primitive["extras"]["geometry"] == "displacement")
        .unwrap();
    let exported_tags = &displacement["extras"]["bspDisplacementTriangleTags"][0];
    let source_tags = &displacement["extras"]["bspDisplacementSourceTriangleTags"][0];

    assert_eq!(result.stats.displacement_triangles, 31);
    assert_eq!(exported_tags.as_array().unwrap().len(), 31);
    assert!(
        exported_tags
            .as_array()
            .unwrap()
            .iter()
            .all(|tag| tag.as_u64().unwrap() & 32 == 0)
    );
    assert_eq!(source_tags.as_array().unwrap().len(), 32);
    assert_eq!(source_tags[0], 33);
}

#[test]
fn reports_optional_feature_capabilities_without_claiming_export_support() {
    let result = export_bsp(&synthetic_bsp(true), None).unwrap();
    let stats = serde_json::to_value(&result.stats).unwrap();

    assert_eq!(stats["capabilities"]["displacements"]["present"], true);
    assert_eq!(stats["capabilities"]["displacements"]["count"], 1);
    assert_eq!(stats["capabilities"]["displacements"]["status"], "exported");
    assert_eq!(stats["capabilities"]["overlays"]["count"], 1);
    assert_eq!(stats["capabilities"]["overlays"]["status"], "detectedOnly");
    assert_eq!(stats["capabilities"]["waterOverlays"]["count"], 1);
    assert_eq!(
        stats["capabilities"]["waterOverlays"]["status"],
        "detectedOnly"
    );
    assert_eq!(stats["capabilities"]["cubemaps"]["count"], 1);
    assert_eq!(stats["capabilities"]["cubemaps"]["status"], "detectedOnly");
}

#[test]
fn reports_unsupported_optional_metadata_versions_without_claiming_support() {
    let mut bsp = synthetic_bsp(true);
    set_lump_version(&mut bsp, 45, 7);

    let result = export_bsp(&bsp, None).unwrap();
    let stats = serde_json::to_value(&result.stats).unwrap();

    assert_eq!(
        stats["capabilities"]["overlays"]["lumpVersions"]["OVERLAYS"],
        7
    );
    assert_eq!(
        stats["capabilities"]["overlays"]["status"],
        "unsupportedVersion"
    );
    assert_eq!(stats["capabilities"]["overlays"]["count"], Value::Null);
}

#[test]
fn rejects_unsupported_displacement_lump_versions() {
    let mut bsp = synthetic_bsp(true);
    set_lump_version(&mut bsp, 26, 1);

    let error = export_bsp(&bsp, None).unwrap_err();
    assert!(error.contains("DISPINFO lump version 1"), "{error}");
}

#[test]
#[ignore = "requires BSP_TO_GLB_LOCAL_DISP_MAP to point to a locally installed map"]
fn exports_a_local_displacement_map() {
    let path = std::env::var("BSP_TO_GLB_LOCAL_DISP_MAP")
        .expect("BSP_TO_GLB_LOCAL_DISP_MAP must name a local BSP");
    let bsp = std::fs::read(&path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"));

    let result = export_bsp(&bsp, None).unwrap();
    let stats = serde_json::to_value(&result.stats).unwrap();

    assert!(result.stats.displacement_faces > 0);
    assert!(result.stats.displacement_vertices > result.stats.displacement_faces * 4);
    assert!(result.stats.displacement_triangles > 0);
    assert_eq!(&result.glb[0..4], b"glTF");
    assert_eq!(stats["capabilities"]["displacements"]["status"], "exported");
}

#[test]
fn no_displacement_export_remains_byte_and_metric_stable() {
    let result = export_bsp(&synthetic_bsp(false), None).unwrap();

    assert_eq!(fnv1a64(&result.glb), 9_571_780_838_537_332_067);
    assert_eq!(result.stats.faces, 2);
    assert_eq!(result.stats.vertices, 8);
    assert_eq!(result.stats.triangles, 4);
    assert_eq!(result.stats.displacement_faces, 0);
}

#[test]
fn matches_three_decimal_lightmap_vertices_without_f32_quantization_loss() {
    let mut bsp = synthetic_bsp(false);
    let vertex_header = 8 + 3 * 16;
    let vertex_lump =
        i32::from_le_bytes(bsp[vertex_header..vertex_header + 4].try_into().unwrap()) as usize;
    put_f32(&mut bsp, vertex_lump, -335.1875);
    let lightmaps = json!({
        "atlasWidth": 128,
        "atlasHeight": 64,
        "faces": [{
            "faceIndex": 0,
            "w": 5,
            "h": 5,
            "atlasX": 8,
            "atlasY": 4,
            "lmVecs": [[0.0625, 0.0, 0.0, 0.0], [0.0, 0.0625, 0.0, 0.0]],
            "lmMinsS": 0,
            "lmMinsT": 0,
            "verts": [[-335.187, 0, 0], [64, 0, 0], [64, 64, 0], [0, 64, 0]]
        }]
    });

    let result = export_bsp(&bsp, Some(lightmaps.to_string().as_bytes())).unwrap();
    assert_eq!(result.stats.lightmapped_faces, 1);
}

#[test]
fn exports_pak_backed_source_material_manifest_and_safe_gltf_flags() {
    let mut bsp = synthetic_bsp(false);
    append_pak(
        &mut bsp,
        &[
            (
                "materials/brick/test.vmt",
                br#"LightmappedGeneric {
                    "$basetexture" "brick/test_diffuse"
                    "$translucent" 1
                    "$nocull" 1
                    "$selfillum" 1
                    Proxies { Sine { resultVar "$alpha" } }
                }"#,
            ),
            ("materials/brick/test_diffuse.vtf", b"synthetic-vtf"),
        ],
    );

    let result = export_bsp(&bsp, None).unwrap();
    let manifest = serde_json::to_value(&result.material_manifest).unwrap();
    let gltf = glb_json(&result.glb);

    assert_eq!(manifest["schemaVersion"], 3);
    assert_eq!(manifest["materials"][0]["name"], "brick/test");
    assert_eq!(
        manifest["materials"][0]["metadata"]["shader"]["family"],
        "lightmappedGeneric"
    );
    assert_eq!(
        manifest["materials"][0]["metadata"]["unsupported"]["proxies"][0],
        "Sine"
    );
    assert_eq!(manifest["unresolvedAssets"].as_array().unwrap().len(), 0);
    assert_eq!(gltf["materials"][0]["doubleSided"], true);
    assert_eq!(gltf["materials"][0]["alphaMode"], "BLEND");
    assert_eq!(
        gltf["materials"][0]["extras"]["sourceMaterialManifestIndex"],
        0
    );
    assert!(gltf["materials"][0].get("emissiveFactor").is_none());
}

#[test]
fn cli_writes_requested_versioned_material_sidecar() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "bsp-to-glb-material-test-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let bsp_path = directory.join("fixture.bsp");
    let glb_path = directory.join("fixture.glb");
    let manifest_path = directory.join("fixture.materials.json");
    fs::write(&bsp_path, synthetic_bsp(false)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .args([
            "--bsp",
            bsp_path.to_str().unwrap(),
            "--out",
            glb_path.to_str().unwrap(),
            "--material-manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 3);
    assert_eq!(manifest["lookupPolicy"], "pakFirst");
    assert_eq!(
        manifest["unresolvedAssets"][0]["lookupPath"],
        "materials/brick/test.vmt"
    );
    assert!(glb_path.is_file());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_writes_content_addressed_material_texture_package() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "bsp-to-glb-texture-test-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let bsp_path = directory.join("fixture.bsp");
    let glb_path = directory.join("fixture.glb");
    let material_manifest_path = directory.join("fixture.materials.json");
    let texture_directory = directory.join("textures");
    let texture_manifest_path = texture_directory.join("manifest.json");
    let mut bsp = synthetic_bsp(false);
    let texture = rgba_vtf([11, 22, 33, 44]);
    append_pak(
        &mut bsp,
        &[
            (
                "materials/brick/test.vmt",
                br#"LightmappedGeneric { "$basetexture" "brick/test" }"#,
            ),
            ("materials/brick/test.vtf", &texture),
        ],
    );
    fs::write(&bsp_path, bsp).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .args([
            "--bsp",
            bsp_path.to_str().unwrap(),
            "--out",
            glb_path.to_str().unwrap(),
            "--material-manifest",
            material_manifest_path.to_str().unwrap(),
            "--texture-output",
            texture_directory.to_str().unwrap(),
            "--texture-manifest",
            texture_manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let textures: Value =
        serde_json::from_slice(&fs::read(&texture_manifest_path).unwrap()).unwrap();
    let materials: Value =
        serde_json::from_slice(&fs::read(&material_manifest_path).unwrap()).unwrap();
    let stats: Value = serde_json::from_slice(&output.stdout).unwrap();
    let file_name = textures["outputs"][0]["fileName"].as_str().unwrap();
    assert!(file_name.starts_with("sha256-"));
    assert!(file_name.ends_with(".png"));
    assert!(texture_directory.join(file_name).is_file());
    assert_eq!(textures["sources"][0]["status"], "decoded");
    assert_eq!(stats["decodedMaterialTextures"], 1);
    assert_eq!(stats["uniqueMaterialTextureOutputs"], 1);
    assert_eq!(
        materials["materials"][0]["textures"][0]["packageSourceIndex"],
        0
    );

    fs::remove_dir_all(directory).unwrap();
}
