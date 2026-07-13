use crate::vtf::{VtfErrorKind, VtfImageSelection, VtfMetadata, decode_vtf, inspect_vtf};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};

const MAX_PAK_ENTRIES: usize = 65_535;
const MAX_VMT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_VTF_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_MATERIAL_BYTES: u64 = 1024 * 1024 * 1024;

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
    pub provenance: String,
}

/// Resolves a canonical Source lookup path such as `materials/brick/wall.vmt`.
///
/// The exporter always checks the BSP PAK before invoking this resolver. A
/// resolver must return the requested bytes and a stable provenance label; it
/// must not return placeholder pixels or claim a resource it cannot provide.
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
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceProvenance {
    Pak { path: String },
    External { resolver: String },
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
    pub path: String,
    pub kind: PakResourceKind,
    pub byte_length: usize,
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

#[derive(Debug)]
enum KvValue {
    String(String),
    Object(Vec<(String, KvValue)>),
}

#[derive(Debug)]
enum Token {
    Text(String),
    Open,
    Close,
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
                tokens.push(Token::Open);
                index += 1;
            }
            b'}' => {
                tokens.push(Token::Close);
                index += 1;
            }
            b'"' => {
                index += 1;
                let mut value = Vec::new();
                let mut closed = false;
                while index < bytes.len() {
                    match bytes[index] {
                        b'"' => {
                            closed = true;
                            index += 1;
                            break;
                        }
                        b'\\' => {
                            let escaped = *bytes
                                .get(index + 1)
                                .ok_or_else(|| "VMT has a trailing escape".to_owned())?;
                            match escaped {
                                b'\\' | b'"' => value.push(escaped),
                                b'n' => value.push(b'\n'),
                                b't' => value.push(b'\t'),
                                _ => {
                                    value.push(b'\\');
                                    value.push(escaped);
                                }
                            }
                            index += 2;
                        }
                        byte => {
                            value.push(byte);
                            index += 1;
                        }
                    }
                }
                if !closed {
                    return Err("VMT has an unterminated quoted string".to_owned());
                }
                tokens.push(Token::Text(String::from_utf8(value).map_err(|error| {
                    format!("VMT quoted value is not UTF-8: {error}")
                })?));
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
                tokens.push(Token::Text(text[start..index].to_owned()));
            }
        }
    }
    Ok(tokens)
}

fn parse_object(tokens: &[Token], index: &mut usize) -> Result<Vec<(String, KvValue)>, String> {
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
            Some(Token::Open) => return Err("VMT object has a block without a key".to_owned()),
            None => return Err("VMT object is missing its closing brace".to_owned()),
        }
        let Token::Text(key) = &tokens[*index] else {
            unreachable!();
        };
        let key = key.clone();
        *index += 1;
        let value = match tokens.get(*index) {
            Some(Token::Text(value)) => {
                *index += 1;
                KvValue::String(value.clone())
            }
            Some(Token::Open) => KvValue::Object(parse_object(tokens, index)?),
            Some(Token::Close) | None => {
                return Err(format!("VMT key {key:?} has no value"));
            }
        };
        values.push((key, value));
    }
}

fn string_input<'a>(values: &'a [(String, KvValue)], key: &str) -> Option<&'a str> {
    values.iter().rev().find_map(|(name, value)| {
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

pub fn parse_vmt(data: &[u8]) -> Result<VmtMaterial, String> {
    if data.len() as u64 > MAX_VMT_BYTES {
        return Err(format!("VMT exceeds the {MAX_VMT_BYTES}-byte safety limit"));
    }
    let tokens = tokenize_keyvalues(data)?;
    let Some(Token::Text(shader)) = tokens.first() else {
        return Err("VMT is missing its shader name".to_owned());
    };
    let mut index = 1;
    let values = parse_object(&tokens, &mut index)?;
    if index != tokens.len() {
        return Err("VMT has trailing content after its root block".to_owned());
    }

    let inputs = values
        .iter()
        .filter_map(|(key, value)| match value {
            KvValue::String(value) => Some((key.to_ascii_lowercase(), value.clone())),
            KvValue::Object(_) => None,
        })
        .collect();
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
    let family = shader_family(shader).to_owned();
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
            name: shader.clone(),
            family,
            inputs,
        },
        textures,
        features,
        surface_prop: string_input(&values, "$surfaceprop").map(str::to_owned),
        unsupported: UnsupportedMaterialFeatures { proxies, animated },
    })
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
    if resource.provenance.trim().is_empty() {
        return Err(format!(
            "external resolver returned empty provenance for {lookup_path}"
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
    Ok(resource)
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
                version: 1,
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
                ResourceProvenance::Pak {
                    path: resource.path.clone(),
                },
            )
        } else if let Some(resource) = resolve_external(resolver, &vmt_path, PakResourceKind::Vmt)?
        {
            let provenance = ResourceProvenance::External {
                resolver: resource.provenance,
            };
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
        let metadata = vmt_data.as_deref().map(parse_vmt).transpose()?;
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
                            ResourceProvenance::Pak {
                                path: resource.path.clone(),
                            },
                        )
                    } else if let Some(resource) =
                        resolve_external(resolver, &lookup_path, PakResourceKind::Vtf)?
                    {
                        (
                            Some(Cow::Owned(resource.data)),
                            ResourceProvenance::External {
                                resolver: resource.provenance,
                            },
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
        schema_version: 2,
        lookup_policy: "pakFirst".to_owned(),
        materials,
        embedded_resources: embedded_resources
            .iter()
            .map(|resource| EmbeddedResourceMetadata {
                path: resource.path.clone(),
                kind: resource.kind,
                byte_length: resource.data.len(),
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
