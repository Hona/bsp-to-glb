use bsp_to_glb::{export_bsp, static_prop_collision_inputs};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn hydrogen_bsp() -> Option<PathBuf> {
    env::var_os("BSP_TO_GLB_HYDROGEN_BSP")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            [
                PathBuf::from("tests/fixtures/jump_hydrogen_rc1_bmv.bsp"),
                PathBuf::from("jump_hydrogen_rc1_bmv.bsp"),
            ]
            .into_iter()
            .find(|path| path.is_file())
        })
}

#[test]
fn hydrogen_static_prop_v10_benchmark_preserves_identity_and_solidity() {
    let Some(path) = hydrogen_bsp() else {
        eprintln!("Hydrogen BSP not present; set BSP_TO_GLB_HYDROGEN_BSP to run this gate");
        return;
    };
    let bsp = fs::read(&path).unwrap();
    let started = Instant::now();
    let result = export_bsp(&bsp, None).unwrap();
    let collision_props = static_prop_collision_inputs(&bsp).unwrap().unwrap();
    let elapsed = started.elapsed();

    assert_eq!(result.stats.static_props, 235);
    assert_eq!(result.stats.solid_static_props, 73);
    assert_eq!(result.props["staticPropLump"]["version"], 10);
    assert_eq!(result.props["staticPropLump"]["layout"], "tf2-v10");
    assert_eq!(result.props["staticProps"].as_array().unwrap().len(), 235);
    assert_eq!(collision_props.len(), 235);
    assert_eq!(
        collision_props
            .iter()
            .filter(|prop| prop.solid_mode != 0)
            .count(),
        73
    );
    assert!(
        result.props["modelAssets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|asset| asset["resolutionStatus"] == "unsupported")
    );
    eprintln!(
        "Hydrogen static-prop export benchmark: {} instances in {:.3?}",
        result.stats.static_props, elapsed
    );
}
