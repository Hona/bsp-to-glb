use crate::vtf::{VtfErrorKind, VtfImageSelection, VtfMetadata, decode_vtf, inspect_vtf};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read};

const MAX_PAK_ENTRIES: usize = 65_535;
const MAX_VMT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_VMT_TOKENS: usize = 262_144;
const MAX_VMT_NESTING: usize = 64;
const MAX_VMT_PATCH_DEPTH: usize = 10;
const MAX_VTF_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_MATERIAL_BYTES: u64 = 1024 * 1024 * 1024;

pub const MATERIAL_MANIFEST_VERSION: u32 = 3;
pub const MATERIAL_TEXTURE_MANIFEST_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PakResourceKind {
    Vmt,
    Vtf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PakResource {
    pub path: String,
    pub kind: PakResourceKind,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMaterialResource {
    pub data: Vec<u8>,
    pub provenance: MaterialResourceProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialResourceProvenance {
    pub mount_id: String,
    pub path: String,
    pub crc32: String,
    pub content_hash: String,
}

/// Resolves a canonical Source lookup path such as `materials/brick/wall.vmt`.
///
/// The exporter always checks the BSP PAK before invoking this resolver. A
/// resolver must return the requested bytes with logical mount/path, CRC32,
/// and SHA-256 provenance; it must not return placeholder pixels or claim a
/// resource it cannot provide.
pub trait MaterialResolver {
    fn resolve(&self, lookup_path: &str) -> Result<Option<ResolvedMaterialResource>, String>;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VmtShaderMetadata {
    pub name: String,
    pub family: String,
    pub inputs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VmtTextureInputs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_texture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bump_map: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_map: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VmtFeatures {
    pub unlit: bool,
    pub translucent: bool,
    pub additive: bool,
    pub alpha_test: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_test_reference: Option<f32>,
    pub no_cull: bool,
    pub bump: bool,
    pub ss_bump: bool,
    pub detail: bool,
    pub self_illum: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedMaterialFeatures {
    pub proxies: Vec<String>,
    pub animated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VmtMaterial {
    pub shader: VmtShaderMetadata,
    pub textures: VmtTextureInputs,
    pub features: VmtFeatures,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_prop: Option<String>,
    pub unsupported: UnsupportedMaterialFeatures,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ResourceProvenance {
    Pak {
        mount_id: String,
        path: String,
        crc32: String,
        content_hash: String,
    },
    External {
        mount_id: String,
        path: String,
        crc32: String,
        content_hash: String,
    },
    BuiltIn,
    Unresolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestResource {
    pub lookup_path: String,
    pub provenance: ResourceProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestTexture {
    pub role: String,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lookup_path: Option<String>,
    pub provenance: ResourceProvenance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_source_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMaterialEntry {
    pub material_index: usize,
    pub name: String,
    pub vmt: ManifestResource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<VmtMaterial>,
    pub textures: Vec<ManifestTexture>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResourceMetadata {
    pub mount_id: String,
    pub path: String,
    pub kind: PakResourceKind,
    pub byte_length: usize,
    pub crc32: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedAsset {
    pub kind: PakResourceKind,
    pub lookup_path: String,
    pub referenced_by: String,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLimitations {
    pub vtf_pixel_conversion: String,
    pub proxies: String,
    pub animated_materials: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMaterialManifest {
    pub schema_version: u32,
    pub lookup_policy: String,
    pub materials: Vec<SourceMaterialEntry>,
    pub embedded_resources: Vec<EmbeddedResourceMetadata>,
    pub unresolved_assets: Vec<UnresolvedAsset>,
    pub limitations: MaterialLimitations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TextureDecodeStatus {
    Decoded,
    Unsupported,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTextureOutput {
    pub content_id: String,
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub byte_length: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTextureSource {
    pub lookup_path: String,
    pub provenance: ResourceProvenance,
    pub status: TextureDecodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<VtfMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<MaterialTextureOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTextureManifest {
    pub schema: String,
    pub version: u32,
    pub selection: VtfImageSelection,
    pub sources: Vec<MaterialTextureSource>,
    pub outputs: Vec<MaterialTextureOutput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialTextureArtifact {
    pub content_id: String,
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceMaterialPackage {
    pub material_manifest: SourceMaterialManifest,
    pub manifest: MaterialTextureManifest,
    pub artifacts: Vec<MaterialTextureArtifact>,
}

#[derive(Clone, Debug)]
enum KvValue {
    String(String),
    Object(Vec<(String, KvValue)>),
}

#[derive(Debug)]
enum Token {
    Text(String),
    Conditional(String),
    Open,
    Close,
}

#[derive(Debug)]
struct ParsedVmt {
    shader: String,
    values: Vec<(String, KvValue)>,
}

#[derive(Default)]
struct PatchChanges {
    insert: Vec<(String, KvValue)>,
    replace: Vec<(String, KvValue)>,
}

fn push_token(tokens: &mut Vec<Token>, token: Token) -> Result<(), String> {
    if tokens.len() >= MAX_VMT_TOKENS {
        return Err(format!(
            "VMT exceeds the {MAX_VMT_TOKENS}-token safety limit"
        ));
    }
    tokens.push(token);
    Ok(())
}

fn tokenize_keyvalues(data: &[u8]) -> Result<Vec<Token>, String> {
    let data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    let text = std::str::from_utf8(data).map_err(|error| format!("VMT is not UTF-8: {error}"))?;
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            byte if byte.is_ascii_whitespace() || byte == 0 => index += 1,
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'{' => {
                push_token(&mut tokens, Token::Open)?;
                index += 1;
            }
            b'}' => {
                push_token(&mut tokens, Token::Close)?;
                index += 1;
            }
            b'"' => {
                index += 1;
                let start = index;
                while index < bytes.len() && bytes[index] != b'"' {
                    index += 1;
                }
                if index == bytes.len() {
                    return Err("VMT has an unterminated quoted string".to_owned());
                }
                push_token(&mut tokens, Token::Text(text[start..index].to_owned()))?;
                index += 1;
            }
            _ => {
                let start = index;
                while index < bytes.len() {
                    if bytes[index].is_ascii_whitespace()
                        || matches!(bytes[index], b'{' | b'}')
                        || (bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/'))
                    {
                        break;
                    }
                    index += 1;
                }
                if start == index {
                    return Err(format!("unexpected VMT byte at offset {index}"));
                }
                let value = text[start..index].to_owned();
                let token = if value.starts_with('[') && value.ends_with(']') {
                    Token::Conditional(value)
                } else {
                    Token::Text(value)
                };
                push_token(&mut tokens, token)?;
            }
        }
    }
    Ok(tokens)
}

fn pc_condition(condition: &str) -> Result<bool, String> {
    let condition = condition
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("invalid VMT conditional {condition:?}"))?;
    let (negated, symbol) = condition
        .strip_prefix('!')
        .map_or((false, condition), |symbol| (true, symbol));
    let active = if symbol.eq_ignore_ascii_case("$WIN32") || symbol.eq_ignore_ascii_case("$WINDOWS")
    {
        true
    } else if symbol.eq_ignore_ascii_case("$X360")
        || symbol.eq_ignore_ascii_case("$OSX")
        || symbol.eq_ignore_ascii_case("$LINUX")
        || symbol.eq_ignore_ascii_case("$POSIX")
    {
        false
    } else {
        return Err(format!("unsupported VMT conditional {condition:?}"));
    };
    Ok(active ^ negated)
}

fn parse_object(
    tokens: &[Token],
    index: &mut usize,
    depth: usize,
) -> Result<Vec<(String, KvValue)>, String> {
    if depth > MAX_VMT_NESTING {
        return Err(format!(
            "VMT nesting exceeds the {MAX_VMT_NESTING}-level safety limit"
        ));
    }
    if !matches!(tokens.get(*index), Some(Token::Open)) {
        return Err("expected VMT opening brace".to_owned());
    }
    *index += 1;
    let mut values = Vec::new();
    loop {
        match tokens.get(*index) {
            Some(Token::Close) => {
                *index += 1;
                return Ok(values);
            }
            Some(Token::Text(_)) => {}
            Some(Token::Conditional(_)) => {
                return Err("VMT object has a conditional without a key".to_owned());
            }
            Some(Token::Open) => return Err("VMT object has a block without a key".to_owned()),
            None => return Err("VMT object is missing its closing brace".to_owned()),
        }
        let Token::Text(key) = &tokens[*index] else {
            unreachable!();
        };
        let key = key.clone();
        *index += 1;
        let mut accepted = true;
        if let Some(Token::Conditional(condition)) = tokens.get(*index) {
            accepted = pc_condition(condition)?;
            *index += 1;
        }
        let value = match tokens.get(*index) {
            Some(Token::Text(value)) => {
                *index += 1;
                KvValue::String(value.clone())
            }
            Some(Token::Open) => KvValue::Object(parse_object(tokens, index, depth + 1)?),
            Some(Token::Conditional(_)) | Some(Token::Close) | None => {
                return Err(format!("VMT key {key:?} has no value"));
            }
        };
        if let Some(Token::Conditional(condition)) = tokens.get(*index) {
            if matches!(value, KvValue::Object(_)) {
                return Err(format!(
                    "VMT block {key:?} has an unsupported trailing conditional"
                ));
            }
            accepted &= pc_condition(condition)?;
            *index += 1;
        }
        if accepted {
            values.push((key, value));
        }
    }
}

fn string_input<'a>(values: &'a [(String, KvValue)], key: &str) -> Option<&'a str> {
    values.iter().find_map(|(name, value)| {
        if name.eq_ignore_ascii_case(key)
            && let KvValue::String(value) = value
        {
            Some(value.as_str())
        } else {
            None
        }
    })
}

fn bool_input(values: &[(String, KvValue)], key: &str) -> bool {
    let Some(value) = string_input(values, key) else {
        return false;
    };
    if value.eq_ignore_ascii_case("true") {
        return true;
    }
    if value.eq_ignore_ascii_case("false") {
        return false;
    }
    value.parse::<f64>().is_ok_and(|number| number != 0.0)
}

fn texture_input(values: &[(String, KvValue)], key: &str) -> Option<String> {
    string_input(values, key)
        .map(|value| value.replace('\\', "/"))
        .filter(|value| !value.is_empty())
}

fn shader_family(shader: &str) -> &'static str {
    if shader.eq_ignore_ascii_case("LightmappedGeneric") {
        "lightmappedGeneric"
    } else if shader.eq_ignore_ascii_case("VertexLitGeneric") {
        "vertexLitGeneric"
    } else if shader.eq_ignore_ascii_case("UnlitGeneric") {
        "unlitGeneric"
    } else if shader.eq_ignore_ascii_case("WorldVertexTransition") {
        "worldVertexTransition"
    } else {
        "unsupported"
    }
}

fn parse_vmt_document(data: &[u8]) -> Result<ParsedVmt, String> {
    if data.len() as u64 > MAX_VMT_BYTES {
        return Err(format!("VMT exceeds the {MAX_VMT_BYTES}-byte safety limit"));
    }
    let tokens = tokenize_keyvalues(data)?;
    let Some(Token::Text(shader)) = tokens.first() else {
        return Err("VMT is missing its shader name".to_owned());
    };
    let mut index = 1;
    let root_active = if let Some(Token::Conditional(condition)) = tokens.get(index) {
        index += 1;
        pc_condition(condition)?
    } else {
        true
    };
    let values = parse_object(&tokens, &mut index, 1)?;
    if index != tokens.len() {
        return Err("VMT has trailing content after its root block".to_owned());
    }
    if !root_active {
        return Err("VMT root is inactive for the PC export target".to_owned());
    }
    Ok(ParsedVmt {
        shader: shader.clone(),
        values,
    })
}

fn material_key(key: &str) -> Result<Option<(String, bool)>, String> {
    let Some((condition, key)) = key.split_once('?') else {
        return Ok(Some((key.to_owned(), false)));
    };
    if condition.is_empty() || key.is_empty() {
        return Err(format!("invalid conditional material key {key:?}"));
    }
    let (negated, condition) = condition
        .strip_prefix('!')
        .map_or((false, condition), |condition| (true, condition));
    let active = if condition.eq_ignore_ascii_case("ldr") || condition.eq_ignore_ascii_case("srgb")
    {
        true
    } else if condition.eq_ignore_ascii_case("hdr")
        || condition.eq_ignore_ascii_case("lowfill")
        || condition.eq_ignore_ascii_case("360")
    {
        false
    } else {
        return Err(format!(
            "unsupported material-key conditional {condition:?}"
        ));
    };
    Ok((active ^ negated).then(|| (key.to_owned(), true)))
}

fn effective_material_values(
    values: Vec<(String, KvValue)>,
) -> Result<Vec<(String, KvValue)>, String> {
    let mut effective = Vec::with_capacity(values.len());
    let mut by_name = HashMap::with_capacity(values.len());
    for (key, value) in values {
        let Some((key, conditional)) = material_key(&key)? else {
            continue;
        };
        let lookup = key.to_ascii_lowercase();
        if let Some(index) = by_name.get(&lookup).copied() {
            if conditional {
                effective[index] = (key, value);
            }
        } else {
            by_name.insert(lookup, effective.len());
            effective.push((key, value));
        }
    }
    Ok(effective)
}

fn material_from_document(document: ParsedVmt) -> Result<VmtMaterial, String> {
    if document.shader.eq_ignore_ascii_case("Patch") {
        return Err("Patch VMT requires material dependency resolution".to_owned());
    }
    let values = effective_material_values(document.values)?;

    let mut inputs = BTreeMap::new();
    for (key, value) in &values {
        if let KvValue::String(value) = value {
            inputs
                .entry(key.to_ascii_lowercase())
                .or_insert_with(|| value.clone());
        }
    }
    let proxies = values
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("Proxies"))
        .filter_map(|(_, value)| match value {
            KvValue::Object(values) => Some(values),
            KvValue::String(_) => None,
        })
        .flat_map(|values| values.iter().map(|(name, _)| name.clone()))
        .collect::<Vec<_>>();
    let animated = proxies.iter().any(|proxy| {
        let proxy = proxy.to_ascii_lowercase();
        proxy.contains("animated") || proxy.contains("texturetoggle")
    });
    let textures = VmtTextureInputs {
        base_texture: texture_input(&values, "$basetexture"),
        bump_map: texture_input(&values, "$bumpmap"),
        detail: texture_input(&values, "$detail"),
        env_map: texture_input(&values, "$envmap"),
    };
    let family = shader_family(&document.shader).to_owned();
    let features = VmtFeatures {
        unlit: family == "unlitGeneric",
        translucent: bool_input(&values, "$translucent"),
        additive: bool_input(&values, "$additive"),
        alpha_test: bool_input(&values, "$alphatest"),
        alpha_test_reference: string_input(&values, "$alphatestreference")
            .and_then(|value| value.parse().ok()),
        no_cull: bool_input(&values, "$nocull"),
        bump: textures.bump_map.is_some(),
        ss_bump: bool_input(&values, "$ssbump"),
        detail: textures.detail.is_some(),
        self_illum: bool_input(&values, "$selfillum"),
    };

    Ok(VmtMaterial {
        shader: VmtShaderMetadata {
            name: document.shader,
            family,
            inputs,
        },
        textures,
        features,
        surface_prop: string_input(&values, "$surfaceprop").map(str::to_owned),
        unsupported: UnsupportedMaterialFeatures { proxies, animated },
    })
}

pub fn parse_vmt(data: &[u8]) -> Result<VmtMaterial, String> {
    material_from_document(parse_vmt_document(data)?)
}

fn normalize_archive_path(path: &str) -> Result<String, String> {
    if path.contains('\0') || path.starts_with('/') || path.starts_with('\\') {
        return Err(format!("unsafe PAK path {path:?}"));
    }
    let path = path.replace('\\', "/");
    let mut output = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(format!("unsafe PAK path {path:?}")),
            part if part.contains(':') => return Err(format!("unsafe PAK path {path:?}")),
            part => output.push(part),
        }
    }
    if output.is_empty() {
        return Err(format!("unsafe PAK path {path:?}"));
    }
    Ok(output.join("/"))
}

fn resource_kind(path: &str) -> Option<PakResourceKind> {
    let lower = path.to_ascii_lowercase();
    if !lower.starts_with("materials/") {
        return None;
    }
    if lower.ends_with(".vmt") {
        Some(PakResourceKind::Vmt)
    } else if lower.ends_with(".vtf") {
        Some(PakResourceKind::Vtf)
    } else {
        None
    }
}

fn parse_pak(data: &[u8]) -> Result<Vec<PakResource>, String> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(data))
        .map_err(|error| format!("invalid BSP PAK ZIP: {error}"))?;
    if archive.len() > MAX_PAK_ENTRIES {
        return Err(format!(
            "BSP PAK has {} entries; limit is {MAX_PAK_ENTRIES}",
            archive.len()
        ));
    }
    let mut resources = Vec::new();
    let mut paths = HashMap::new();
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("failed to read BSP PAK entry {index}: {error}"))?;
        if file.is_dir() {
            continue;
        }
        let path = normalize_archive_path(file.name())?;
        let Some(kind) = resource_kind(&path) else {
            continue;
        };
        let size_limit = match kind {
            PakResourceKind::Vmt => MAX_VMT_BYTES,
            PakResourceKind::Vtf => MAX_VTF_BYTES,
        };
        if file.size() > size_limit {
            return Err(format!(
                "BSP PAK resource {path:?} declares {} bytes; limit is {size_limit}",
                file.size()
            ));
        }
        total_size = total_size
            .checked_add(file.size())
            .ok_or_else(|| "BSP PAK material resource size overflows".to_owned())?;
        if total_size > MAX_TOTAL_MATERIAL_BYTES {
            return Err(format!(
                "BSP PAK material resources exceed the {MAX_TOTAL_MATERIAL_BYTES}-byte limit"
            ));
        }
        let lookup_key = path.to_ascii_lowercase();
        if let Some(existing) = paths.insert(lookup_key, path.clone()) {
            return Err(format!(
                "BSP PAK has ambiguous duplicate material paths {existing:?} and {path:?}"
            ));
        }
        let capacity = usize::try_from(file.size())
            .map_err(|_| format!("BSP PAK resource {path:?} is too large for this platform"))?;
        let mut output = Vec::with_capacity(capacity);
        file.by_ref()
            .take(size_limit + 1)
            .read_to_end(&mut output)
            .map_err(|error| format!("failed to decompress BSP PAK resource {path:?}: {error}"))?;
        if output.len() as u64 != file.size() {
            return Err(format!(
                "BSP PAK resource {path:?} decoded to {} bytes, expected {}",
                output.len(),
                file.size()
            ));
        }
        resources.push(PakResource {
            path,
            kind,
            data: output,
        });
    }
    Ok(resources)
}

pub fn read_bsp_pak_resources(data: &[u8]) -> Result<Vec<PakResource>, String> {
    let bsp = super::parse_bsp(data)?;
    parse_pak(&bsp.lumps[super::LUMP_PAKFILE])
}

fn source_lookup_path(reference: &str, extension: &str) -> Result<String, String> {
    let mut reference = reference.trim().replace('\\', "/");
    if reference
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("materials/"))
    {
        reference.drain(..10);
    }
    if reference
        .get(reference.len().saturating_sub(extension.len())..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(extension))
    {
        reference.truncate(reference.len() - extension.len());
    }
    let normalized = normalize_archive_path(&reference)?;
    Ok(format!("materials/{normalized}{extension}"))
}

fn lookup_pak<'a>(
    resources: &'a [PakResource],
    by_path: &HashMap<String, usize>,
    lookup_path: &str,
) -> Option<&'a PakResource> {
    by_path
        .get(&lookup_path.to_ascii_lowercase())
        .and_then(|index| resources.get(*index))
}

fn validate_external(
    resource: ResolvedMaterialResource,
    lookup_path: &str,
    kind: PakResourceKind,
) -> Result<ResolvedMaterialResource, String> {
    if resource.provenance.mount_id.is_empty()
        || resource.provenance.mount_id.len() > 128
        || !resource
            .provenance
            .mount_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(format!(
            "external resolver returned invalid logical mount ID for {lookup_path}"
        ));
    }
    let limit = match kind {
        PakResourceKind::Vmt => MAX_VMT_BYTES,
        PakResourceKind::Vtf => MAX_VTF_BYTES,
    };
    if resource.data.len() as u64 > limit {
        return Err(format!(
            "external resolver returned {} bytes for {lookup_path}; limit is {limit}",
            resource.data.len()
        ));
    }
    if !resource.provenance.path.eq_ignore_ascii_case(lookup_path) {
        return Err(format!(
            "external resolver returned provenance path {:?} for {lookup_path}",
            resource.provenance.path
        ));
    }
    let crc32 = format!("{:08x}", crc32fast::hash(&resource.data));
    if resource.provenance.crc32 != crc32 {
        return Err(format!(
            "external resolver returned incorrect CRC provenance for {lookup_path}"
        ));
    }
    let content_hash = sha256_content_id(&resource.data);
    if resource.provenance.content_hash != content_hash {
        return Err(format!(
            "external resolver returned incorrect content hash provenance for {lookup_path}"
        ));
    }
    Ok(resource)
}

fn pak_provenance(path: &str, data: &[u8]) -> ResourceProvenance {
    ResourceProvenance::Pak {
        mount_id: "bspPak".to_owned(),
        path: path.to_ascii_lowercase(),
        crc32: format!("{:08x}", crc32fast::hash(data)),
        content_hash: sha256_content_id(data),
    }
}

fn external_provenance(provenance: MaterialResourceProvenance) -> ResourceProvenance {
    ResourceProvenance::External {
        mount_id: provenance.mount_id,
        path: provenance.path,
        crc32: provenance.crc32,
        content_hash: provenance.content_hash,
    }
}

fn resolve_external(
    resolver: Option<&dyn MaterialResolver>,
    lookup_path: &str,
    kind: PakResourceKind,
) -> Result<Option<ResolvedMaterialResource>, String> {
    resolver
        .map(|resolver| {
            resolver
                .resolve(lookup_path)
                .map_err(|error| format!("material resolver failed for {lookup_path}: {error}"))?
                .map(|resource| validate_external(resource, lookup_path, kind))
                .transpose()
        })
        .transpose()
        .map(Option::flatten)
}

fn apply_patch_entries(
    destination: &mut Vec<(String, KvValue)>,
    source: &[(String, KvValue)],
    replace_only: bool,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_VMT_NESTING {
        return Err(format!(
            "VMT patch nesting exceeds the {MAX_VMT_NESTING}-level safety limit"
        ));
    }
    let mut by_name = HashMap::with_capacity(destination.len() + source.len());
    for (index, (key, _)) in destination.iter().enumerate() {
        by_name.entry(key.to_ascii_lowercase()).or_insert(index);
    }
    for (key, value) in source {
        let lookup = key.to_ascii_lowercase();
        if let Some(index) = by_name.get(&lookup).copied() {
            if let KvValue::Object(source_values) = value {
                if !matches!(destination[index].1, KvValue::Object(_)) {
                    destination[index].1 = KvValue::Object(Vec::new());
                }
                let KvValue::Object(destination_values) = &mut destination[index].1 else {
                    unreachable!();
                };
                apply_patch_entries(destination_values, source_values, replace_only, depth + 1)?;
            } else {
                destination[index].1 = value.clone();
            }
        } else if !replace_only {
            by_name.insert(lookup, destination.len());
            destination.push((key.clone(), value.clone()));
        }
    }
    Ok(())
}

fn patch_section<'a>(
    values: &'a [(String, KvValue)],
    key: &str,
) -> Result<Option<&'a [(String, KvValue)]>, String> {
    let Some((_, value)) = values
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
    else {
        return Ok(None);
    };
    match value {
        KvValue::Object(values) => Ok(Some(values)),
        KvValue::String(_) => Err(format!("Patch {key:?} must be an object")),
    }
}

fn accumulate_patch(document: &ParsedVmt, changes: &mut PatchChanges) -> Result<String, String> {
    for (key, _) in &document.values {
        if !key.eq_ignore_ascii_case("include")
            && !key.eq_ignore_ascii_case("insert")
            && !key.eq_ignore_ascii_case("replace")
        {
            return Err(format!("unsupported Patch command {key:?}"));
        }
    }
    let include = string_input(&document.values, "include")
        .filter(|include| !include.trim().is_empty())
        .ok_or_else(|| "Patch VMT is missing a non-empty include".to_owned())?;
    if let Some(insert) = patch_section(&document.values, "insert")? {
        apply_patch_entries(&mut changes.insert, insert, false, 1)?;
    }
    if let Some(replace) = patch_section(&document.values, "replace")? {
        apply_patch_entries(&mut changes.replace, replace, false, 1)?;
    }
    Ok(include.to_owned())
}

fn resolve_effective_vmt(
    root_data: &[u8],
    root_path: &str,
    embedded_resources: &[PakResource],
    by_path: &HashMap<String, usize>,
    resolver: Option<&dyn MaterialResolver>,
) -> Result<VmtMaterial, String> {
    let mut document = parse_vmt_document(root_data)
        .map_err(|error| format!("failed to parse {root_path}: {error}"))?;
    let mut current_path = root_path.to_owned();
    let mut visited = HashSet::from([root_path.to_ascii_lowercase()]);
    let mut changes = PatchChanges::default();
    let mut patch_depth = 0;

    while document.shader.eq_ignore_ascii_case("Patch") {
        if patch_depth >= MAX_VMT_PATCH_DEPTH {
            return Err(format!(
                "Patch include depth and dependency count exceed the {MAX_VMT_PATCH_DEPTH}-file safety limit at {current_path}"
            ));
        }
        let include = accumulate_patch(&document, &mut changes)
            .map_err(|error| format!("invalid Patch VMT {current_path}: {error}"))?;
        let include_path = source_lookup_path(&include, ".vmt")?;
        if !visited.insert(include_path.to_ascii_lowercase()) {
            return Err(format!(
                "Patch include cycle from {current_path} to {include_path}"
            ));
        }

        let data = if let Some(resource) = lookup_pak(embedded_resources, by_path, &include_path) {
            Cow::Borrowed(resource.data.as_slice())
        } else if let Some(resource) =
            resolve_external(resolver, &include_path, PakResourceKind::Vmt)?
        {
            Cow::Owned(resource.data)
        } else {
            return Err(format!(
                "Patch VMT {current_path} includes unavailable dependency {include_path}"
            ));
        };
        document = parse_vmt_document(&data)
            .map_err(|error| format!("failed to parse Patch dependency {include_path}: {error}"))?;
        current_path = include_path;
        patch_depth += 1;
    }

    apply_patch_entries(&mut document.values, &changes.insert, false, 1)?;
    apply_patch_entries(&mut document.values, &changes.replace, true, 1)?;
    material_from_document(document)
}

struct TexturePackageBuilder {
    selection: VtfImageSelection,
    sources: Vec<MaterialTextureSource>,
    source_by_path: HashMap<String, usize>,
    outputs: Vec<MaterialTextureOutput>,
    artifacts: Vec<MaterialTextureArtifact>,
    artifact_by_pixels: HashMap<String, usize>,
}

impl TexturePackageBuilder {
    fn new(selection: VtfImageSelection) -> Self {
        Self {
            selection,
            sources: Vec::new(),
            source_by_path: HashMap::new(),
            outputs: Vec::new(),
            artifacts: Vec::new(),
            artifact_by_pixels: HashMap::new(),
        }
    }

    fn source(&self, lookup_path: &str) -> Option<(usize, ResourceProvenance)> {
        self.source_by_path
            .get(&lookup_path.to_ascii_lowercase())
            .map(|index| (*index, self.sources[*index].provenance.clone()))
    }

    fn add_source(
        &mut self,
        lookup_path: String,
        provenance: ResourceProvenance,
        data: &[u8],
    ) -> Result<usize, String> {
        let (status, metadata, output, error) = match decode_vtf(data, self.selection) {
            Ok(decoded) => {
                let pixel_key = rgba_pixel_key(decoded.width, decoded.height, &decoded.pixels);
                let output = if let Some(index) = self.artifact_by_pixels.get(&pixel_key) {
                    self.outputs[*index].clone()
                } else {
                    let png = encode_rgba_png(decoded.width, decoded.height, &decoded.pixels)?;
                    let content_id = sha256_content_id(&png);
                    let digest = content_id
                        .strip_prefix("sha256:")
                        .expect("RGBA content IDs use SHA-256");
                    let file_name = format!("sha256-{digest}.png");
                    let output = MaterialTextureOutput {
                        content_id: content_id.clone(),
                        file_name: file_name.clone(),
                        width: decoded.width,
                        height: decoded.height,
                        byte_length: png.len(),
                    };
                    let index = self.artifacts.len();
                    self.artifact_by_pixels.insert(pixel_key, index);
                    self.outputs.push(output.clone());
                    self.artifacts.push(MaterialTextureArtifact {
                        content_id,
                        file_name,
                        width: decoded.width,
                        height: decoded.height,
                        png,
                    });
                    output
                };
                (
                    TextureDecodeStatus::Decoded,
                    Some(decoded.metadata),
                    Some(output),
                    None,
                )
            }
            Err(decode_error) => {
                let metadata = inspect_vtf(data).ok();
                let status = match decode_error.kind {
                    VtfErrorKind::Invalid => TextureDecodeStatus::Invalid,
                    VtfErrorKind::Unsupported => TextureDecodeStatus::Unsupported,
                };
                (status, metadata, None, Some(decode_error.message))
            }
        };
        let index = self.sources.len();
        self.source_by_path
            .insert(lookup_path.to_ascii_lowercase(), index);
        self.sources.push(MaterialTextureSource {
            lookup_path,
            provenance,
            status,
            metadata,
            output,
            error,
        });
        Ok(index)
    }

    fn finish(self) -> (MaterialTextureManifest, Vec<MaterialTextureArtifact>) {
        (
            MaterialTextureManifest {
                schema: "bsp-to-glb/material-textures".to_owned(),
                version: MATERIAL_TEXTURE_MANIFEST_VERSION,
                selection: self.selection,
                sources: self.sources,
                outputs: self.outputs,
            },
            self.artifacts,
        )
    }
}

fn rgba_pixel_key(width: u32, height: u32, pixels: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(width.to_le_bytes());
    hasher.update(height.to_le_bytes());
    hasher.update(pixels);
    format!("{:x}", hasher.finalize())
}

fn sha256_content_id(data: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(data))
}

fn encode_rgba_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "material texture dimensions overflow".to_owned())?;
    if width == 0 || height == 0 || pixels.len() != expected {
        return Err("material texture dimensions do not match its RGBA pixels".to_owned());
    }
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("failed to encode material texture PNG header: {error}"))?;
        writer
            .write_image_data(pixels)
            .map_err(|error| format!("failed to encode material texture PNG pixels: {error}"))?;
    }
    Ok(output)
}

