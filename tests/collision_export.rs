use bsp_to_glb::{CollisionExportInput, StaticPropCollisionInput, export_collision_sidecar};
use serde_json::Value;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn synthetic_collision_bsp() -> Vec<u8> {
    let mut lumps = vec![Vec::<u8>::new(); 64];
    let mut versions = [0_i32; 64];
    lumps[0] = br#"
{
"classname" "worldspawn"
}
{
"classname" "func_brush"
"targetname" "collision_only"
"model" "*1"
"origin" "128 32 16"
"angles" "0 90 0"
}
"#
    .to_vec();

    let plane_values = [
        ([1.0, 0.0, 0.0], 64.0, 0),
        ([-1.0, 0.0, 0.0], 0.0, 0),
        ([0.0, 1.0, 0.0], 64.0, 1),
        ([0.0, -1.0, 0.0], 0.0, 1),
        ([0.0, 0.0, 1.0], 64.0, 2),
        ([0.0, 0.0, -1.0], 0.0, 2),
    ];
    let mut planes = vec![0; plane_values.len() * 20];
    for (index, (normal, distance, plane_type)) in plane_values.iter().enumerate() {
        let offset = index * 20;
        for (axis, value) in normal.iter().enumerate() {
            put_f32(&mut planes, offset + axis * 4, *value);
        }
        put_f32(&mut planes, offset + 12, *distance);
        put_i32(&mut planes, offset + 16, *plane_type);
    }
    lumps[1] = planes;

    let mut nodes = vec![0; 2 * 32];
    put_i32(&mut nodes, 0, 0);
    put_i32(&mut nodes, 4, -1);
    put_i32(&mut nodes, 8, -1);
    put_i32(&mut nodes, 32, 1);
    put_i32(&mut nodes, 36, -2);
    put_i32(&mut nodes, 40, -2);
    lumps[5] = nodes;

    let mut leaves = vec![0; 2 * 32];
    put_i32(&mut leaves, 0, 0x1);
    put_i16(&mut leaves, 4, 0);
    put_u16(&mut leaves, 24, 0);
    put_u16(&mut leaves, 26, 1);
    put_i32(&mut leaves, 32, 0x1_0001);
    put_i16(&mut leaves, 36, 1);
    put_u16(&mut leaves, 32 + 24, 1);
    put_u16(&mut leaves, 32 + 26, 1);
    lumps[10] = leaves;
    versions[10] = 1;

    lumps[17] = [0_u16, 1_u16]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect();

    let mut brushes = vec![0; 2 * 12];
    put_i32(&mut brushes, 0, 0);
    put_i32(&mut brushes, 4, 3);
    put_i32(&mut brushes, 8, 0x1);
    put_i32(&mut brushes, 12, 3);
    put_i32(&mut brushes, 16, 3);
    put_i32(&mut brushes, 20, 0x1_0001);
    lumps[18] = brushes;

    let mut brush_sides = vec![0; 6 * 8];
    for side in 0..6 {
        let offset = side * 8;
        put_u16(&mut brush_sides, offset, side as u16);
        put_i16(&mut brush_sides, offset + 2, side as i16 - 1);
        put_i16(&mut brush_sides, offset + 4, -1);
        put_i16(&mut brush_sides, offset + 6, i16::from(side == 5));
    }
    lumps[19] = brush_sides;

    let mut models = vec![0; 2 * 48];
    put_i32(&mut models, 36, 0);
    put_i32(&mut models, 40, 0);
    put_i32(&mut models, 44, 0);
    put_f32(&mut models, 48 + 24, 128.0);
    put_f32(&mut models, 48 + 28, 32.0);
    put_f32(&mut models, 48 + 32, 16.0);
    put_i32(&mut models, 48 + 36, 1);
    put_i32(&mut models, 48 + 40, 0);
    put_i32(&mut models, 48 + 44, 0);
    lumps[14] = models;

    let mut physics = Vec::new();
    physics.extend_from_slice(&0_i32.to_le_bytes());
    physics.extend_from_slice(&4_i32.to_le_bytes());
    physics.extend_from_slice(&4_i32.to_le_bytes());
    physics.extend_from_slice(&1_i32.to_le_bytes());
    physics.extend_from_slice(&[1, 2, 3, 4]);
    physics.extend_from_slice(b"a=b\0");
    physics.extend_from_slice(&1_i32.to_le_bytes());
    physics.extend_from_slice(&3_i32.to_le_bytes());
    physics.extend_from_slice(&0_i32.to_le_bytes());
    physics.extend_from_slice(&2_i32.to_le_bytes());
    physics.extend_from_slice(&[5, 6, 7]);
    physics.extend_from_slice(&(-1_i32).to_le_bytes());
    physics.extend_from_slice(&0_i32.to_le_bytes());
    physics.extend_from_slice(&0_i32.to_le_bytes());
    physics.extend_from_slice(&0_i32.to_le_bytes());
    lumps[29] = physics;

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
        put_i32(&mut bsp, header + 8, versions[index]);
    }
    bsp
}

fn sidecar_json(bsp: &[u8], input: &CollisionExportInput<'_>) -> (Value, usize) {
    let result = export_collision_sidecar(bsp, input).unwrap();
    (
        serde_json::from_slice(&result.json).unwrap(),
        result.stats.world_model_brushes,
    )
}

