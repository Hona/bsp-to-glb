use serde_json::Value;
use std::process::Command;

fn run(argument: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .arg(argument)
        .output()
        .expect("bsp-to-glb should run")
}

#[test]
fn version_output_matches_the_cargo_package() {
    let output = run("--version");

    assert!(
        output.status.success(),
        "version command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("bsp-to-glb {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.2.0");
    assert!(output.stderr.is_empty());
}

#[test]
fn version_json_is_stable_machine_readable_build_metadata() {
    let output = run("--version-json");

    assert!(
        output.status.success(),
        "version JSON command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(metadata["schema"], "bsp-to-glb.build-metadata");
    assert_eq!(metadata["schemaVersion"], 2);
    assert_eq!(metadata["name"], "bsp-to-glb");
    assert_eq!(metadata["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        metadata["target"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        metadata["profile"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(metadata.get("sourceCommit").is_some());
    assert_eq!(metadata["capabilities"]["brushGeometry"], "supported");
    assert_eq!(metadata["capabilities"]["displacements"], "supported");
    assert_eq!(metadata["capabilities"]["directLightmaps"], "supported");
    assert_eq!(metadata["capabilities"]["materialResolution"], "supported");
    assert_eq!(metadata["capabilities"]["bspPakArchive"], "supported");
    assert_eq!(metadata["capabilities"]["vtfPixelConversion"], "supported");
    assert_eq!(metadata["capabilities"]["visibility"], "supported");
    assert_eq!(metadata["capabilities"]["entityGraph"], "supported");
    assert_eq!(
        metadata["capabilities"]["decodedPhysicsCollision"],
        "supported"
    );
    assert_eq!(metadata["components"]["materialManifest"], 3);
    assert_eq!(metadata["components"]["materialMountPlan"], 1);
    assert_eq!(metadata["components"]["materialTextures"], 2);
    assert_eq!(metadata["components"]["bspPak"], 1);
    assert_eq!(metadata["components"]["visibilitySidecar"], 2);
    assert_eq!(metadata["components"]["entityGraph"], 1);
    assert_eq!(metadata["components"]["staticPhysics"], 1);
    assert_eq!(metadata["capabilities"]["overlays"], "detectedOnly");
    assert_eq!(metadata["capabilities"]["propGeometry"], "unsupported");
    assert!(output.stderr.is_empty());
}
