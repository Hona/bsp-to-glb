use bsp_to_glb::{CollisionExportInput, export_bsp, export_collision_sidecar};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "Usage: bsp-to-glb --bsp <compiled.bsp> [--out <map.glb>] [--collision-out <map.collision.json>] [--lightmaps <lightmap_data.json>]"
}

fn run() -> Result<(), String> {
    let mut bsp_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut collision_output_path: Option<PathBuf> = None;
    let mut lightmap_path: Option<PathBuf> = None;
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
            _ => return Err(format!("unknown argument: {flag}\n{}", usage())),
        }
        index += 2;
    }
    let bsp_path = bsp_path.ok_or_else(|| usage().to_owned())?;
    if output_path.is_none() && collision_output_path.is_none() {
        return Err(usage().to_owned());
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
        let result = export_bsp(&bsp, lightmaps.as_deref())?;
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(&output_path, &result.glb)
            .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
        render_stats = Some(result.stats);
    }
    if let Some(output_path) = collision_output_path {
        let result = export_collision_sidecar(&bsp, &CollisionExportInput::default())?;
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(&output_path, &result.json)
            .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
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
