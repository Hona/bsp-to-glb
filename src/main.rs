use bsp_to_glb::{export_bsp, export_bsp_with_visibility};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "Usage: bsp-to-glb --bsp <compiled.bsp> --out <map.glb> [--lightmaps <lightmap_data.json>] [--visibility-out <map.visibility.json>]"
}

fn run() -> Result<(), String> {
    let mut bsp_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut lightmap_path: Option<PathBuf> = None;
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
            "--lightmaps" => lightmap_path = Some(value.into()),
            "--visibility-out" => visibility_path = Some(value.into()),
            _ => return Err(format!("unknown argument: {flag}\n{}", usage())),
        }
        index += 2;
    }
    let bsp_path = bsp_path.ok_or_else(|| usage().to_owned())?;
    let output_path = output_path.ok_or_else(|| usage().to_owned())?;
    let bsp = fs::read(&bsp_path)
        .map_err(|error| format!("failed to read {}: {error}", bsp_path.display()))?;
    let lightmaps = lightmap_path
        .as_ref()
        .map(|path| {
            fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
        })
        .transpose()?;
    let result = if visibility_path.is_some() {
        export_bsp_with_visibility(&bsp, lightmaps.as_deref())?
    } else {
        export_bsp(&bsp, lightmaps.as_deref())?
    };
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&output_path, &result.glb)
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    if let Some(path) = visibility_path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let sidecar = result
            .visibility
            .as_ref()
            .ok_or_else(|| "visibility sidecar was not generated".to_owned())?;
        fs::write(&path, sidecar.to_json()?)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    println!(
        "{}",
        serde_json::to_string(&result.stats)
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
