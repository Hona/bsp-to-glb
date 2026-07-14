use bsp_to_glb::phy::{DecodeLimits, decode_phy};
use bsp_to_glb::static_physics::{
    STATIC_PHYSICS_BINARY_VERSION, StaticPhysicsCounts, StaticPhysicsLimits,
    encode_phy_shape_bundle,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Output<T> {
    schema: &'static str,
    schema_version: u32,
    decoded: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    binary: Option<BinaryOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    solid_shape_indices: Option<Vec<Option<u32>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    counts: Option<StaticPhysicsCounts>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BinaryOutput {
    format: &'static str,
    version: u32,
    byte_length: usize,
    sha256: String,
}

fn usage() -> &'static str {
    "Usage: decode-phy --input <model.phy> --out <decoded.json> [--binary-out <shapes.bin>]"
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

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut binary_output: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].to_string_lossy();
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}\n{}", usage()))?;
        match flag.as_ref() {
            "--input" => input = Some(value.into()),
            "--out" => output = Some(value.into()),
            "--binary-out" => binary_output = Some(value.into()),
            _ => return Err(format!("unknown argument: {flag}\n{}", usage())),
        }
        index += 2;
    }
    let input = input.ok_or_else(|| usage().to_owned())?;
    let output = output.ok_or_else(|| usage().to_owned())?;
    let bytes =
        fs::read(&input).map_err(|error| format!("failed to read {}: {error}", input.display()))?;
    let decoded = decode_phy(&bytes, DecodeLimits::default()).map_err(|error| error.to_string())?;
    let encoded = binary_output
        .as_ref()
        .map(|_| encode_phy_shape_bundle(-1, &decoded.solids, StaticPhysicsLimits::default()))
        .transpose()
        .map_err(|error| error.to_string())?;
    let binary = encoded.as_ref().map(|bundle| BinaryOutput {
        format: "bsp-to-glb/static-physics-binary",
        version: STATIC_PHYSICS_BINARY_VERSION,
        byte_length: bundle.binary.len(),
        sha256: format!("{:x}", Sha256::digest(&bundle.binary)),
    });
    let json = serde_json::to_vec(&Output {
        schema: "bsp-to-glb/phy",
        schema_version: 1,
        decoded,
        binary,
        solid_shape_indices: encoded
            .as_ref()
            .map(|bundle| bundle.solid_shape_indices.clone()),
        counts: encoded.as_ref().map(|bundle| bundle.counts.clone()),
    })
    .map_err(|error| format!("failed to serialize decoded PHY: {error}"))?;
    create_parent(&output)?;
    fs::write(&output, json)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    if let (Some(path), Some(bundle)) = (binary_output, encoded) {
        create_parent(&path)?;
        fs::write(&path, bundle.binary)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
