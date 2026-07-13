use bsp_to_glb::export_bsp;
use serde_json::{Value, json};

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
            if displacement && face == 0 { 0 } else { -1 },
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
    }
    bsp
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

fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
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

    assert_eq!(fnv1a64(&result.glb), 3_257_560_727_136_978_702);
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
