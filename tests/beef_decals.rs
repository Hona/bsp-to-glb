use bsp_to_glb::{
    DecalOverlayStatus, ExportOptions, MountedMaterialResolver, VtfImageSelection,
    export_bsp_with_options_and_material_resolver,
};

#[test]
fn beef_decal_inventory_and_numbered_door_vectors_are_exact() {
    let Ok(bsp_path) = std::env::var("BSP_TO_GLB_BEEF_BSP") else {
        return;
    };
    let mount_plan = std::env::var("BSP_TO_GLB_BEEF_MOUNT_PLAN")
        .expect("BSP_TO_GLB_BEEF_MOUNT_PLAN must accompany BSP_TO_GLB_BEEF_BSP");
    let bsp = std::fs::read(bsp_path).expect("Beef BSP must be readable");
    let resolver = MountedMaterialResolver::from_json_file(mount_plan.as_ref())
        .expect("Beef material mount plan must be valid");
    let result = export_bsp_with_options_and_material_resolver(
        &bsp,
        &ExportOptions {
            material_texture_selection: Some(VtfImageSelection::default()),
            ..ExportOptions::default()
        },
        Some(&resolver),
    )
    .expect("Beef decal export must succeed");
    let sidecar = result.decal_overlays;

    assert_eq!(sidecar.inventory.infodecals, 39);
    assert_eq!(sidecar.inventory.compiled_overlays, 0);
    assert_eq!(sidecar.inventory.water_overlays, 0);
    assert_eq!(sidecar.coverage.handled, 38);
    assert_eq!(sidecar.coverage.unsupported, 1);
    assert_eq!(sidecar.coverage.malformed, 0);
    assert_eq!(sidecar.coverage.unknown, 0);
    let unsupported: Vec<_> = sidecar
        .records
        .iter()
        .filter(|record| record.status == DecalOverlayStatus::Unsupported)
        .map(|record| {
            (
                record.entity_index,
                record.material_name.as_deref(),
                record.reason.as_str(),
            )
        })
        .collect();
    assert_eq!(
        unsupported,
        [(
            Some(310),
            Some("decals/custom/interro_ad"),
            "material-unresolved"
        )]
    );

    let expected = [
        (
            220,
            "signs/number_01",
            [4616.0, 3176.26, -3043.97],
            vec![
                35, 36, 37, 40, 43, 44, 45, 56, 57, 58, 65, 68, 69, 77, 957, 958,
            ],
        ),
        (
            221,
            "signs/number_02",
            [4616.0, 3424.54, -3048.52],
            vec![35, 36, 38, 41, 46, 47, 48, 54, 55, 61, 66, 67, 74, 75],
        ),
        (
            222,
            "signs/number_03",
            [4616.0, 3673.06, -3049.86],
            vec![35, 39, 42, 49, 50, 51, 52, 53, 60, 62, 63, 64, 73, 76],
        ),
    ];
    for (entity_index, material, origin, faces) in expected {
        let record = sidecar
            .records
            .iter()
            .find(|record| record.entity_index == Some(entity_index))
            .expect("numbered door decal must be retained");
        assert_eq!(record.status, DecalOverlayStatus::Handled);
        assert_eq!(record.material_name.as_deref(), Some(material));
        assert_eq!(record.origin, Some(origin));
        assert_eq!(record.target.as_ref().unwrap().bsp_model_index, 0);
        assert_eq!(record.target.as_ref().unwrap().bsp_face_indices, faces);
        assert!(record.fragments.iter().all(|fragment| {
            let normal = fragment.normals[0];
            let distance = fragment.positions[0]
                .iter()
                .zip(normal)
                .map(|(component, axis)| component * axis)
                .sum::<f32>();
            fragment.positions.iter().all(|position| {
                let vertex_distance = position
                    .iter()
                    .zip(normal)
                    .map(|(component, axis)| component * axis)
                    .sum::<f32>();
                (vertex_distance - distance).abs() <= f32::EPSILON
            })
        }));
    }
}
