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
    assert_eq!(sidecar.inventory.fragments, 62);
    assert_eq!(sidecar.coverage.handled, 34);
    assert_eq!(sidecar.coverage.inert, 4);
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

    let inert: Vec<_> = sidecar
        .records
        .iter()
        .filter(|record| record.status == DecalOverlayStatus::Inert)
        .map(|record| {
            let target = record.target.as_ref().unwrap();
            (
                record.entity_index.unwrap(),
                target.bsp_model_index,
                target.bsp_face_indices.clone(),
                record.reason.as_str(),
            )
        })
        .collect();
    assert_eq!(
        inert,
        [
            (339, 22, vec![2981], "target-surface-does-not-accept-decals"),
            (340, 22, vec![2981], "target-surface-does-not-accept-decals"),
            (341, 25, vec![2999], "target-surface-does-not-accept-decals"),
            (342, 25, vec![2999], "target-surface-does-not-accept-decals"),
        ]
    );

    let expected_targets = [
        (220, 93, vec![3439]),
        (221, 94, vec![3445]),
        (222, 95, vec![3451]),
        (233, 0, vec![2794]),
        (234, 0, vec![2834, 2837]),
        (235, 0, vec![2418]),
        (236, 0, vec![2418]),
        (328, 0, vec![2106]),
        (329, 0, vec![1845, 1863]),
        (330, 0, vec![1908, 1909, 2380, 2381]),
        (331, 0, vec![1986]),
        (332, 0, vec![2077, 2081, 2637, 2677]),
        (333, 0, vec![2624, 2633]),
        (334, 0, vec![2746]),
        (335, 0, vec![2735, 2738]),
        (336, 0, vec![2153, 2154]),
        (337, 0, vec![2432, 2433]),
        (338, 0, vec![2432, 2433]),
        (343, 0, vec![285]),
        (344, 0, vec![282, 285]),
        (345, 0, vec![233]),
        (346, 0, vec![229, 233]),
        (347, 0, vec![607]),
        (348, 0, vec![607]),
        (349, 0, vec![523, 524]),
        (350, 0, vec![523, 524]),
        (351, 0, vec![1296, 1299]),
        (352, 0, vec![1296, 1299]),
        (354, 0, vec![1400]),
        (355, 0, vec![1400, 1408]),
        (356, 0, vec![1412, 1413, 1416, 1417]),
        (357, 0, vec![1416, 1417]),
        (358, 0, vec![2446, 2447, 2462, 2463]),
        (359, 0, vec![2462, 2463]),
    ];
    assert_eq!(expected_targets.len(), sidecar.coverage.handled);
    for (entity_index, model_index, faces) in expected_targets {
        let record = sidecar
            .records
            .iter()
            .find(|record| record.entity_index == Some(entity_index))
            .unwrap();
        assert_eq!(record.status, DecalOverlayStatus::Handled);
        assert_eq!(record.target.as_ref().unwrap().bsp_model_index, model_index);
        assert_eq!(record.target.as_ref().unwrap().bsp_face_indices, faces);
    }

    let expected = [
        (
            220,
            "signs/number_01",
            [4616.0, 3176.26, -3043.97],
            93,
            216,
            3439,
        ),
        (
            221,
            "signs/number_02",
            [4616.0, 3424.54, -3048.52],
            94,
            217,
            3445,
        ),
        (
            222,
            "signs/number_03",
            [4616.0, 3673.06, -3049.86],
            95,
            218,
            3451,
        ),
    ];
    for (entity_index, material, origin, model, parent, face) in expected {
        let record = sidecar
            .records
            .iter()
            .find(|record| record.entity_index == Some(entity_index))
            .expect("numbered door decal must be retained");
        assert_eq!(record.status, DecalOverlayStatus::Handled);
        assert_eq!(record.material_name.as_deref(), Some(material));
        assert_eq!(record.origin, Some(origin));
        assert_eq!(record.target.as_ref().unwrap().bsp_model_index, model);
        assert_eq!(record.target.as_ref().unwrap().bsp_face_indices, [face]);
        assert_eq!(record.parent_entity_index, Some(parent));
        assert_eq!(record.fragments.len(), 1);
        let fragment = &record.fragments[0];
        assert_eq!(
            fragment.positions,
            [
                [8.0, -48.0, -96.0],
                [8.0, -48.0, 96.0],
                [8.0, 48.0, 96.0],
                [8.0, 48.0, -96.0],
            ]
        );
        let u_span = fragment
            .uvs
            .iter()
            .map(|uv| uv[0])
            .fold(f32::NEG_INFINITY, f32::max)
            - fragment
                .uvs
                .iter()
                .map(|uv| uv[0])
                .fold(f32::INFINITY, f32::min);
        let v_span = fragment
            .uvs
            .iter()
            .map(|uv| uv[1])
            .fold(f32::NEG_INFINITY, f32::max)
            - fragment
                .uvs
                .iter()
                .map(|uv| uv[1])
                .fold(f32::INFINITY, f32::min);
        assert!((96.0 / u_span - 128.0).abs() < 1e-4);
        assert!((192.0 / v_span - 256.0).abs() < 1e-4);
    }

    for record in &sidecar.records {
        for fragment in &record.fragments {
            for triangle in fragment.indices.chunks_exact(3) {
                let a = fragment.positions[triangle[0] as usize];
                let b = fragment.positions[triangle[1] as usize];
                let c = fragment.positions[triangle[2] as usize];
                let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let cross = [
                    ab[1] * ac[2] - ab[2] * ac[1],
                    ab[2] * ac[0] - ab[0] * ac[2],
                    ab[0] * ac[1] - ab[1] * ac[0],
                ];
                let normal = fragment.normals[triangle[0] as usize];
                assert!(cross[0] * normal[0] + cross[1] * normal[1] + cross[2] * normal[2] > 0.0);
            }
        }
    }
}
