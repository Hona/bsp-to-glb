use bsp_to_glb::static_physics::{
    StaticPhysicsLimits, decode_shape_bundle, export_bsp_static_physics,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn hydrogen_bsp() -> Option<PathBuf> {
    env::var_os("BSP_TO_GLB_HYDROGEN_BSP")
        .or_else(|| env::var_os("HYDROGEN_BSP"))
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

#[test]
fn hydrogen_static_physics_counts_and_binary_are_deterministic() {
    let Some(path) = hydrogen_bsp() else {
        eprintln!("Hydrogen BSP not present; set BSP_TO_GLB_HYDROGEN_BSP to run this gate");
        return;
    };
    let bsp = fs::read(path).unwrap();
    let started = Instant::now();
    let first = export_bsp_static_physics(&bsp, StaticPhysicsLimits::default()).unwrap();
    let elapsed = started.elapsed();

    assert_eq!(first.manifest.stats.models, 151);
    assert_eq!(first.manifest.stats.solids, 152);
    assert_eq!(first.manifest.stats.decoded_solids, 152);
    assert_eq!(first.manifest.stats.unsupported_solids, 0);
    assert_eq!(first.manifest.stats.shapes, 152);
    assert_eq!(first.manifest.stats.convexes, 3_511);
    assert_eq!(first.manifest.stats.vertices, 29_329);
    assert_eq!(first.manifest.stats.faces, 44_614);
    assert_eq!(first.binary.len(), 1_147_000);
    assert_eq!(
        first.manifest.binary.sha256,
        "a975698e8f0ab971b77308ab6aaa2326f7243e1c6f703338adfe189e228a5d98"
    );
    assert!(
        elapsed.as_secs_f32() < 5.0,
        "Hydrogen decode took {elapsed:?}"
    );

    let decoded = decode_shape_bundle(&first.binary, StaticPhysicsLimits::default()).unwrap();
    assert_eq!(decoded.counts.shapes, 152);
    assert_eq!(decoded.counts.convexes, 3_511);
    let repeated = export_bsp_static_physics(&bsp, StaticPhysicsLimits::default()).unwrap();
    assert_eq!(repeated.binary, first.binary);
    assert_eq!(repeated.manifest, first.manifest);
    eprintln!("Hydrogen static physics exported in {elapsed:?}");
}
