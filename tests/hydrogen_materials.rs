use bsp_to_glb::{
    ExportOptions, MaterialResolver, MountedMaterialResolver, ResourceProvenance,
    TextureDecodeStatus, VtfImageSelection, build_source_material_manifest,
    export_bsp_with_options, export_bsp_with_options_and_material_resolver, read_bsp_pak_resources,
};
use serde_json::json;
use std::collections::BTreeSet;
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
    let material_names = result
        .material_manifest
        .materials
        .iter()
        .map(|material| material.name.clone())
        .collect::<Vec<_>>();
    let parse_started = Instant::now();
    let parsed_manifest =
        build_source_material_manifest(&material_names, &pak_resources, None).unwrap();
    let parse_elapsed = parse_started.elapsed();
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
    let packaged_subresources = package
        .manifest
        .sources
        .iter()
        .filter(|source| source.status == TextureDecodeStatus::Decoded)
        .map(|source| source.strict_subresource_outputs.len())
        .sum::<usize>();
    assert_eq!(package.artifacts.len(), package.manifest.outputs.len());
    assert!(package.artifacts.len() <= packaged_subresources);
    assert!(package.manifest.sources.iter().all(|source| {
        if source.status != TextureDecodeStatus::Decoded {
            return source.strict_subresource_outputs.is_empty();
        }
        let metadata = source.metadata.as_ref().unwrap();
        let expected = (0..metadata.mip_count)
            .map(|mip| (metadata.depth >> mip).max(1) as usize)
            .sum::<usize>()
            * usize::from(metadata.frames)
            * usize::from(metadata.faces);
        source.strict_subresource_outputs.len() == expected
    }));
    assert_eq!(parsed_manifest.materials.len(), material_names.len());
    assert_eq!(resources.len(), pak_resources.len());
    assert_eq!(resources.len(), pak_vmts + pak_vtfs);
    eprintln!(
        "Hydrogen PAK scan: {:.3} ms; effective material parse: {:.3} ms; material package export: {:.3} ms, materials={}, pak_vmt={}, pak_vtf={}, references={}, decoded={}, unsupported={}, invalid={}, unique_png={}",
        pak_elapsed.as_secs_f64() * 1_000.0,
        parse_elapsed.as_secs_f64() * 1_000.0,
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

#[test]
#[ignore = "requires HYDROGEN_BSP and TF2_GAME_DIR"]
fn hydrogen_stock_material_resolution_and_benchmark() {
    let bsp_path = env::var_os("HYDROGEN_BSP").expect("set HYDROGEN_BSP");
    let game_dir = PathBuf::from(env::var_os("TF2_GAME_DIR").expect("set TF2_GAME_DIR"));
    let bsp = fs::read(bsp_path).unwrap();
    let plan = serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "mounts": [
            { "id": "tfLoose", "kind": "directory", "path": game_dir },
            { "id": "tfMisc", "kind": "vpk", "path": game_dir.join("tf2_misc_dir.vpk") },
            { "id": "tfTextures", "kind": "vpk", "path": game_dir.join("tf2_textures_dir.vpk") }
        ]
    }))
    .unwrap();

    let index_started = Instant::now();
    let resolver = MountedMaterialResolver::from_json(&plan).unwrap();
    let index_elapsed = index_started.elapsed();
    let six = [
        "materials/LIGHTS/WHITE001.vmt",
        "materials/Lights/White001.vtf",
        "materials/TEST/COLOR008.vmt",
        "materials/TOOLS/TOOLSBLACK.vmt",
        "materials/TOOLS/TOOLSNODRAW.vmt",
        "materials/TOOLS/TOOLSTRIGGER.vmt",
    ];
    let read_started = Instant::now();
    for path in six {
        assert!(resolver.resolve(path).unwrap().is_some(), "missing {path}");
    }
    let read_elapsed = read_started.elapsed();
    assert!(
        index_elapsed + read_elapsed < std::time::Duration::from_millis(250),
        "cold resolver overhead was {:.3} ms",
        (index_elapsed + read_elapsed).as_secs_f64() * 1_000.0
    );

    let result = export_bsp_with_options_and_material_resolver(
        &bsp,
        &ExportOptions::default(),
        Some(&resolver),
    )
    .unwrap();
    let mut stock_resources = BTreeSet::new();
    for material in &result.material_manifest.materials {
        if matches!(material.vmt.provenance, ResourceProvenance::External { .. }) {
            stock_resources.insert(material.vmt.lookup_path.clone());
        }
        for texture in &material.textures {
            if matches!(texture.provenance, ResourceProvenance::External { .. }) {
                stock_resources.insert(texture.lookup_path.clone().unwrap());
            }
        }
    }
    assert!(
        six.iter().all(|path| stock_resources.contains(*path)),
        "Hydrogen no longer references all benchmark resources: {stock_resources:?}"
    );
    let unresolved = result
        .material_manifest
        .unresolved_assets
        .iter()
        .map(|asset| asset.lookup_path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        unresolved,
        BTreeSet::from(["materials/TOOLS/TOOLSSKYBOX2D.vmt".to_owned()])
    );

    eprintln!(
        "Hydrogen stock resolver: index={:.3} ms, six reads={:.3} ms, resources={six:?}, unresolved={unresolved:?}",
        index_elapsed.as_secs_f64() * 1_000.0,
        read_elapsed.as_secs_f64() * 1_000.0,
    );
}