struct MaterialBuild {
    manifest: SourceMaterialManifest,
    texture_package: Option<(MaterialTextureManifest, Vec<MaterialTextureArtifact>)>,
}

fn build_materials(
    material_names: &[String],
    embedded_resources: &[PakResource],
    resolver: Option<&dyn MaterialResolver>,
    texture_selection: Option<VtfImageSelection>,
) -> Result<MaterialBuild, String> {
    let mut by_path = HashMap::new();
    for (index, resource) in embedded_resources.iter().enumerate() {
        let normalized = normalize_archive_path(&resource.path)?;
        if normalized != resource.path || resource_kind(&normalized) != Some(resource.kind) {
            return Err(format!(
                "invalid embedded material resource path {:?}",
                resource.path
            ));
        }
        if by_path
            .insert(resource.path.to_ascii_lowercase(), index)
            .is_some()
        {
            return Err(format!(
                "duplicate embedded material resource path {:?}",
                resource.path
            ));
        }
    }

    let mut materials = Vec::with_capacity(material_names.len());
    let mut unresolved_assets = Vec::new();
    let mut texture_package = texture_selection.map(TexturePackageBuilder::new);
    for (material_index, name) in material_names.iter().enumerate() {
        let vmt_path = source_lookup_path(name, ".vmt")?;
        let (vmt_data, vmt_provenance) = if let Some(resource) =
            lookup_pak(embedded_resources, &by_path, &vmt_path)
        {
            (
                Some(resource.data.clone()),
                pak_provenance(&vmt_path, &resource.data),
            )
        } else if let Some(resource) = resolve_external(resolver, &vmt_path, PakResourceKind::Vmt)?
        {
            let provenance = external_provenance(resource.provenance);
            (Some(resource.data), provenance)
        } else {
            unresolved_assets.push(UnresolvedAsset {
                kind: PakResourceKind::Vmt,
                lookup_path: vmt_path.clone(),
                referenced_by: name.clone(),
                role: "materialDefinition".to_owned(),
            });
            (None, ResourceProvenance::Unresolved)
        };
        let metadata = vmt_data
            .as_deref()
            .map(|data| {
                resolve_effective_vmt(data, &vmt_path, embedded_resources, &by_path, resolver)
            })
            .transpose()?;
        let mut textures = Vec::new();
        if let Some(material) = &metadata {
            let references = [
                ("baseTexture", material.textures.base_texture.as_deref()),
                ("bumpMap", material.textures.bump_map.as_deref()),
                ("detail", material.textures.detail.as_deref()),
                ("envMap", material.textures.env_map.as_deref()),
            ];
            for (role, reference) in references {
                let Some(reference) = reference else {
                    continue;
                };
                let built_in = role == "envMap"
                    && (reference.eq_ignore_ascii_case("env_cubemap")
                        || reference.to_ascii_lowercase().starts_with("_rt_"));
                if built_in {
                    textures.push(ManifestTexture {
                        role: role.to_owned(),
                        reference: reference.to_owned(),
                        lookup_path: None,
                        provenance: ResourceProvenance::BuiltIn,
                        package_source_index: None,
                    });
                    continue;
                }
                let lookup_path = source_lookup_path(reference, ".vtf")?;
                let existing_source = texture_package
                    .as_ref()
                    .and_then(|package| package.source(&lookup_path));
                let (provenance, package_source_index) = if let Some((index, provenance)) =
                    existing_source
                {
                    (provenance, Some(index))
                } else {
                    let (data, provenance) = if let Some(resource) =
                        lookup_pak(embedded_resources, &by_path, &lookup_path)
                    {
                        (
                            Some(Cow::Borrowed(resource.data.as_slice())),
                            pak_provenance(&lookup_path, &resource.data),
                        )
                    } else if let Some(resource) =
                        resolve_external(resolver, &lookup_path, PakResourceKind::Vtf)?
                    {
                        (
                            Some(Cow::Owned(resource.data)),
                            external_provenance(resource.provenance),
                        )
                    } else {
                        unresolved_assets.push(UnresolvedAsset {
                            kind: PakResourceKind::Vtf,
                            lookup_path: lookup_path.clone(),
                            referenced_by: name.clone(),
                            role: role.to_owned(),
                        });
                        (None, ResourceProvenance::Unresolved)
                    };
                    let index = if let (Some(package), Some(data)) =
                        (texture_package.as_mut(), data.as_deref())
                    {
                        Some(package.add_source(lookup_path.clone(), provenance.clone(), data)?)
                    } else {
                        None
                    };
                    (provenance, index)
                };
                textures.push(ManifestTexture {
                    role: role.to_owned(),
                    reference: reference.to_owned(),
                    lookup_path: Some(lookup_path),
                    provenance,
                    package_source_index,
                });
            }
        }
        materials.push(SourceMaterialEntry {
            material_index,
            name: name.clone(),
            vmt: ManifestResource {
                lookup_path: vmt_path,
                provenance: vmt_provenance,
            },
            metadata,
            textures,
        });
    }

    let manifest = SourceMaterialManifest {
        schema_version: MATERIAL_MANIFEST_VERSION,
        lookup_policy: "pakFirst".to_owned(),
        materials,
        embedded_resources: embedded_resources
            .iter()
            .map(|resource| EmbeddedResourceMetadata {
                mount_id: "bspPak".to_owned(),
                path: resource.path.clone(),
                kind: resource.kind,
                byte_length: resource.data.len(),
                crc32: format!("{:08x}", crc32fast::hash(&resource.data)),
                content_hash: sha256_content_id(&resource.data),
            })
            .collect(),
        unresolved_assets,
        limitations: MaterialLimitations {
            vtf_pixel_conversion: "optionalSelectedRgbaPngPackage".to_owned(),
            proxies: "metadataOnly".to_owned(),
            animated_materials: "metadataOnly".to_owned(),
        },
    };
    Ok(MaterialBuild {
        manifest,
        texture_package: texture_package.map(TexturePackageBuilder::finish),
    })
}

pub fn build_source_material_manifest(
    material_names: &[String],
    embedded_resources: &[PakResource],
    resolver: Option<&dyn MaterialResolver>,
) -> Result<SourceMaterialManifest, String> {
    Ok(build_materials(material_names, embedded_resources, resolver, None)?.manifest)
}

pub fn build_source_material_package(
    material_names: &[String],
    embedded_resources: &[PakResource],
    resolver: Option<&dyn MaterialResolver>,
    selection: VtfImageSelection,
) -> Result<SourceMaterialPackage, String> {
    let built = build_materials(
        material_names,
        embedded_resources,
        resolver,
        Some(selection),
    )?;
    let (manifest, artifacts) = built
        .texture_package
        .expect("texture selection creates a material texture package");
    Ok(SourceMaterialPackage {
        material_manifest: built.manifest,
        manifest,
        artifacts,
    })
}

pub(crate) fn parse_embedded_resources(data: &[u8]) -> Result<Vec<PakResource>, String> {
    parse_pak(data)
}
