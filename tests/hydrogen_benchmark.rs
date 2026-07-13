use bsp_to_glb::{ExportOptions, export_bsp_with_options};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[test]
#[ignore = "requires jump_hydrogen_rc1_bmv.bsp (set HYDROGEN_BSP or place it in the repository root)"]
fn hydrogen_lightmap_contract_and_benchmark() {
    let path = env::var_os("HYDROGEN_BSP")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("jump_hydrogen_rc1_bmv.bsp"));
    let bsp = fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    let started = Instant::now();
    let result = export_bsp_with_options(&bsp, &ExportOptions::default()).unwrap();
    let elapsed = started.elapsed();
    let artifacts = result.lightmaps.as_ref().expect("Hydrogen has lightmaps");
    let manifest = serde_json::to_value(&artifacts.manifest).unwrap();
    let faces = manifest["faces"].as_array().unwrap();

    assert_eq!(result.stats.lightmapped_faces, 9_135);
    assert_eq!(result.stats.bumped_lightmapped_faces, 4_529);
    assert_eq!(faces.len(), 9_135);
    assert_eq!(
        faces
            .iter()
            .filter(|face| face["bumpLight"] == true)
            .count(),
        4_529
    );
    assert!(faces.iter().all(|face| face["faceIndex"].is_u64()));
    assert!(
        faces
            .iter()
            .all(|face| face["styles"].as_array().unwrap().len() == 1)
    );
    assert_eq!(
        artifacts.flat.pixels.len(),
        artifacts.flat.width as usize * artifacts.flat.height as usize * 4
    );
    assert!(
        artifacts
            .directional
            .iter()
            .all(|atlas| atlas.pixels.len() == artifacts.flat.pixels.len())
    );

    eprintln!(
        "Hydrogen direct geometry + lightmaps: {:.3} ms, atlas={}x{}",
        elapsed.as_secs_f64() * 1_000.0,
        artifacts.flat.width,
        artifacts.flat.height
    );
}
