use bsp_to_glb::{
    CollisionExportInput, ExportOptions, LightmapSet, encode_lightmap_png, export_bsp,
    export_bsp_with_options, export_bsp_with_options_and_visibility, export_bsp_with_visibility,
    export_collision_sidecar,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage() -> &'static str {
    "Usage: bsp-to-glb --bsp <compiled.bsp> [--out <map.glb>] [--collision-out <map.collision.json>] [--visibility-out <map.visibility.json>] [--lightmaps <lightmap_data.json> | --lightmap-set <auto|ldr|hdr|none>] [--atlas-width <pixels>] [--lightmap-atlas <flat.png>] [--lightmap-manifest <lightmaps.json>] [--material-manifest <materials.json>] [--props-out <props.json>]"
}

fn create_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn write(path: &Path, data: &[u8]) -> Result<(), String> {
    create_parent(path)?;
    fs::write(path, data).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn directional_atlas_path(flat: &Path, channel: usize) -> Result<PathBuf, String> {
    let stem = flat
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "lightmap atlas path must have a UTF-8 file name".to_owned())?;
    let extension = flat
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");
    Ok(flat.with_file_name(format!("{stem}.bump-{channel}.{extension}")))
}

fn manifest_uri(path: &Path, manifest_path: Option<&Path>) -> String {
    let base = manifest_path
        .and_then(Path::parent)
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    pathdiff::diff_paths(path, base)
        .unwrap_or_else(|| path.to_owned())
        .to_string_lossy()
        .replace('\\', "/")
}

