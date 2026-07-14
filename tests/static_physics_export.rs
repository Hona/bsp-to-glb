use bsp_to_glb::phy::{DecodeLimits, PhyShapeStatus};
use bsp_to_glb::static_physics::{
    STATIC_PHYSICS_BINARY_VERSION, StaticPhysicsLimits, decode_shape_bundle,
    encode_phy_shape_bundle, export_bsp_static_physics,
};

const HEADER_BYTES: usize = 4 + 4 + 64 * 16 + 4;
const VPHY: u32 = u32::from_le_bytes(*b"VPHY");
const IVPS: u32 = u32::from_le_bytes(*b"IVPS");

fn put_i16(data: &mut [u8], offset: usize, value: i16) {
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

fn compact_surface() -> Vec<u8> {
    let mut bytes = vec![0_u8; 156];
    put_u32(&mut bytes, 28, (156_u32 << 8) | 3);
    put_i32(&mut bytes, 32, 48);
    put_i32(&mut bytes, 44, IVPS as i32);
    put_i32(&mut bytes, 48, 0);
    put_i32(&mut bytes, 52, 28);
    put_i32(&mut bytes, 76, 32);
    put_i32(&mut bytes, 80, 7);
    put_u32(&mut bytes, 84, 5 << 8);
    put_i16(&mut bytes, 88, 1);
    put_u32(&mut bytes, 92, 6 << 24);
    put_u32(&mut bytes, 96, 0);
    put_u32(&mut bytes, 100, 1);
    put_u32(&mut bytes, 104, 2);
    for (index, point) in [[0.0, 0.0, 0.0], [0.0254, 0.0, 0.0], [0.0, 0.0, 0.0254]]
        .into_iter()
        .enumerate()
    {
        for (axis, value) in point.into_iter().enumerate() {
            put_f32(&mut bytes, 108 + index * 16 + axis * 4, value);
        }
    }
    bytes
}

fn collision_data() -> Vec<u8> {
    let surface = compact_surface();
    let mut body = Vec::new();
    body.extend_from_slice(&VPHY.to_le_bytes());
    body.extend_from_slice(&0x0100_i16.to_le_bytes());
    body.extend_from_slice(&0_i16.to_le_bytes());
    body.extend_from_slice(&(surface.len() as i32).to_le_bytes());
    body.extend_from_slice(&[0_u8; 12]);
    body.extend_from_slice(&0_i32.to_le_bytes());
    body.extend_from_slice(&surface);
    let mut framed = Vec::new();
    framed.extend_from_slice(&(body.len() as i32).to_le_bytes());
    framed.extend_from_slice(&body);
    framed
}

fn synthetic_bsp() -> Vec<u8> {
    let collision = collision_data();
    let key_data = b"solid { \"index\" \"0\" \"mass\" \"5\" \"unknown\" \"kept\" }\0";
    let mut lump = Vec::new();
    lump.extend_from_slice(&3_i32.to_le_bytes());
    lump.extend_from_slice(&(collision.len() as i32).to_le_bytes());
    lump.extend_from_slice(&(key_data.len() as i32).to_le_bytes());
    lump.extend_from_slice(&1_i32.to_le_bytes());
    lump.extend_from_slice(&collision);
    lump.extend_from_slice(key_data);
    lump.extend_from_slice(&(-1_i32).to_le_bytes());
    lump.extend_from_slice(&(-1_i32).to_le_bytes());
    lump.extend_from_slice(&0_i32.to_le_bytes());
    lump.extend_from_slice(&0_i32.to_le_bytes());

    let mut bsp = vec![0_u8; HEADER_BYTES];
    bsp[0..4].copy_from_slice(b"VBSP");
    put_i32(&mut bsp, 4, 20);
    let offset = bsp.len();
    bsp.extend_from_slice(&lump);
    let header = 8 + 29 * 16;
    put_i32(&mut bsp, header, offset as i32);
    put_i32(&mut bsp, header + 4, lump.len() as i32);
    bsp
}

#[test]
fn encodes_a_deterministic_engine_neutral_shape_bundle() {
    let exported = export_bsp_static_physics(&synthetic_bsp(), StaticPhysicsLimits::default())
        .expect("synthetic BSP physics should export");
    assert_eq!(exported.manifest.schema, "bsp-to-glb/static-physics");
    assert_eq!(exported.manifest.schema_version, 1);
    assert_eq!(exported.manifest.coordinate_system, "Source XYZ");
    assert_eq!(exported.manifest.stats.models, 1);
    assert_eq!(exported.manifest.stats.decoded_solids, 1);
    assert_eq!(exported.manifest.stats.convexes, 1);
    assert_eq!(exported.manifest.stats.vertices, 3);
    assert_eq!(exported.manifest.stats.faces, 1);
    assert_eq!(exported.manifest.models[0].model_index, 3);
    assert_eq!(exported.manifest.models[0].solids[0].shape_index, Some(0));
    assert_eq!(
        exported.manifest.models[0].key_data.solids[0].mass,
        Some(5.0)
    );
    assert_eq!(exported.binary.len(), 208);

    let decoded = decode_shape_bundle(&exported.binary, StaticPhysicsLimits::default())
        .expect("encoded binary should validate");
    assert_eq!(decoded.version, STATIC_PHYSICS_BINARY_VERSION);
    assert_eq!(decoded.shapes[0].source_id, 3);
    assert_eq!(decoded.shapes[0].solid.status, PhyShapeStatus::Decoded);
    assert_eq!(
        decoded.shapes[0].solid.convexes[0].faces[0].indices,
        [2, 1, 0]
    );
    assert_eq!(
        decoded.shapes[0].solid.convexes[0].faces[0].material_index,
        6
    );
    assert!(!decoded.shapes[0].solid.convexes[0].faces[0].is_virtual);

    let repeated =
        export_bsp_static_physics(&synthetic_bsp(), StaticPhysicsLimits::default()).unwrap();
    assert_eq!(repeated.binary, exported.binary);
    assert_eq!(
        repeated.manifest.binary.sha256,
        exported.manifest.binary.sha256
    );
}

#[test]
fn shape_bundle_rejects_truncation_and_declared_count_exhaustion() {
    let exported =
        export_bsp_static_physics(&synthetic_bsp(), StaticPhysicsLimits::default()).unwrap();
    for end in 0..exported.binary.len() {
        assert!(
            decode_shape_bundle(&exported.binary[..end], StaticPhysicsLimits::default()).is_err(),
            "prefix {end} unexpectedly decoded"
        );
    }
    let limits = StaticPhysicsLimits {
        max_total_vertices: 2,
        ..StaticPhysicsLimits::default()
    };
    assert!(export_bsp_static_physics(&synthetic_bsp(), limits).is_err());

    let mut invalid_face = exported.binary.clone();
    let face_offset = u32::from_le_bytes(invalid_face[48..52].try_into().unwrap()) as usize;
    put_u32(&mut invalid_face, face_offset, 99);
    assert!(decode_shape_bundle(&invalid_face, StaticPhysicsLimits::default()).is_err());
}

#[test]
fn generic_phy_bundle_uses_caller_owned_source_ids() {
    let collide =
        bsp_to_glb::phy::decode_physcollide(&collision_data(), b"", 1, DecodeLimits::default())
            .unwrap();
    let encoded = encode_phy_shape_bundle(41, &collide.solids, StaticPhysicsLimits::default())
        .expect("generic PHY shapes should encode");
    let decoded = decode_shape_bundle(&encoded.binary, StaticPhysicsLimits::default()).unwrap();
    assert_eq!(decoded.shapes[0].source_id, 41);
    assert_eq!(encoded.solid_shape_indices, vec![Some(0)]);
}
