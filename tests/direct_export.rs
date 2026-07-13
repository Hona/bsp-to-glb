use bsp_to_glb::export_bsp;
use serde_json::{Value, json};
use std::io::Cursor;

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

    let mut planes = vec![0; 20];
    put_f32(&mut planes, 8, 1.0);
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
            if displacement && face == 1 { 0 } else { -1 },
        );
        faces[offset + 16] = 0;
        faces[offset + 17..offset + 20].fill(255);
        put_i32(&mut faces, offset + 20, 0);
        put_i32(&mut faces, offset + 36, 4);
        put_i32(&mut faces, offset + 40, 4);
    }
    put_u16(&mut faces, 48, 1);
    put_u16(&mut faces, 50, 0);
    lumps[7] = faces;

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
    }
    bsp
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
fn rejects_displacements_instead_of_dropping_them() {
    let error = export_bsp(&synthetic_bsp(true), None).unwrap_err();
    assert!(error.contains("displacement"), "unexpected error: {error}");
    assert!(error.contains("face 1"), "unexpected error: {error}");
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
