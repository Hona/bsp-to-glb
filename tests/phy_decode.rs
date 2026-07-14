use bsp_to_glb::phy::{
    DecodeLimits, PhyKeyValue, PhyShapeStatus, PhySolidEncoding, UnsupportedShapeKind, decode_phy,
    decode_physcollide,
};

const VPHY: u32 = u32::from_le_bytes(*b"VPHY");
const YHPV: u32 = u32::from_le_bytes(*b"YHPV");
const IVPS: u32 = u32::from_le_bytes(*b"IVPS");
const MOPP: u32 = u32::from_le_bytes(*b"MOPP");

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_i16(bytes: &mut [u8], offset: usize, value: i16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_f32(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn compact_surface(identifier: u32) -> Vec<u8> {
    let mut bytes = vec![0_u8; 156];
    for (index, value) in [0.0254, 0.0508, -0.0762].into_iter().enumerate() {
        write_f32(&mut bytes, index * 4, value);
    }
    for (index, value) in [4.0, 5.0, 6.0].into_iter().enumerate() {
        write_f32(&mut bytes, 12 + index * 4, value);
    }
    write_f32(&mut bytes, 24, 0.254);
    write_u32(&mut bytes, 28, (156_u32 << 8) | 7);
    write_i32(&mut bytes, 32, 48);
    write_i32(&mut bytes, 44, identifier as i32);

    let node = 48;
    write_i32(&mut bytes, node, 0);
    write_i32(&mut bytes, node + 4, 28);
    write_f32(&mut bytes, node + 8, 0.1);
    write_f32(&mut bytes, node + 12, 0.2);
    write_f32(&mut bytes, node + 16, 0.3);
    write_f32(&mut bytes, node + 20, 1.0);

    let ledge = 76;
    write_i32(&mut bytes, ledge, 32);
    write_i32(&mut bytes, ledge + 4, 99);
    write_u32(&mut bytes, ledge + 8, 5 << 8);
    write_i16(&mut bytes, ledge + 12, 1);

    let triangle = 92;
    write_u32(&mut bytes, triangle, 1 | (2 << 12) | (5 << 24));
    write_u32(&mut bytes, triangle + 4, 0);
    write_u32(&mut bytes, triangle + 8, 1);
    write_u32(&mut bytes, triangle + 12, 2);

    for (index, point) in [[0.0254, 0.0, 0.0], [0.0, 0.0, 0.0254], [0.0, -0.0254, 0.0]]
        .into_iter()
        .enumerate()
    {
        for (axis, value) in point.into_iter().enumerate() {
            write_f32(&mut bytes, 108 + index * 16 + axis * 4, value);
        }
        write_f32(&mut bytes, 108 + index * 16 + 12, 1.0);
    }
    bytes
}

fn phy_with_solid(modern_type: Option<i16>, legacy_identifier: u32, keydata: &str) -> Vec<u8> {
    let surface = compact_surface(legacy_identifier);
    let mut body = Vec::new();
    if let Some(model_type) = modern_type {
        body.extend_from_slice(&VPHY.to_le_bytes());
        body.extend_from_slice(&0x0100_i16.to_le_bytes());
        body.extend_from_slice(&model_type.to_le_bytes());
        body.extend_from_slice(&(surface.len() as i32).to_le_bytes());
        body.extend_from_slice(
            &[1.0_f32, 2.0, 3.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        body.extend_from_slice(&0_i32.to_le_bytes());
    }
    body.extend_from_slice(&surface);

    let mut phy = Vec::new();
    phy.extend_from_slice(&16_i32.to_le_bytes());
    phy.extend_from_slice(&0_i32.to_le_bytes());
    phy.extend_from_slice(&1_i32.to_le_bytes());
    phy.extend_from_slice(&0x1234_5678_i32.to_le_bytes());
    phy.extend_from_slice(&(body.len() as i32).to_le_bytes());
    phy.extend_from_slice(&body);
    phy.extend_from_slice(keydata.as_bytes());
    phy.push(0);
    phy
}

const KEYDATA: &str = r#"
solid {
  "index" "0"
  "name" "fixture"
  "mass" "12.5"
  "surfaceprop" "metal"
  "masscenteroverride" "1 2 3"
  "drag" "0.75"
  "mystery" "kept π"
}
custom {
  "foo" "7"
}
"#;

#[test]
fn decodes_modern_polygon_geometry_metadata_and_typed_keydata() {
    let decoded = decode_phy(
        &phy_with_solid(Some(0), IVPS, KEYDATA),
        DecodeLimits::default(),
    )
    .expect("modern polygon fixture should decode");

    assert_eq!(decoded.header.header_size, 16);
    assert_eq!(decoded.header.solid_count, 1);
    assert_eq!(decoded.header.checksum, 0x1234_5678);
    assert_eq!(decoded.solids[0].encoding, PhySolidEncoding::ModernPolygon);
    assert_eq!(decoded.solids[0].status, PhyShapeStatus::Decoded);
    assert_eq!(decoded.solids[0].drag_axis_areas, Some([1.0, 2.0, 3.0]));
    assert_eq!(decoded.solids[0].center_of_mass, [1.0, -3.0, -2.0]);
    assert_eq!(decoded.solids[0].rotation_inertia, [4.0, 6.0, 5.0]);
    assert!((decoded.solids[0].upper_limit_radius - 10.0).abs() < 1e-4);
    assert_eq!(decoded.solids[0].convexes.len(), 1);
    assert_eq!(decoded.solids[0].convexes[0].client_data, 99);
    assert_eq!(
        decoded.solids[0].convexes[0].vertices,
        vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );
    assert_eq!(decoded.solids[0].convexes[0].faces[0].indices, [2, 1, 0]);
    assert_eq!(decoded.solids[0].convexes[0].faces[0].material_index, 5);
    assert!(!decoded.solids[0].convexes[0].faces[0].is_virtual);
    assert_eq!(decoded.key_data.solids[0].index, Some(0));
    assert_eq!(decoded.key_data.solids[0].name.as_deref(), Some("fixture"));
    assert_eq!(decoded.key_data.solids[0].mass, Some(12.5));
    assert_eq!(
        decoded.key_data.solids[0].surface_prop.as_deref(),
        Some("metal")
    );
    assert_eq!(
        decoded.key_data.solids[0].mass_center_override,
        Some([1.0, 2.0, 3.0])
    );
    assert_eq!(decoded.key_data.solids[0].drag, Some(0.75));
    assert!(decoded.key_data.solids[0].unknown.iter().any(|entry| matches!(entry, PhyKeyValue::Scalar { key, value } if key == "mystery" && value == "kept π")));
    assert_eq!(decoded.key_data.unknown_blocks[0].name, "custom");
}

#[test]
fn decodes_legacy_compact_surface_with_identical_winding() {
    let decoded = decode_phy(&phy_with_solid(None, IVPS, ""), DecodeLimits::default())
        .expect("legacy compact fixture should decode");
    assert_eq!(decoded.solids[0].encoding, PhySolidEncoding::LegacyCompact);
    assert_eq!(decoded.solids[0].status, PhyShapeStatus::Decoded);
    assert_eq!(decoded.solids[0].convexes[0].faces[0].indices, [2, 1, 0]);
    assert_eq!(decoded.solids[0].drag_axis_areas, None);
}

#[test]
fn decodes_the_same_framed_stream_from_a_physcollide_block() {
    let phy = phy_with_solid(Some(0), IVPS, KEYDATA);
    let solid_size = i32::from_le_bytes(phy[16..20].try_into().unwrap()) as usize;
    let collision_end = 20 + solid_size;
    let decoded = decode_physcollide(
        &phy[16..collision_end],
        &phy[collision_end..],
        1,
        DecodeLimits::default(),
    )
    .expect("PHYSCOLLIDE block should share the PHY solid decoder");

    assert_eq!(decoded.solids[0].convexes[0].faces[0].indices, [2, 1, 0]);
    assert_eq!(decoded.key_data.solids[0].mass, Some(12.5));
}

#[test]
fn preserves_explicit_unsupported_shape_categories() {
    for (model_type, expected) in [
        (1, UnsupportedShapeKind::Mopp),
        (2, UnsupportedShapeKind::Ball),
        (3, UnsupportedShapeKind::Virtual),
    ] {
        let decoded = decode_phy(
            &phy_with_solid(Some(model_type), IVPS, ""),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(
            decoded.solids[0].status,
            PhyShapeStatus::Unsupported(expected)
        );
        assert!(decoded.solids[0].convexes.is_empty());
    }
    let decoded = decode_phy(&phy_with_solid(None, MOPP, ""), DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.solids[0].status,
        PhyShapeStatus::Unsupported(UnsupportedShapeKind::Mopp)
    );

    let mut swapped = phy_with_solid(Some(0), IVPS, "");
    swapped[20..24].copy_from_slice(&YHPV.to_le_bytes());
    let decoded = decode_phy(&swapped, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.solids[0].status,
        PhyShapeStatus::Unsupported(UnsupportedShapeKind::SwappedEndian)
    );
}

#[test]
fn rejects_truncation_cycles_overflow_and_limit_exhaustion() {
    let mut truncated = phy_with_solid(Some(0), IVPS, "");
    truncated.truncate(truncated.len() - 20);
    assert!(
        decode_phy(&truncated, DecodeLimits::default())
            .unwrap_err()
            .to_string()
            .contains("truncated")
    );

    let mut cyclic = phy_with_solid(Some(0), IVPS, "");
    let surface = 16 + 4 + 28;
    write_i32(&mut cyclic, surface + 48, 28);
    write_i32(&mut cyclic, surface + 76, -28);
    assert!(
        decode_phy(&cyclic, DecodeLimits::default())
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );

    let mut overflow = phy_with_solid(Some(0), IVPS, "");
    let ledge = 16 + 4 + 28 + 76;
    write_i32(&mut overflow, ledge, i32::MAX);
    assert!(
        decode_phy(&overflow, DecodeLimits::default())
            .unwrap_err()
            .to_string()
            .contains("range")
    );

    let limits = DecodeLimits {
        max_triangles: 0,
        ..DecodeLimits::default()
    };
    assert!(
        decode_phy(&phy_with_solid(Some(0), IVPS, ""), limits)
            .unwrap_err()
            .to_string()
            .contains("triangle limit")
    );

    let framed = phy_with_solid(Some(0), IVPS, "");
    let size = i32::from_le_bytes(framed[16..20].try_into().unwrap()) as usize;
    let collision = &framed[16..20 + size];
    for end in 0..collision.len() {
        assert!(
            decode_physcollide(&collision[..end], b"", 1, DecodeLimits::default()).is_err(),
            "truncated solid prefix {end} unexpectedly decoded"
        );
    }
}

#[test]
fn output_is_deterministic() {
    let input = phy_with_solid(Some(0), IVPS, KEYDATA);
    let first = decode_phy(&input, DecodeLimits::default()).unwrap();
    let second = decode_phy(&input, DecodeLimits::default()).unwrap();
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn rejects_bytes_outside_declared_modern_and_ledge_ranges() {
    let mut trailing = phy_with_solid(Some(0), IVPS, "");
    let body_size = i32::from_le_bytes(trailing[16..20].try_into().unwrap()) as usize;
    trailing.insert(20 + body_size, 0xaa);
    write_i32(&mut trailing, 16, (body_size + 1) as i32);
    assert!(
        decode_phy(&trailing, DecodeLimits::default())
            .unwrap_err()
            .to_string()
            .contains("trailing")
    );

    let mut escaped_triangles = phy_with_solid(Some(0), IVPS, "");
    let ledge = 16 + 4 + 28 + 76;
    write_i16(&mut escaped_triangles, ledge + 12, 5);
    assert!(
        decode_phy(&escaped_triangles, DecodeLimits::default())
            .unwrap_err()
            .to_string()
            .contains("ledge")
    );
}