fn run() -> Result<(), String> {
    let mut bsp_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut collision_output_path: Option<PathBuf> = None;
    let mut lightmap_path: Option<PathBuf> = None;
    let mut material_manifest_path: Option<PathBuf> = None;
    let mut props_output_path: Option<PathBuf> = None;
    let mut atlas_path: Option<PathBuf> = None;
    let mut manifest_path: Option<PathBuf> = None;
    let mut options = ExportOptions::default();
    let mut visibility_path: Option<PathBuf> = None;
    let args: Vec<_> = env::args_os().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].to_string_lossy();
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}\n{}", usage()))?;
        match flag.as_ref() {
            "--bsp" => bsp_path = Some(value.into()),
            "--out" => output_path = Some(value.into()),
            "--collision-out" => collision_output_path = Some(value.into()),
            "--lightmaps" => lightmap_path = Some(value.into()),
            "--material-manifest" => material_manifest_path = Some(value.into()),
            "--props-out" => props_output_path = Some(value.into()),
            "--lightmap-atlas" => atlas_path = Some(value.into()),
            "--lightmap-manifest" => manifest_path = Some(value.into()),
            "--lightmap-set" => {
                options.lightmap_set = match value.to_string_lossy().as_ref() {
                    "auto" => LightmapSet::Auto,
                    "ldr" => LightmapSet::Ldr,
                    "hdr" => LightmapSet::Hdr,
                    "none" => LightmapSet::None,
                    selection => {
                        return Err(format!("unknown lightmap set: {selection}\n{}", usage()));
                    }
                };
            }
            "--atlas-width" => {
                options.atlas_width = value
                    .to_string_lossy()
                    .parse()
                    .map_err(|_| format!("invalid atlas width: {}", value.to_string_lossy()))?;
            }
            "--visibility-out" => visibility_path = Some(value.into()),
            _ => return Err(format!("unknown argument: {flag}\n{}", usage())),
        }
        index += 2;
    }
    let bsp_path = bsp_path.ok_or_else(|| usage().to_owned())?;
    if output_path.is_none() && collision_output_path.is_none() {
        return Err(usage().to_owned());
    }
    if output_path.is_none() && visibility_path.is_some() {
        return Err("--visibility-out requires --out because it references GLB chunks".to_owned());
    }
    let bsp = fs::read(&bsp_path)
        .map_err(|error| format!("failed to read {}: {error}", bsp_path.display()))?;
    let lightmaps = lightmap_path
        .as_ref()
        .map(|path| {
            fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
        })
        .transpose()?;
    let mut render_stats = None;
    let mut collision_stats = None;
    if let Some(output_path) = output_path {
        let mut result = if lightmaps.is_some() {
            if atlas_path.is_some() || manifest_path.is_some() {
                return Err(
                    "--lightmaps cannot be combined with direct lightmap atlas outputs".to_owned(),
                );
            }
            if visibility_path.is_some() {
                export_bsp_with_visibility(&bsp, lightmaps.as_deref())?
            } else {
                export_bsp(&bsp, lightmaps.as_deref())?
            }
        } else if visibility_path.is_some() {
            export_bsp_with_options_and_visibility(&bsp, &options)?
        } else {
            export_bsp_with_options(&bsp, &options)?
        };
        let material_manifest = material_manifest_path
            .as_ref()
            .map(|_| {
                serde_json::to_vec_pretty(&result.material_manifest)
                    .map_err(|error| format!("failed to serialize material manifest: {error}"))
            })
            .transpose()?;
        let props = props_output_path
            .as_ref()
            .map(|_| {
                serde_json::to_vec_pretty(&result.props)
                    .map_err(|error| format!("failed to serialize prop metadata: {error}"))
            })
            .transpose()?;
        write(&output_path, &result.glb)?;
        if let (Some(path), Some(manifest)) = (&material_manifest_path, material_manifest) {
            write(path, &manifest)?;
        }
        if let (Some(path), Some(props)) = (&props_output_path, props) {
            write(path, &props)?;
        }
        if atlas_path.is_some() || manifest_path.is_some() {
            let artifacts = result.lightmaps.as_mut().ok_or_else(|| {
                "selected BSP lightmap pair contains no supported lit faces".to_owned()
            })?;
            if let Some(flat_path) = &atlas_path {
                let directional_paths = [
                    directional_atlas_path(flat_path, 0)?,
                    directional_atlas_path(flat_path, 1)?,
                    directional_atlas_path(flat_path, 2)?,
                ];
                write(flat_path, &encode_lightmap_png(&artifacts.flat)?)?;
                for (image, path) in artifacts.directional.iter().zip(&directional_paths) {
                    write(path, &encode_lightmap_png(image)?)?;
                }
                artifacts.manifest.set_channel_uris([
                    manifest_uri(flat_path, manifest_path.as_deref()),
                    manifest_uri(&directional_paths[0], manifest_path.as_deref()),
                    manifest_uri(&directional_paths[1], manifest_path.as_deref()),
                    manifest_uri(&directional_paths[2], manifest_path.as_deref()),
                ]);
            }
            if let Some(path) = &manifest_path {
                let manifest = serde_json::to_vec_pretty(&artifacts.manifest)
                    .map_err(|error| format!("failed to serialize lightmap manifest: {error}"))?;
                write(path, &manifest)?;
            }
        }
        if let Some(path) = &visibility_path {
            let sidecar = result
                .visibility
                .as_ref()
                .ok_or_else(|| "visibility sidecar was not generated".to_owned())?;
            write(path, &sidecar.to_json()?)?;
        }
        render_stats = Some(result.stats);
    }
    if let Some(output_path) = collision_output_path {
        let result = export_collision_sidecar(&bsp, &CollisionExportInput::default())?;
        write(&output_path, &result.json)?;
        collision_stats = Some(result.stats);
    }
    let stats = match (&render_stats, &collision_stats) {
        (Some(render), None) => serde_json::to_value(render),
        (None, Some(collision)) => serde_json::to_value(collision),
        _ => Ok(serde_json::json!({
            "render": render_stats,
            "collision": collision_stats
        })),
    }
    .map_err(|error| format!("failed to serialize stats: {error}"))?;
    println!(
        "{}",
        serde_json::to_string(&stats)
            .map_err(|error| format!("failed to serialize stats: {error}"))?
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