#[test]
fn exports_direct_versioned_brush_collision_and_model_ownership() {
    let props = [StaticPropCollisionInput {
        prop_index: 7,
        model_name: "models/props/test.mdl".to_owned(),
        solid_mode: 6,
    }];
    let input = CollisionExportInput {
        static_props: Some(&props),
    };
    let bsp = synthetic_collision_bsp();
    let result = export_collision_sidecar(&bsp, &input).unwrap();
    let sidecar: Value = serde_json::from_slice(&result.json).unwrap();

    assert_eq!(sidecar["schema"], "bsp-to-glb/collision");
    assert_eq!(sidecar["version"], 1);
    assert_eq!(sidecar["geometrySource"], "bspBrushes");
    assert_eq!(sidecar["renderTriangleSubstitution"], false);
    assert_eq!(sidecar["coordinateSystem"], "Source XYZ");
    assert_eq!(sidecar["planes"].as_array().unwrap().len(), 6);
    assert_eq!(sidecar["planes"][0]["distance"], 64.0);
    assert_eq!(sidecar["planes"][0]["planeType"], 0);
    assert_eq!(sidecar["brushSides"].as_array().unwrap().len(), 6);
    assert_eq!(sidecar["brushSides"][5]["bevel"], 1);
    assert_eq!(sidecar["leafBrushes"], serde_json::json!([0, 1]));
    assert_eq!(sidecar["leaves"][1]["contents"], 0x1_0001_u32);
    assert_eq!(sidecar["brushes"][1]["contents"], 0x1_0001_u32);
    assert_eq!(sidecar["brushes"][1]["playerClip"], true);
    assert_eq!(
        sidecar["brushes"][1]["modelIndices"],
        serde_json::json!([1])
    );
    assert_eq!(sidecar["models"][0]["brushIndices"], serde_json::json!([0]));
    assert_eq!(sidecar["models"][1]["brushIndices"], serde_json::json!([1]));
    assert_eq!(sidecar["models"][1]["numRenderFaces"], 0);
    assert_eq!(sidecar["models"][1]["classname"], "func_brush");
    assert_eq!(sidecar["models"][1]["targetname"], "collision_only");
    assert_eq!(sidecar["staticProps"][0]["propIndex"], 7);
    assert_eq!(sidecar["staticProps"][0]["solidMode"], 6);
    assert_eq!(sidecar["staticPropInputAvailable"], true);

    assert_eq!(result.stats.brushes, 2);
    assert_eq!(result.stats.brush_sides, 6);
    assert_eq!(result.stats.world_model_brushes, 1);
    assert_eq!(result.stats.player_clip_brushes, 1);
    assert_eq!(result.stats.models, 2);
}

#[test]
fn preserves_raw_physcollide_model_blocks_without_claiming_decoding() {
    let (sidecar, _) = sidecar_json(&synthetic_collision_bsp(), &CollisionExportInput::default());

    assert_eq!(sidecar["physicsCollision"]["decodeStatus"], "unsupported");
    assert_eq!(sidecar["physicsCollision"]["rawEncoding"], "base64");
    assert_eq!(
        sidecar["physicsCollision"]["blocks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(sidecar["physicsCollision"]["blocks"][0]["modelIndex"], 0);
    assert_eq!(sidecar["physicsCollision"]["blocks"][0]["dataSize"], 4);
    assert_eq!(sidecar["physicsCollision"]["blocks"][0]["keyDataSize"], 4);
    assert_eq!(sidecar["physicsCollision"]["blocks"][0]["solidCount"], 1);
    assert_eq!(
        sidecar["physicsCollision"]["blocks"][0]["collisionDataBase64"],
        "AQIDBA=="
    );
    assert_eq!(
        sidecar["physicsCollision"]["blocks"][0]["keyDataBase64"],
        "YT1iAA=="
    );
    assert_eq!(sidecar["physicsCollision"]["blocks"][1]["modelIndex"], 1);
    assert_eq!(sidecar["physicsCollision"]["terminatorPresent"], true);
}

#[test]
fn marks_static_prop_input_unavailable_when_game_lump_data_is_not_supplied() {
    let (sidecar, _) = sidecar_json(&synthetic_collision_bsp(), &CollisionExportInput::default());
    assert_eq!(sidecar["staticPropInputAvailable"], false);
    assert_eq!(sidecar["staticProps"], serde_json::json!([]));
}

#[test]
fn rejects_leafbrush_references_outside_the_brush_lump() {
    let mut bsp = synthetic_collision_bsp();
    let leafbrush_header = 8 + 17 * 16;
    let leafbrush_offset = i32::from_le_bytes(
        bsp[leafbrush_header..leafbrush_header + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    put_u16(&mut bsp, leafbrush_offset, 99);

    let error = export_collision_sidecar(&bsp, &CollisionExportInput::default()).unwrap_err();
    assert!(error.contains("leaf 0"), "unexpected error: {error}");
    assert!(error.contains("brush 99"), "unexpected error: {error}");
}

#[test]
fn cli_writes_collision_sidecar_without_requiring_render_export() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("bsp-to-glb-collision-{nonce}"));
    fs::create_dir_all(&directory).unwrap();
    let bsp_path = directory.join("synthetic.bsp");
    let collision_path = directory.join("synthetic.collision.json");
    fs::write(&bsp_path, synthetic_collision_bsp()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .arg("--bsp")
        .arg(&bsp_path)
        .arg("--collision-out")
        .arg(&collision_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sidecar: Value = serde_json::from_slice(&fs::read(&collision_path).unwrap()).unwrap();
    assert_eq!(sidecar["schema"], "bsp-to-glb/collision");
    assert_eq!(sidecar["brushes"].as_array().unwrap().len(), 2);
    fs::remove_dir_all(directory).unwrap();
}
