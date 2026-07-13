use bsp_to_glb::{CollisionExportInput, export_collision_sidecar};
use serde_json::Value;
use std::fs;

#[test]
#[ignore = "requires HYDROGEN_BSP pointing to jump_hydrogen_rc1_bmv.bsp"]
fn hydrogen_collision_acceptance_counts_match_compiled_bsp() {
    let path = std::env::var_os("HYDROGEN_BSP")
        .expect("HYDROGEN_BSP must point to jump_hydrogen_rc1_bmv.bsp");
    let bsp = fs::read(path).unwrap();
    let result = export_collision_sidecar(&bsp, &CollisionExportInput::default()).unwrap();
    let sidecar: Value = serde_json::from_slice(&result.json).unwrap();

    assert_eq!(result.stats.brushes, 3_511);
    assert_eq!(result.stats.brush_sides, 31_092);
    assert_eq!(result.stats.world_model_brushes, 2_575);
    assert_eq!(result.stats.player_clip_brushes, 259);
    assert_eq!(result.stats.models, 151);
    assert_eq!(sidecar["models"][147]["numRenderFaces"], 0);
    assert!(
        !sidecar["models"][147]["brushIndices"]
            .as_array()
            .unwrap()
            .is_empty(),
        "model 147 must retain collision despite having no render faces"
    );
    assert_eq!(sidecar["geometrySource"], "bspBrushes");
    assert_eq!(sidecar["renderTriangleSubstitution"], false);
}
