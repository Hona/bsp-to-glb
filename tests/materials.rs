use bsp_to_glb::{
    MaterialResolver, PakResourceKind, ResolvedMaterialResource, build_source_material_manifest,
    parse_vmt, read_bsp_pak_resources,
};
use serde_json::to_value;
use std::cell::RefCell;
use std::io::{Cursor, Write};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

const HEADER_SIZE: usize = 4 + 4 + 64 * 16 + 4;

fn put_i32(data: &mut [u8], offset: usize, value: i32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn bsp_with_pak(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (path, data) in entries {
        writer.start_file(path, options).unwrap();
        writer.write_all(data).unwrap();
    }
    let pak = writer.finish().unwrap().into_inner();

    let mut bsp = vec![0; HEADER_SIZE];
    bsp[0..4].copy_from_slice(b"VBSP");
    put_i32(&mut bsp, 4, 20);
    let offset = bsp.len();
    bsp.extend_from_slice(&pak);
    let header = 8 + 40 * 16;
    put_i32(&mut bsp, header, offset as i32);
    put_i32(&mut bsp, header + 4, pak.len() as i32);
    bsp
}

#[test]
fn parses_supported_vmt_shader_inputs_and_limitations() {
    let material = parse_vmt(
        br#"
        LightmappedGeneric
        {
            // Values and keys are case-insensitive in Source material files.
            "$BaseTexture" "Brick\\Wall_A"
            "$translucent" 1
            "$additive" "1"
            "$alphatest" 1
            "$alphatestreference" ".42"
            "$nocull" 1
            "$bumpmap" "brick/wall_a_normal"
            "$ssbump" 1
            "$detail" "detail/noise_detail"
            "$selfillum" 1
            "$envmap" env_cubemap
            "$surfaceprop" "Concrete"
            Proxies
            {
                AnimatedTexture
                {
                    animatedtexturevar "$basetexture"
                    animatedtextureframenumvar "$frame"
                }
            }
        }
        "#,
    )
    .unwrap();

    assert_eq!(material.shader.name, "LightmappedGeneric");
    assert_eq!(material.shader.family, "lightmappedGeneric");
    assert_eq!(
        material.textures.base_texture.as_deref(),
        Some("Brick/Wall_A")
    );
    assert_eq!(
        material.textures.bump_map.as_deref(),
        Some("brick/wall_a_normal")
    );
    assert_eq!(
        material.textures.detail.as_deref(),
        Some("detail/noise_detail")
    );
    assert_eq!(material.textures.env_map.as_deref(), Some("env_cubemap"));
    assert!(material.features.translucent);
    assert!(material.features.additive);
    assert!(material.features.alpha_test);
    assert_eq!(material.features.alpha_test_reference, Some(0.42));
    assert!(material.features.no_cull);
    assert!(material.features.bump);
    assert!(material.features.ss_bump);
    assert!(material.features.detail);
    assert!(material.features.self_illum);
    assert!(!material.features.unlit);
    assert_eq!(material.surface_prop.as_deref(), Some("Concrete"));
    assert_eq!(material.unsupported.proxies, ["AnimatedTexture"]);
    assert!(material.unsupported.animated);
    assert_eq!(material.shader.inputs["$basetexture"], "Brick\\Wall_A");
}

#[test]
fn identifies_unlit_materials_and_rejects_malformed_keyvalues() {
    let material = parse_vmt(br#"UnlitGeneric { "$basetexture" "ui/panel" }"#).unwrap();
    assert_eq!(material.shader.family, "unlitGeneric");
    assert!(material.features.unlit);

    let error = parse_vmt(br#"LightmappedGeneric { "$basetexture" }"#).unwrap_err();
    assert!(error.contains("value"), "unexpected error: {error}");
}

#[test]
fn handles_utf8_bom_and_non_ascii_values() {
    let material =
        parse_vmt(b"\xef\xbb\xbfUnlitGeneric { \"$basetexture\" \"custom/m\xc3\xa4t\" }").unwrap();
    assert_eq!(material.shader.family, "unlitGeneric");
    assert_eq!(
        material.textures.base_texture.as_deref(),
        Some("custom/m\u{e4}t")
    );
}

#[test]
fn exposes_only_embedded_material_resources_with_original_paths() {
    let bsp = bsp_with_pak(&[
        (
            "materials/Brick/Test.vmt",
            br#"LightmappedGeneric { "$basetexture" "Brick/Test_D" }"#,
        ),
        ("materials/Brick/Test_D.vtf", b"synthetic-vtf"),
        ("maps/readme.txt", b"not a material resource"),
    ]);

    let resources = read_bsp_pak_resources(&bsp).unwrap();
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].path, "materials/Brick/Test.vmt");
    assert_eq!(resources[0].kind, PakResourceKind::Vmt);
    assert!(resources[0].data.starts_with(b"LightmappedGeneric"));
    assert_eq!(resources[1].path, "materials/Brick/Test_D.vtf");
    assert_eq!(resources[1].kind, PakResourceKind::Vtf);
    assert_eq!(resources[1].data, b"synthetic-vtf");
}

#[test]
fn rejects_parent_traversal_in_pak_paths() {
    let bsp = bsp_with_pak(&[("materials/safe/../../escape.vmt", b"LightmappedGeneric {}")]);
    let error = read_bsp_pak_resources(&bsp).unwrap_err();
    assert!(
        error.contains("unsafe PAK path"),
        "unexpected error: {error}"
    );
}

struct FixtureResolver {
    requests: RefCell<Vec<String>>,
}

impl MaterialResolver for FixtureResolver {
    fn resolve(&self, lookup_path: &str) -> Result<Option<ResolvedMaterialResource>, String> {
        self.requests.borrow_mut().push(lookup_path.to_owned());
        if lookup_path.eq_ignore_ascii_case("materials/brick/test_normal.vtf") {
            Ok(Some(ResolvedMaterialResource {
                data: b"external-synthetic-vtf".to_vec(),
                provenance: "fixture-resolver".to_owned(),
            }))
        } else {
            Ok(None)
        }
    }
}

#[test]
fn manifest_uses_pak_first_then_reports_external_and_unresolved_assets() {
    let bsp = bsp_with_pak(&[
        (
            "materials/brick/test.vmt",
            br#"LightmappedGeneric {
                "$basetexture" "brick/test_diffuse"
                "$bumpmap" "brick/test_normal"
                "$detail" "detail/missing"
            }"#,
        ),
        ("materials/brick/test_diffuse.vtf", b"synthetic-vtf"),
    ]);
    let resources = read_bsp_pak_resources(&bsp).unwrap();
    let resolver = FixtureResolver {
        requests: RefCell::new(Vec::new()),
    };
    let manifest =
        build_source_material_manifest(&["brick/test".to_owned()], &resources, Some(&resolver))
            .unwrap();
    let json = to_value(&manifest).unwrap();

    assert_eq!(json["schemaVersion"], 2);
    assert_eq!(json["lookupPolicy"], "pakFirst");
    assert_eq!(
        json["materials"][0]["vmt"]["lookupPath"],
        "materials/brick/test.vmt"
    );
    assert_eq!(json["materials"][0]["vmt"]["provenance"]["kind"], "pak");
    assert_eq!(
        json["materials"][0]["textures"][0]["provenance"]["kind"],
        "pak"
    );
    assert_eq!(
        json["materials"][0]["textures"][1]["provenance"]["kind"],
        "external"
    );
    assert_eq!(
        json["unresolvedAssets"][0]["lookupPath"],
        "materials/detail/missing.vtf"
    );
    assert_eq!(
        json["limitations"]["vtfPixelConversion"],
        "optionalSelectedRgbaPngPackage"
    );
    assert_eq!(json["limitations"]["proxies"], "metadataOnly");
    assert_eq!(json["limitations"]["animatedMaterials"], "metadataOnly");

    assert_eq!(
        resolver.requests.into_inner(),
        [
            "materials/brick/test_normal.vtf",
            "materials/detail/missing.vtf"
        ]
    );
}
