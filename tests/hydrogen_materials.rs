use bsp_to_glb::{
    ExportOptions, TextureDecodeStatus, VtfImageSelection, export_bsp_with_options,
    read_bsp_pak_resources,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[test]
#[ignore = "requires jump_hydrogen_rc1_bmv.bsp (set HYDROGEN_BSP or place it in the repository root)"]
fn hydrogen_pak_material_coverage_and_benchmark() {
    let path = env::var_os("HYDROGEN_BSP")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("jump_hydrogen_rc1_bmv.bsp"));
    let bsp = fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let options = ExportOptions {
        material_texture_selection: Some(VtfImageSelection::default()),
        ..ExportOptions::default()
    };

    let pak_started = Instant::now();
    let pak_resources = read_bsp_pak_resources(&bsp).unwrap();
    let pak_elapsed = pak_started.elapsed();
    let started = Instant::now();
    let result = export_bsp_with_options(&bsp, &options).unwrap();
    let elapsed = started.elapsed();
    let package = result
        .material_textures
        .as_ref()
        .expect("texture packaging was requested");
    let resources = &result.material_manifest.embedded_resources;
    let pak_vmts = resources
        .iter()
        .filter(|resource| resource.kind == bsp_to_glb::PakResourceKind::Vmt)
        .count();
    let pak_vtfs = resources
        .iter()
        .filter(|resource| resource.kind == bsp_to_glb::PakResourceKind::Vtf)
        .count();
    let decoded = package
        .manifest
        .sources
        .iter()
        .filter(|source| source.status == TextureDecodeStatus::Decoded)
        .count();
    let unsupported = package
        .manifest
        .sources
        .iter()
        .filter(|source| source.status == TextureDecodeStatus::Unsupported)
        .count();
    let invalid = package
        .manifest
        .sources
        .iter()
        .filter(|source| source.status == TextureDecodeStatus::Invalid)
        .count();

    assert_eq!(
        decoded + unsupported + invalid,
        package.manifest.sources.len()
    );
    assert!(package.artifacts.len() <= decoded);
    assert_eq!(resources.len(), pak_resources.len());
    assert_eq!(resources.len(), pak_vmts + pak_vtfs);
    eprintln!(
        "Hydrogen PAK scan: {:.3} ms; material package export: {:.3} ms, materials={}, pak_vmt={}, pak_vtf={}, references={}, decoded={}, unsupported={}, invalid={}, unique_png={}",
        pak_elapsed.as_secs_f64() * 1_000.0,
        elapsed.as_secs_f64() * 1_000.0,
        result.material_manifest.materials.len(),
        pak_vmts,
        pak_vtfs,
        package.manifest.sources.len(),
        decoded,
        unsupported,
        invalid,
        package.artifacts.len(),
    );
}
