use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const STUDIO_MODEL_PACKAGE_VERSION: u32 = 2;
pub const STUDIO_MODEL_MDL_VERSION: i32 = 48;
pub const STUDIO_MODEL_MDL_VERSIONS: [i32; 4] = [44, 45, 46, 48];
pub const STUDIO_MODEL_VVD_VERSION: i32 = 4;
pub const STUDIO_MODEL_VTX_VERSION: i32 = 7;

const MDL_HEADER_BYTES: usize = 408;
const MDL_BONE_BYTES: usize = 216;
const MDL_ANIMATION_BYTES: usize = 100;
const MDL_SEQUENCE_BYTES: usize = 212;
const MDL_TEXTURE_BYTES: usize = 64;
const MDL_BODYPART_BYTES: usize = 16;
const MDL_MODEL_BYTES: usize = 148;
const MDL_MESH_BYTES: usize = 116;
const MDL_ATTACHMENT_BYTES: usize = 92;
const MDL_FLEX_BYTES: usize = 60;
const MDL_FLEX_DESCRIPTOR_BYTES: usize = 4;
const MDL_FLEX_CONTROLLER_BYTES: usize = 20;
const MDL_FLEX_RULE_BYTES: usize = 12;
const MDL_FLEX_OPERATION_BYTES: usize = 8;
const VVD_HEADER_BYTES: usize = 64;
const VVD_FIXUP_BYTES: usize = 12;
const VVD_VERTEX_BYTES: usize = 48;
const VVD_TANGENT_BYTES: usize = 16;
const VTX_HEADER_BYTES: usize = 36;
const VTX_BODYPART_BYTES: usize = 8;
const VTX_MODEL_BYTES: usize = 8;
const VTX_LOD_BYTES: usize = 12;
const VTX_MESH_BYTES: usize = 9;
const VTX_STRIP_GROUP_BYTES: usize = 25;
const VTX_VERTEX_BYTES: usize = 9;
const VTX_STRIP_BYTES: usize = 27;
const SOURCE_TO_GLTF_ROTATION: [f32; 4] = [-0.70710677, 0.0, 0.0, 0.70710677];

const MAX_BONES: usize = 128;
const MAX_BODY_PARTS: usize = 4_096;
const MAX_MODELS: usize = 4_096;
const MAX_MESHES: usize = 65_536;
const MAX_VERTICES: usize = 4_000_000;
const MAX_INDICES: usize = 12_000_000;
const MAX_LODS: usize = 8;
const MAX_MATERIALS: usize = 4_096;
const MAX_SKIN_FAMILIES: usize = 256;
const MAX_ANIMATIONS: usize = 4_096;
const MAX_SEQUENCES: usize = 4_096;
const MAX_ATTACHMENTS: usize = 4_096;
const MAX_FLEXES: usize = 65_536;
const MAX_FLEX_VERTICES: usize = 4_000_000;
const MAX_FLEX_OPERATIONS: usize = 65_536;
const MAX_STRING_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StudioFeatureStatus {
    Supported,
    DetectedOnly,
    Unsupported,
    Missing,
    Malformed,
    NotPresent,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioDomainStatus {
    pub status: StudioFeatureStatus,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSourceFile {
    pub role: &'static str,
    pub extension: &'static str,
    pub byte_length: usize,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<i32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioBone {
    pub index: usize,
    pub name: String,
    pub parent: i32,
    pub position: [f32; 3],
    pub quaternion: [f32; 4],
    pub rotation_euler: [f32; 3],
    pub position_scale: [f32; 3],
    pub rotation_scale: [f32; 3],
    pub pose_to_bone: [f32; 12],
    pub alignment: [f32; 4],
    pub flags: i32,
    pub contents: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioAttachment {
    pub index: usize,
    pub name: String,
    pub flags: u32,
    pub bone: usize,
    pub local: [f32; 12],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gltf_node: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioAnimation {
    pub index: usize,
    pub name: String,
    pub fps: f32,
    pub flags: i32,
    pub frame_count: usize,
    pub animation_block: i32,
    pub section_frame_count: usize,
    pub ik_rule_count: usize,
    pub local_hierarchy_count: usize,
    pub zero_frame_count: usize,
    pub decode_status: StudioFeatureStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gltf_animation: Option<usize>,
    pub sample_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSequence {
    pub index: usize,
    pub name: String,
    pub activity_name: String,
    pub flags: i32,
    pub activity: i32,
    pub activity_weight: i32,
    pub group_size: [usize; 2],
    pub animation_indices: Vec<i16>,
    pub event_count: usize,
    pub auto_layers: Vec<StudioSequenceLayer>,
    pub ik_locks: Vec<StudioSequenceIkLock>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSequenceLayer {
    pub sequence: i16,
    pub pose: i16,
    pub flags: i32,
    pub start: f32,
    pub peak: f32,
    pub tail: f32,
    pub end: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSequenceIkLock {
    pub chain: i32,
    pub position_weight: f32,
    pub local_rotation_weight: f32,
    pub flags: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioMaterial {
    pub index: usize,
    pub name: String,
    pub search_paths: Vec<String>,
    pub candidates: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSkinFamily {
    pub index: usize,
    pub texture_indices: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioMeshLod {
    pub lod: usize,
    pub switch_point: f32,
    pub gltf_mesh: usize,
    pub gltf_node: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub material_slot: usize,
    pub material_index: usize,
    pub morph_targets: Vec<StudioMorphTarget>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioMorphTarget {
    pub target: usize,
    pub flex_descriptor: usize,
    pub flex_pair: i32,
    pub thresholds: [f32; 4],
    pub affected_vertices: usize,
    pub vertex_animation_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrinkle_accessor: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioFlexDescriptor {
    pub index: usize,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioFlexController {
    pub index: usize,
    pub controller_type: String,
    pub name: String,
    pub minimum: f32,
    pub maximum: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioFlexOperation {
    pub operation: i32,
    pub operand_bits: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioFlexRule {
    pub index: usize,
    pub flex_descriptor: i32,
    pub operations: Vec<StudioFlexOperation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSubModel {
    pub index: usize,
    pub name: String,
    pub vertex_count: usize,
    pub mesh_lods: Vec<StudioMeshLod>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioBodyPart {
    pub index: usize,
    pub name: String,
    pub base: i32,
    pub default_model: usize,
    pub models: Vec<StudioSubModel>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioModelFormatMatrix {
    pub mdl: StudioDomainStatus,
    pub vvd: StudioDomainStatus,
    pub vtx: StudioDomainStatus,
    pub ani: StudioDomainStatus,
    pub phy: StudioDomainStatus,
    pub geometry: StudioDomainStatus,
    pub skins: StudioDomainStatus,
    pub bodygroups: StudioDomainStatus,
    pub lods: StudioDomainStatus,
    pub skeleton: StudioDomainStatus,
    pub animations: StudioDomainStatus,
    pub sequences: StudioDomainStatus,
    pub attachments: StudioDomainStatus,
    pub flexes: StudioDomainStatus,
    pub include_models: StudioDomainStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioModelManifest {
    pub schema: &'static str,
    pub schema_version: u32,
    pub source_path: String,
    pub package_content_hash: String,
    pub selected_skin: usize,
    pub checksum: i32,
    pub mdl_version: i32,
    pub mdl_name: String,
    pub flags: i32,
    pub bounds: StudioBounds,
    pub source_files: Vec<StudioSourceFile>,
    pub formats: StudioModelFormatMatrix,
    pub materials: Vec<StudioMaterial>,
    pub skin_families: Vec<StudioSkinFamily>,
    pub body_parts: Vec<StudioBodyPart>,
    pub bones: Vec<StudioBone>,
    pub animations: Vec<StudioAnimation>,
    pub sequences: Vec<StudioSequence>,
    pub attachments: Vec<StudioAttachment>,
    pub include_models: Vec<String>,
    pub flex_descriptors: Vec<StudioFlexDescriptor>,
    pub flex_controllers: Vec<StudioFlexController>,
    pub flex_rules: Vec<StudioFlexRule>,
    pub physics_link: StudioPhysicsLink,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioBounds {
    pub eye_position: [f32; 3],
    pub illumination_position: [f32; 3],
    pub hull_min: [f32; 3],
    pub hull_max: [f32; 3],
    pub view_min: [f32; 3],
    pub view_max: [f32; 3],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioPhysicsLink {
    pub status: StudioFeatureStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug)]
pub struct StudioModelExport {
    pub glb: Vec<u8>,
    pub manifest: StudioModelManifest,
}

pub struct StudioModelInput<'a> {
    pub source_path: &'a str,
    pub mdl: &'a [u8],
    pub vvd: &'a [u8],
    pub vtx: &'a [u8],
    pub ani: Option<&'a [u8]>,
    pub phy: Option<&'a [u8]>,
    pub skin: usize,
}

#[derive(Clone)]
struct MdlMesh {
    material: i32,
    vertex_count: usize,
    vertex_offset: usize,
    flexes: Vec<MdlFlex>,
}

#[derive(Clone)]
struct MdlFlexVertex {
    position_delta: [f32; 3],
    normal_delta: [f32; 3],
    wrinkle_delta: Option<f32>,
}

#[derive(Clone)]
struct MdlFlex {
    descriptor: usize,
    thresholds: [f32; 4],
    pair: i32,
    animation_type: u8,
    vertices: BTreeMap<usize, MdlFlexVertex>,
}

#[derive(Clone)]
struct MdlModel {
    name: String,
    vertex_count: usize,
    vertex_start: usize,
    meshes: Vec<MdlMesh>,
}

#[derive(Clone)]
struct ParsedBodyPart {
    name: String,
    base: i32,
    models: Vec<MdlModel>,
}

struct ParsedMdl {
    checksum: i32,
    version: i32,
    name: String,
    flags: i32,
    bounds: StudioBounds,
    bones: Vec<StudioBone>,
    animations: Vec<MdlAnimation>,
    sequences: Vec<StudioSequence>,
    attachments: Vec<StudioAttachment>,
    materials: Vec<StudioMaterial>,
    skins: Vec<StudioSkinFamily>,
    body_parts: Vec<ParsedBodyPart>,
    include_models: Vec<String>,
    flex_descriptors: Vec<StudioFlexDescriptor>,
    flex_controllers: Vec<StudioFlexController>,
    flex_rules: Vec<StudioFlexRule>,
    mesh_flex_count: usize,
    animation_block_count: usize,
    animation_blocks: Vec<(usize, usize)>,
}

#[derive(Clone)]
struct MdlAnimationSection {
    block: i32,
    index: i32,
}

#[derive(Clone)]
struct MdlAnimation {
    metadata: StudioAnimation,
    descriptor_offset: usize,
    animation_index: i32,
    sections: Vec<MdlAnimationSection>,
}

#[derive(Clone, Copy)]
struct VvdVertex {
    weights: [f32; 3],
    bones: [u8; 3],
    bone_count: u8,
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    tangent: [f32; 4],
}

struct ParsedVvd {
    checksum: i32,
    lod_vertex_counts: Vec<usize>,
    source_vertices: Vec<VvdVertex>,
    fixups: Vec<VvdFixup>,
}

#[derive(Clone, Copy)]
struct VvdFixup {
    lod: usize,
    source: usize,
    destination: usize,
    count: usize,
}

impl ParsedVvd {
    fn vertex(&self, lod: usize, destination: usize) -> Result<VvdVertex, String> {
        if lod >= self.lod_vertex_counts.len() {
            return Err(format!("VVD LOD {lod} is missing"));
        }
        let source = if self.fixups.is_empty() {
            destination
        } else {
            let fixup = self
                .fixups
                .iter()
                .find(|fixup| {
                    destination >= fixup.destination
                        && destination < fixup.destination + fixup.count
                })
                .ok_or_else(|| {
                    format!("VVD destination vertex {destination} has no fixup mapping")
                })?;
            if fixup.lod < lod {
                return Err(format!(
                    "VVD destination vertex {destination} is culled from LOD {lod}"
                ));
            }
            fixup.source + destination - fixup.destination
        };
        self.source_vertices
            .get(source)
            .copied()
            .ok_or_else(|| format!("VVD source vertex {source} is missing"))
    }
}

#[derive(Clone)]
struct VtxStripGroup {
    source_vertex_ids: Vec<usize>,
    triangles: Vec<[usize; 3]>,
}

#[derive(Clone)]
struct VtxMesh {
    strip_groups: Vec<VtxStripGroup>,
}

#[derive(Clone)]
struct VtxLod {
    switch_point: f32,
    meshes: Vec<VtxMesh>,
}

#[derive(Clone)]
struct VtxModel {
    lods: Vec<VtxLod>,
}

#[derive(Clone)]
struct VtxBodyPart {
    models: Vec<VtxModel>,
}

struct ParsedVtx {
    checksum: i32,
    lod_count: usize,
    body_parts: Vec<VtxBodyPart>,
    material_replacements: Vec<BTreeMap<usize, String>>,
}

fn checked_range<'a>(
    data: &'a [u8],
    offset: usize,
    length: usize,
    context: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| format!("{context} range overflows"))?;
    data.get(offset..end).ok_or_else(|| {
        format!(
            "{context} range {offset}..{end} exceeds {} bytes",
            data.len()
        )
    })
}

fn table_range<'a>(
    data: &'a [u8],
    offset: usize,
    count: usize,
    stride: usize,
    maximum: usize,
    context: &str,
) -> Result<&'a [u8], String> {
    if count > maximum {
        return Err(format!("{context} count {count} exceeds {maximum}"));
    }
    let length = count
        .checked_mul(stride)
        .ok_or_else(|| format!("{context} byte length overflows"))?;
    checked_range(data, offset, length, context)
}

fn i16_at(data: &[u8], offset: usize, context: &str) -> Result<i16, String> {
    Ok(i16::from_le_bytes(
        checked_range(data, offset, 2, context)?.try_into().unwrap(),
    ))
}

fn u16_at(data: &[u8], offset: usize, context: &str) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        checked_range(data, offset, 2, context)?.try_into().unwrap(),
    ))
}

fn i32_at(data: &[u8], offset: usize, context: &str) -> Result<i32, String> {
    Ok(i32::from_le_bytes(
        checked_range(data, offset, 4, context)?.try_into().unwrap(),
    ))
}

fn u32_at(data: &[u8], offset: usize, context: &str) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        checked_range(data, offset, 4, context)?.try_into().unwrap(),
    ))
}

fn usize_i32(data: &[u8], offset: usize, context: &str) -> Result<usize, String> {
    usize::try_from(i32_at(data, offset, context)?).map_err(|_| format!("{context} is negative"))
}

fn f32_at(data: &[u8], offset: usize, context: &str) -> Result<f32, String> {
    let value = f32::from_bits(u32_at(data, offset, context)?);
    if !value.is_finite() {
        return Err(format!("{context} is not finite"));
    }
    Ok(value)
}

fn vec3_at(data: &[u8], offset: usize, context: &str) -> Result<[f32; 3], String> {
    Ok([
        f32_at(data, offset, context)?,
        f32_at(data, offset + 4, context)?,
        f32_at(data, offset + 8, context)?,
    ])
}

fn vec4_at(data: &[u8], offset: usize, context: &str) -> Result<[f32; 4], String> {
    Ok([
        f32_at(data, offset, context)?,
        f32_at(data, offset + 4, context)?,
        f32_at(data, offset + 8, context)?,
        f32_at(data, offset + 12, context)?,
    ])
}

fn matrix3x4_at(data: &[u8], offset: usize, context: &str) -> Result<[f32; 12], String> {
    let mut output = [0.0; 12];
    for (index, value) in output.iter_mut().enumerate() {
        *value = f32_at(data, offset + index * 4, context)?;
    }
    Ok(output)
}

fn c_string(data: &[u8], offset: usize, context: &str) -> Result<String, String> {
    let remaining = data
        .get(offset..)
        .ok_or_else(|| format!("{context} string offset {offset} is out of bounds"))?;
    let length = remaining
        .iter()
        .take(MAX_STRING_BYTES + 1)
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("{context} string is unterminated or too long"))?;
    let value = std::str::from_utf8(&remaining[..length])
        .map_err(|error| format!("{context} string is not UTF-8: {error}"))?;
    Ok(value.replace('\\', "/"))
}

fn relative_string(
    data: &[u8],
    base: usize,
    relative: i32,
    context: &str,
) -> Result<String, String> {
    let offset = if relative >= 0 {
        base.checked_add(relative as usize)
    } else {
        base.checked_sub(relative.unsigned_abs() as usize)
    }
    .ok_or_else(|| format!("{context} relative string offset overflows"))?;
    c_string(data, offset, context)
}

fn sha256(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn normalized_model_path(value: &str) -> Result<String, String> {
    let path = value.trim().replace('\\', "/").to_ascii_lowercase();
    if !path.starts_with("models/")
        || !path.ends_with(".mdl")
        || path.contains('\0')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("invalid Source model path {value:?}"));
    }
    Ok(path)
}

fn material_candidates(search_paths: &[String], name: &str) -> Vec<String> {
    search_paths
        .iter()
        .map(|directory| {
            let directory = directory.trim_matches('/');
            if directory.is_empty() {
                format!("materials/{name}.vmt")
            } else {
                format!("materials/{directory}/{name}.vmt")
            }
        })
        .collect()
}

fn parse_mdl(data: &[u8]) -> Result<ParsedMdl, String> {
    if data.get(..4) != Some(b"IDST") {
        return Err("MDL signature is not IDST".to_owned());
    }
    let version = i32_at(data, 4, "MDL version")?;
    if !STUDIO_MODEL_MDL_VERSIONS.contains(&version) {
        return Err(format!(
            "unsupported MDL version {version}; TF2 contract accepts Source-compatible versions {STUDIO_MODEL_MDL_VERSIONS:?}"
        ));
    }
    checked_range(data, 0, MDL_HEADER_BYTES, "MDL header")?;
    let declared_length = usize_i32(data, 76, "MDL declared length")?;
    if declared_length < MDL_HEADER_BYTES || declared_length > data.len() {
        return Err(format!(
            "MDL declared length {declared_length} is outside {MDL_HEADER_BYTES}..={} bytes",
            data.len()
        ));
    }
    let data = &data[..declared_length];
    let checksum = i32_at(data, 8, "MDL checksum")?;
    let name = c_string(&data[12..76], 0, "MDL name")?;
    let flags = i32_at(data, 152, "MDL flags")?;
    let bounds = StudioBounds {
        eye_position: vec3_at(data, 80, "MDL eye position")?,
        illumination_position: vec3_at(data, 92, "MDL illumination position")?,
        hull_min: vec3_at(data, 104, "MDL hull minimum")?,
        hull_max: vec3_at(data, 116, "MDL hull maximum")?,
        view_min: vec3_at(data, 128, "MDL view minimum")?,
        view_max: vec3_at(data, 140, "MDL view maximum")?,
    };

    let bone_count = usize_i32(data, 156, "MDL bone count")?;
    let bone_offset = usize_i32(data, 160, "MDL bone offset")?;
    table_range(
        data,
        bone_offset,
        bone_count,
        MDL_BONE_BYTES,
        MAX_BONES,
        "MDL bones",
    )?;
    let mut bones = Vec::with_capacity(bone_count);
    for index in 0..bone_count {
        let offset = bone_offset + index * MDL_BONE_BYTES;
        let name_relative = i32_at(data, offset, "MDL bone name offset")?;
        let parent = i32_at(data, offset + 4, "MDL bone parent")?;
        if parent < -1 || parent >= index as i32 {
            return Err(format!("MDL bone {index} has invalid parent {parent}"));
        }
        let quaternion = vec4_at(data, offset + 44, "MDL bone quaternion")?;
        let quaternion_length = quaternion.iter().map(|value| value * value).sum::<f32>();
        if quaternion_length <= 1e-12 {
            return Err(format!("MDL bone {index} has a zero quaternion"));
        }
        bones.push(StudioBone {
            index,
            name: relative_string(data, offset, name_relative, "MDL bone name")?,
            parent,
            position: vec3_at(data, offset + 32, "MDL bone position")?,
            quaternion,
            rotation_euler: vec3_at(data, offset + 60, "MDL bone Euler rotation")?,
            position_scale: vec3_at(data, offset + 72, "MDL bone position scale")?,
            rotation_scale: vec3_at(data, offset + 84, "MDL bone rotation scale")?,
            pose_to_bone: matrix3x4_at(data, offset + 96, "MDL bone pose-to-bone")?,
            alignment: vec4_at(data, offset + 144, "MDL bone alignment")?,
            flags: i32_at(data, offset + 160, "MDL bone flags")?,
            contents: i32_at(data, offset + 180, "MDL bone contents")?,
        });
    }

    let animation_count = usize_i32(data, 180, "MDL animation count")?;
    let animation_offset = usize_i32(data, 184, "MDL animation offset")?;
    table_range(
        data,
        animation_offset,
        animation_count,
        MDL_ANIMATION_BYTES,
        MAX_ANIMATIONS,
        "MDL animations",
    )?;
    let mut animations = Vec::with_capacity(animation_count);
    for index in 0..animation_count {
        let offset = animation_offset + index * MDL_ANIMATION_BYTES;
        let animation_block = i32_at(data, offset + 52, "MDL animation block")?;
        let frame_count = usize_i32(data, offset + 16, "MDL animation frame count")?;
        if frame_count == 0 || frame_count > 1_000_000 {
            return Err(format!(
                "MDL animation {index} has invalid frame count {frame_count}"
            ));
        }
        let fps = f32_at(data, offset + 8, "MDL animation FPS")?;
        if fps <= 0.0 {
            return Err(format!("MDL animation {index} has non-positive FPS {fps}"));
        }
        let section_frame_count =
            usize_i32(data, offset + 84, "MDL animation section frame count")?;
        let mut sections = Vec::new();
        if section_frame_count > 0 {
            let section_count = frame_count
                .checked_div(section_frame_count)
                .and_then(|count| count.checked_add(2))
                .ok_or_else(|| format!("MDL animation {index} section count overflows"))?;
            let section_offset = relative_table_offset(
                offset,
                i32_at(data, offset + 80, "MDL animation section offset")?,
                "MDL animation section table",
            )?;
            table_range(
                data,
                section_offset,
                section_count,
                8,
                1_000_002,
                "MDL animation sections",
            )?;
            for section in 0..section_count {
                let section_offset = section_offset + section * 8;
                sections.push(MdlAnimationSection {
                    block: i32_at(data, section_offset, "MDL animation section block")?,
                    index: i32_at(data, section_offset + 4, "MDL animation section index")?,
                });
            }
        }
        animations.push(MdlAnimation {
            metadata: StudioAnimation {
                index,
                name: relative_string(
                    data,
                    offset,
                    i32_at(data, offset + 4, "MDL animation name offset")?,
                    "MDL animation name",
                )?,
                fps,
                flags: i32_at(data, offset + 12, "MDL animation flags")?,
                frame_count,
                animation_block,
                section_frame_count,
                ik_rule_count: usize_i32(data, offset + 60, "MDL animation IK rule count")?,
                local_hierarchy_count: usize_i32(
                    data,
                    offset + 72,
                    "MDL animation local hierarchy count",
                )?,
                zero_frame_count: if version < 47 {
                    0
                } else {
                    u16_at(data, offset + 90, "MDL animation zero frame count")? as usize
                },
                decode_status: StudioFeatureStatus::DetectedOnly,
                gltf_animation: None,
                sample_count: 0,
            },
            descriptor_offset: offset,
            animation_index: i32_at(data, offset + 56, "MDL animation data offset")?,
            sections,
        });
    }

    let sequence_count = usize_i32(data, 188, "MDL sequence count")?;
    let sequence_offset = usize_i32(data, 192, "MDL sequence offset")?;
    table_range(
        data,
        sequence_offset,
        sequence_count,
        MDL_SEQUENCE_BYTES,
        MAX_SEQUENCES,
        "MDL sequences",
    )?;
    let mut sequences = Vec::with_capacity(sequence_count);
    for index in 0..sequence_count {
        let offset = sequence_offset + index * MDL_SEQUENCE_BYTES;
        let group_x = usize_i32(data, offset + 68, "MDL sequence group X")?;
        let group_y = usize_i32(data, offset + 72, "MDL sequence group Y")?;
        let blend_count = group_x
            .checked_mul(group_y)
            .ok_or_else(|| format!("MDL sequence {index} blend count overflows"))?;
        if blend_count > MAX_ANIMATIONS {
            return Err(format!(
                "MDL sequence {index} blend count {blend_count} exceeds {MAX_ANIMATIONS}"
            ));
        }
        let animation_relative = i32_at(data, offset + 60, "MDL sequence animation offset")?;
        let animation_offset = if animation_relative >= 0 {
            offset.checked_add(animation_relative as usize)
        } else {
            offset.checked_sub(animation_relative.unsigned_abs() as usize)
        }
        .ok_or_else(|| format!("MDL sequence {index} animation table offset overflows"))?;
        table_range(
            data,
            animation_offset,
            blend_count,
            2,
            MAX_ANIMATIONS,
            "MDL sequence animation table",
        )?;
        let mut animation_indices = Vec::with_capacity(blend_count);
        for blend in 0..blend_count {
            let animation = i16_at(
                data,
                animation_offset + blend * 2,
                "MDL sequence animation index",
            )?;
            if animation < 0 || animation as usize >= animation_count {
                return Err(format!(
                    "MDL sequence {index} references invalid local animation {animation}"
                ));
            }
            animation_indices.push(animation);
        }
        let event_count = usize_i32(data, offset + 24, "MDL sequence event count")?;
        if event_count > 65_536 {
            return Err(format!("MDL sequence {index} event count is unbounded"));
        }
        let event_offset = usize_i32(data, offset + 28, "MDL sequence event offset")?;
        if event_count > 0 {
            table_range(
                data,
                offset
                    .checked_add(event_offset)
                    .ok_or_else(|| "MDL sequence event offset overflows".to_owned())?,
                event_count,
                80,
                65_536,
                "MDL sequence events",
            )?;
        }
        let auto_layer_count = usize_i32(data, offset + 148, "MDL sequence layer count")?;
        let auto_layer_offset = relative_table_offset(
            offset,
            i32_at(data, offset + 152, "MDL sequence layer offset")?,
            "MDL sequence layer table",
        )?;
        table_range(
            data,
            auto_layer_offset,
            auto_layer_count,
            24,
            MAX_SEQUENCES,
            "MDL sequence layers",
        )?;
        let auto_layers = (0..auto_layer_count)
            .map(|layer| {
                let layer_offset = auto_layer_offset + layer * 24;
                Ok(StudioSequenceLayer {
                    sequence: i16_at(data, layer_offset, "MDL sequence layer sequence")?,
                    pose: i16_at(data, layer_offset + 2, "MDL sequence layer pose")?,
                    flags: i32_at(data, layer_offset + 4, "MDL sequence layer flags")?,
                    start: f32_at(data, layer_offset + 8, "MDL sequence layer start")?,
                    peak: f32_at(data, layer_offset + 12, "MDL sequence layer peak")?,
                    tail: f32_at(data, layer_offset + 16, "MDL sequence layer tail")?,
                    end: f32_at(data, layer_offset + 20, "MDL sequence layer end")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let ik_lock_count = usize_i32(data, offset + 164, "MDL sequence IK lock count")?;
        let ik_lock_offset = relative_table_offset(
            offset,
            i32_at(data, offset + 168, "MDL sequence IK lock offset")?,
            "MDL sequence IK lock table",
        )?;
        table_range(
            data,
            ik_lock_offset,
            ik_lock_count,
            32,
            MAX_SEQUENCES,
            "MDL sequence IK locks",
        )?;
        let ik_locks = (0..ik_lock_count)
            .map(|lock| {
                let lock_offset = ik_lock_offset + lock * 32;
                Ok(StudioSequenceIkLock {
                    chain: i32_at(data, lock_offset, "MDL sequence IK lock chain")?,
                    position_weight: f32_at(
                        data,
                        lock_offset + 4,
                        "MDL sequence IK lock position weight",
                    )?,
                    local_rotation_weight: f32_at(
                        data,
                        lock_offset + 8,
                        "MDL sequence IK lock local rotation weight",
                    )?,
                    flags: i32_at(data, lock_offset + 12, "MDL sequence IK lock flags")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        sequences.push(StudioSequence {
            index,
            name: relative_string(
                data,
                offset,
                i32_at(data, offset + 4, "MDL sequence name offset")?,
                "MDL sequence name",
            )?,
            activity_name: relative_string(
                data,
                offset,
                i32_at(data, offset + 8, "MDL sequence activity name offset")?,
                "MDL sequence activity name",
            )?,
            flags: i32_at(data, offset + 12, "MDL sequence flags")?,
            activity: i32_at(data, offset + 16, "MDL sequence activity")?,
            activity_weight: i32_at(data, offset + 20, "MDL sequence activity weight")?,
            group_size: [group_x, group_y],
            animation_indices,
            event_count,
            auto_layers,
            ik_locks,
        });
    }

    let texture_count = usize_i32(data, 204, "MDL texture count")?;
    let texture_offset = usize_i32(data, 208, "MDL texture offset")?;
    table_range(
        data,
        texture_offset,
        texture_count,
        MDL_TEXTURE_BYTES,
        MAX_MATERIALS,
        "MDL textures",
    )?;
    let search_path_count = usize_i32(data, 212, "MDL material search path count")?;
    let search_path_offset = usize_i32(data, 216, "MDL material search path offset")?;
    table_range(
        data,
        search_path_offset,
        search_path_count,
        4,
        256,
        "MDL material search paths",
    )?;
    let mut search_paths = Vec::with_capacity(search_path_count);
    for index in 0..search_path_count {
        search_paths.push(
            c_string(
                data,
                usize_i32(
                    data,
                    search_path_offset + index * 4,
                    "MDL material search path string offset",
                )?,
                "MDL material search path",
            )?
            .trim_matches('/')
            .to_ascii_lowercase(),
        );
    }
    let mut materials = Vec::with_capacity(texture_count);
    for index in 0..texture_count {
        let offset = texture_offset + index * MDL_TEXTURE_BYTES;
        let material_name = relative_string(
            data,
            offset,
            i32_at(data, offset, "MDL texture name offset")?,
            "MDL texture name",
        )?
        .trim_end_matches(".vmt")
        .to_ascii_lowercase();
        materials.push(StudioMaterial {
            index,
            candidates: material_candidates(&search_paths, &material_name),
            name: material_name,
            search_paths: search_paths.clone(),
        });
    }

    let skin_reference_count = usize_i32(data, 220, "MDL skin reference count")?;
    let skin_family_count = usize_i32(data, 224, "MDL skin family count")?;
    let skin_offset = usize_i32(data, 228, "MDL skin offset")?;
    if skin_family_count > MAX_SKIN_FAMILIES || skin_reference_count > MAX_MATERIALS {
        return Err("MDL skin table exceeds supported hard limits".to_owned());
    }
    let skin_entries = skin_reference_count
        .checked_mul(skin_family_count)
        .ok_or_else(|| "MDL skin table count overflows".to_owned())?;
    table_range(
        data,
        skin_offset,
        skin_entries,
        2,
        MAX_SKIN_FAMILIES * MAX_MATERIALS,
        "MDL skin table",
    )?;
    let mut skins = Vec::with_capacity(skin_family_count);
    for family in 0..skin_family_count {
        let mut texture_indices = Vec::with_capacity(skin_reference_count);
        for slot in 0..skin_reference_count {
            let texture = i16_at(
                data,
                skin_offset + (family * skin_reference_count + slot) * 2,
                "MDL skin texture index",
            )?;
            if texture < 0 || texture as usize >= texture_count {
                return Err(format!(
                    "MDL skin family {family} slot {slot} references invalid texture {texture}"
                ));
            }
            texture_indices.push(texture as usize);
        }
        skins.push(StudioSkinFamily {
            index: family,
            texture_indices,
        });
    }

    let flex_descriptor_count = usize_i32(data, 260, "MDL flex descriptor count")?;
    let flex_descriptor_offset = usize_i32(data, 264, "MDL flex descriptor offset")?;
    table_range(
        data,
        flex_descriptor_offset,
        flex_descriptor_count,
        MDL_FLEX_DESCRIPTOR_BYTES,
        MAX_FLEXES,
        "MDL flex descriptors",
    )?;
    let mut flex_descriptors = Vec::with_capacity(flex_descriptor_count);
    for index in 0..flex_descriptor_count {
        let offset = flex_descriptor_offset + index * MDL_FLEX_DESCRIPTOR_BYTES;
        flex_descriptors.push(StudioFlexDescriptor {
            index,
            name: relative_string(
                data,
                offset,
                i32_at(data, offset, "MDL flex descriptor name offset")?,
                "MDL flex descriptor name",
            )?,
        });
    }

    let flex_controller_count = usize_i32(data, 268, "MDL flex controller count")?;
    let flex_controller_offset = usize_i32(data, 272, "MDL flex controller offset")?;
    table_range(
        data,
        flex_controller_offset,
        flex_controller_count,
        MDL_FLEX_CONTROLLER_BYTES,
        MAX_FLEXES,
        "MDL flex controllers",
    )?;
    let mut flex_controllers = Vec::with_capacity(flex_controller_count);
    for index in 0..flex_controller_count {
        let offset = flex_controller_offset + index * MDL_FLEX_CONTROLLER_BYTES;
        flex_controllers.push(StudioFlexController {
            index,
            controller_type: relative_string(
                data,
                offset,
                i32_at(data, offset, "MDL flex controller type offset")?,
                "MDL flex controller type",
            )?,
            name: relative_string(
                data,
                offset,
                i32_at(data, offset + 4, "MDL flex controller name offset")?,
                "MDL flex controller name",
            )?,
            minimum: f32_at(data, offset + 12, "MDL flex controller minimum")?,
            maximum: f32_at(data, offset + 16, "MDL flex controller maximum")?,
        });
    }

    let flex_rule_count = usize_i32(data, 276, "MDL flex rule count")?;
    let flex_rule_offset = usize_i32(data, 280, "MDL flex rule offset")?;
    table_range(
        data,
        flex_rule_offset,
        flex_rule_count,
        MDL_FLEX_RULE_BYTES,
        MAX_FLEXES,
        "MDL flex rules",
    )?;
    let mut flex_rules = Vec::with_capacity(flex_rule_count);
    let mut total_flex_operations = 0_usize;
    for index in 0..flex_rule_count {
        let offset = flex_rule_offset + index * MDL_FLEX_RULE_BYTES;
        let operation_count = usize_i32(data, offset + 4, "MDL flex operation count")?;
        total_flex_operations = total_flex_operations
            .checked_add(operation_count)
            .ok_or_else(|| "MDL flex operation count overflows".to_owned())?;
        if total_flex_operations > MAX_FLEX_OPERATIONS {
            return Err(format!(
                "MDL flex operation count exceeds {MAX_FLEX_OPERATIONS}"
            ));
        }
        let operation_offset = relative_table_offset(
            offset,
            i32_at(data, offset + 8, "MDL flex operation offset")?,
            "MDL flex operation table",
        )?;
        table_range(
            data,
            operation_offset,
            operation_count,
            MDL_FLEX_OPERATION_BYTES,
            MAX_FLEX_OPERATIONS,
            "MDL flex operations",
        )?;
        let operations = (0..operation_count)
            .map(|operation| {
                let operation_offset = operation_offset + operation * MDL_FLEX_OPERATION_BYTES;
                Ok(StudioFlexOperation {
                    operation: i32_at(data, operation_offset, "MDL flex operation")?,
                    operand_bits: u32_at(data, operation_offset + 4, "MDL flex operand")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        flex_rules.push(StudioFlexRule {
            index,
            flex_descriptor: i32_at(data, offset, "MDL flex rule descriptor")?,
            operations,
        });
    }

    let flex_scale = if flags & 0x0020_0000 != 0 {
        f32_at(data, 392, "MDL flex fixed-point scale")?
    } else {
        1.0 / 4096.0
    };
    if flex_scale <= 0.0 {
        return Err("MDL flex fixed-point scale is not positive".to_owned());
    }

    let body_part_count = usize_i32(data, 232, "MDL body part count")?;
    let body_part_offset = usize_i32(data, 236, "MDL body part offset")?;
    table_range(
        data,
        body_part_offset,
        body_part_count,
        MDL_BODYPART_BYTES,
        MAX_BODY_PARTS,
        "MDL body parts",
    )?;
    let mut body_parts = Vec::with_capacity(body_part_count);
    let mut total_models = 0_usize;
    let mut total_meshes = 0_usize;
    let mut mesh_flex_count = 0_usize;
    for body_part_index in 0..body_part_count {
        let offset = body_part_offset + body_part_index * MDL_BODYPART_BYTES;
        let model_count = usize_i32(data, offset + 4, "MDL body part model count")?;
        total_models = total_models
            .checked_add(model_count)
            .ok_or_else(|| "MDL model count overflows".to_owned())?;
        if total_models > MAX_MODELS {
            return Err(format!("MDL model count exceeds {MAX_MODELS}"));
        }
        let model_relative = usize_i32(data, offset + 12, "MDL body part model offset")?;
        let model_offset = offset
            .checked_add(model_relative)
            .ok_or_else(|| "MDL body part model offset overflows".to_owned())?;
        table_range(
            data,
            model_offset,
            model_count,
            MDL_MODEL_BYTES,
            MAX_MODELS,
            "MDL models",
        )?;
        let mut models = Vec::with_capacity(model_count);
        for model_index in 0..model_count {
            let model_offset = model_offset + model_index * MDL_MODEL_BYTES;
            let mesh_count = usize_i32(data, model_offset + 72, "MDL model mesh count")?;
            total_meshes = total_meshes
                .checked_add(mesh_count)
                .ok_or_else(|| "MDL mesh count overflows".to_owned())?;
            if total_meshes > MAX_MESHES {
                return Err(format!("MDL mesh count exceeds {MAX_MESHES}"));
            }
            let mesh_relative = usize_i32(data, model_offset + 76, "MDL model mesh offset")?;
            let mesh_offset = model_offset
                .checked_add(mesh_relative)
                .ok_or_else(|| "MDL model mesh offset overflows".to_owned())?;
            table_range(
                data,
                mesh_offset,
                mesh_count,
                MDL_MESH_BYTES,
                MAX_MESHES,
                "MDL meshes",
            )?;
            let vertex_count = usize_i32(data, model_offset + 80, "MDL model vertex count")?;
            if vertex_count > MAX_VERTICES {
                return Err(format!("MDL model vertex count exceeds {MAX_VERTICES}"));
            }
            let vertex_bytes = usize_i32(data, model_offset + 84, "MDL model vertex offset")?;
            if !vertex_bytes.is_multiple_of(VVD_VERTEX_BYTES) {
                return Err(format!(
                    "MDL body part {body_part_index} model {model_index} vertex offset is not vertex-aligned"
                ));
            }
            let mut meshes = Vec::with_capacity(mesh_count);
            for mesh_index in 0..mesh_count {
                let mesh_offset = mesh_offset + mesh_index * MDL_MESH_BYTES;
                let material = i32_at(data, mesh_offset, "MDL mesh material")?;
                if material < 0 || material as usize >= skin_reference_count {
                    return Err(format!(
                        "MDL mesh {mesh_index} material slot {material} is invalid"
                    ));
                }
                let flex_count = usize_i32(data, mesh_offset + 16, "MDL mesh flex count")?;
                mesh_flex_count = mesh_flex_count
                    .checked_add(flex_count)
                    .ok_or_else(|| "MDL mesh flex count overflows".to_owned())?;
                if mesh_flex_count > MAX_FLEXES {
                    return Err(format!("MDL mesh flex count exceeds {MAX_FLEXES}"));
                }
                let flex_offset = relative_table_offset(
                    mesh_offset,
                    i32_at(data, mesh_offset + 20, "MDL mesh flex offset")?,
                    "MDL mesh flex table",
                )?;
                table_range(
                    data,
                    flex_offset,
                    flex_count,
                    MDL_FLEX_BYTES,
                    MAX_FLEXES,
                    "MDL mesh flexes",
                )?;
                let mut flexes = Vec::with_capacity(flex_count);
                for flex_index in 0..flex_count {
                    let flex_offset = flex_offset + flex_index * MDL_FLEX_BYTES;
                    let descriptor = usize_i32(data, flex_offset, "MDL mesh flex descriptor")?;
                    if descriptor >= flex_descriptor_count {
                        return Err(format!(
                            "MDL mesh flex {flex_index} references missing descriptor {descriptor}"
                        ));
                    }
                    let vertex_count =
                        usize_i32(data, flex_offset + 20, "MDL mesh flex vertex count")?;
                    let animation_type =
                        *checked_range(data, flex_offset + 32, 1, "MDL mesh flex animation type")?
                            .first()
                            .unwrap();
                    let vertex_stride = match animation_type {
                        0 => 16,
                        1 => 18,
                        _ => {
                            return Err(format!(
                                "MDL mesh flex {flex_index} has unknown vertex animation type {animation_type}"
                            ));
                        }
                    };
                    let vertex_offset = relative_table_offset(
                        flex_offset,
                        i32_at(data, flex_offset + 24, "MDL mesh flex vertex offset")?,
                        "MDL mesh flex vertex table",
                    )?;
                    table_range(
                        data,
                        vertex_offset,
                        vertex_count,
                        vertex_stride,
                        MAX_FLEX_VERTICES,
                        "MDL mesh flex vertices",
                    )?;
                    let mut vertices = BTreeMap::new();
                    for vertex in 0..vertex_count {
                        let offset = vertex_offset + vertex * vertex_stride;
                        let mesh_vertex = u16_at(data, offset, "MDL flex mesh vertex")? as usize;
                        if mesh_vertex >= usize_i32(data, mesh_offset + 8, "MDL mesh vertex count")?
                        {
                            return Err(format!(
                                "MDL mesh flex {flex_index} vertex {mesh_vertex} exceeds its mesh"
                            ));
                        }
                        let scaled = |offset: usize| -> Result<f32, String> {
                            Ok(i16_at(data, offset, "MDL flex delta")? as f32 * flex_scale)
                        };
                        let record = MdlFlexVertex {
                            position_delta: [
                                scaled(offset + 4)?,
                                scaled(offset + 6)?,
                                scaled(offset + 8)?,
                            ],
                            normal_delta: [
                                scaled(offset + 10)?,
                                scaled(offset + 12)?,
                                scaled(offset + 14)?,
                            ],
                            wrinkle_delta: (animation_type == 1)
                                .then(|| scaled(offset + 16))
                                .transpose()?,
                        };
                        if vertices.insert(mesh_vertex, record).is_some() {
                            return Err(format!(
                                "MDL mesh flex {flex_index} repeats vertex {mesh_vertex}"
                            ));
                        }
                    }
                    flexes.push(MdlFlex {
                        descriptor,
                        thresholds: [
                            f32_at(data, flex_offset + 4, "MDL flex threshold")?,
                            f32_at(data, flex_offset + 8, "MDL flex threshold")?,
                            f32_at(data, flex_offset + 12, "MDL flex threshold")?,
                            f32_at(data, flex_offset + 16, "MDL flex threshold")?,
                        ],
                        pair: i32_at(data, flex_offset + 28, "MDL flex pair")?,
                        animation_type,
                        vertices,
                    });
                }
                meshes.push(MdlMesh {
                    material,
                    vertex_count: usize_i32(data, mesh_offset + 8, "MDL mesh vertex count")?,
                    vertex_offset: usize_i32(data, mesh_offset + 12, "MDL mesh vertex offset")?,
                    flexes,
                });
            }
            models.push(MdlModel {
                name: c_string(
                    checked_range(data, model_offset, 64, "MDL model name")?,
                    0,
                    "MDL model name",
                )?,
                vertex_count,
                vertex_start: vertex_bytes / VVD_VERTEX_BYTES,
                meshes,
            });
        }
        body_parts.push(ParsedBodyPart {
            name: relative_string(
                data,
                offset,
                i32_at(data, offset, "MDL body part name offset")?,
                "MDL body part name",
            )?,
            base: i32_at(data, offset + 8, "MDL body part base")?,
            models,
        });
    }

    let attachment_count = usize_i32(data, 240, "MDL attachment count")?;
    let attachment_offset = usize_i32(data, 244, "MDL attachment offset")?;
    table_range(
        data,
        attachment_offset,
        attachment_count,
        MDL_ATTACHMENT_BYTES,
        MAX_ATTACHMENTS,
        "MDL attachments",
    )?;
    let mut attachments = Vec::with_capacity(attachment_count);
    for index in 0..attachment_count {
        let offset = attachment_offset + index * MDL_ATTACHMENT_BYTES;
        let bone = usize_i32(data, offset + 8, "MDL attachment bone")?;
        if bone >= bone_count {
            return Err(format!(
                "MDL attachment {index} references missing bone {bone}"
            ));
        }
        attachments.push(StudioAttachment {
            index,
            name: relative_string(
                data,
                offset,
                i32_at(data, offset, "MDL attachment name offset")?,
                "MDL attachment name",
            )?,
            flags: u32_at(data, offset + 4, "MDL attachment flags")?,
            bone,
            local: matrix3x4_at(data, offset + 12, "MDL attachment transform")?,
            gltf_node: None,
        });
    }

    let include_count = usize_i32(data, 336, "MDL include model count")?;
    let include_offset = usize_i32(data, 340, "MDL include model offset")?;
    table_range(
        data,
        include_offset,
        include_count,
        8,
        MAX_MODELS,
        "MDL include models",
    )?;
    let mut include_models = Vec::with_capacity(include_count);
    for index in 0..include_count {
        let offset = include_offset + index * 8;
        include_models.push(normalized_model_path(&relative_string(
            data,
            offset,
            i32_at(data, offset + 4, "MDL include model name offset")?,
            "MDL include model name",
        )?)?);
    }

    let animation_block_count = usize_i32(data, 352, "MDL animation block count")?;
    let animation_block_offset = usize_i32(data, 356, "MDL animation block offset")?;
    table_range(
        data,
        animation_block_offset,
        animation_block_count,
        8,
        MAX_ANIMATIONS,
        "MDL animation blocks",
    )?;
    let mut animation_blocks = Vec::with_capacity(animation_block_count);
    for block in 0..animation_block_count {
        let offset = animation_block_offset + block * 8;
        let start = usize_i32(data, offset, "MDL animation block start")?;
        let end = usize_i32(data, offset + 4, "MDL animation block end")?;
        if end < start {
            return Err(format!("MDL animation block {block} has a reversed range"));
        }
        animation_blocks.push((start, end));
    }

    Ok(ParsedMdl {
        checksum,
        version,
        name,
        flags,
        bounds,
        bones,
        animations,
        sequences,
        attachments,
        materials,
        skins,
        body_parts,
        include_models,
        flex_descriptors,
        flex_controllers,
        flex_rules,
        mesh_flex_count,
        animation_block_count,
        animation_blocks,
    })
}

struct DecodedAnimation {
    translations: Vec<Vec<[f32; 3]>>,
    rotations: Vec<Vec<[f32; 4]>>,
}

type DecodedBoneFrame = (Vec<[f32; 3]>, Vec<[f32; 4]>);

fn compressed_quaternion(
    data: &[u8],
    offset: usize,
    wide: bool,
    context: &str,
) -> Result<([f32; 4], usize), String> {
    let (x, y, z, negative_w, bytes) = if wide {
        let low = u32_at(data, offset, context)?;
        let high = u32_at(data, offset + 4, context)?;
        (
            (low & 0x1f_ffff) as f32 / 1_048_576.5 - 1_048_576.0 / 1_048_576.5,
            ((((high & 0x03ff) << 11) | (low >> 21)) & 0x1f_ffff) as f32 / 1_048_576.5
                - 1_048_576.0 / 1_048_576.5,
            ((high >> 10) & 0x1f_ffff) as f32 / 1_048_576.5 - 1_048_576.0 / 1_048_576.5,
            high & 0x8000_0000 != 0,
            8,
        )
    } else {
        let x = u16_at(data, offset, context)?;
        let y = u16_at(data, offset + 2, context)?;
        let z = u16_at(data, offset + 4, context)?;
        (
            (x as i32 - 32_768) as f32 / 32_768.0,
            (y as i32 - 32_768) as f32 / 32_768.0,
            ((z & 0x7fff) as i32 - 16_384) as f32 / 16_384.0,
            z & 0x8000 != 0,
            6,
        )
    };
    let squared = x * x + y * y + z * z;
    if squared > 1.001 {
        return Err(format!("{context} has invalid XYZ magnitude {squared}"));
    }
    let w = (1.0 - squared).max(0.0).sqrt() * if negative_w { -1.0 } else { 1.0 };
    Ok(([x, y, z, w], bytes))
}

fn compressed_vector48(data: &[u8], offset: usize, context: &str) -> Result<[f32; 3], String> {
    checked_range(data, offset, 6, context)?;
    let output = [
        half::f16::from_bits(u16_at(data, offset, context)?).to_f32(),
        half::f16::from_bits(u16_at(data, offset + 2, context)?).to_f32(),
        half::f16::from_bits(u16_at(data, offset + 4, context)?).to_f32(),
    ];
    if output.iter().any(|value| !value.is_finite()) {
        return Err(format!("{context} contains a non-finite component"));
    }
    Ok(output)
}

fn euler_quaternion(rotation: [f32; 3]) -> [f32; 4] {
    let [roll, pitch, yaw] = rotation.map(|value| value * 0.5);
    let (sin_roll, cos_roll) = roll.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    [
        sin_roll * cos_pitch * cos_yaw - cos_roll * sin_pitch * sin_yaw,
        cos_roll * sin_pitch * cos_yaw + sin_roll * cos_pitch * sin_yaw,
        cos_roll * cos_pitch * sin_yaw - sin_roll * sin_pitch * cos_yaw,
        cos_roll * cos_pitch * cos_yaw + sin_roll * sin_pitch * sin_yaw,
    ]
}

fn animation_value(
    data: &[u8],
    table_offset: usize,
    relative: i16,
    frame: usize,
    scale: f32,
    context: &str,
) -> Result<f32, String> {
    if relative == 0 {
        return Ok(0.0);
    }
    let mut offset = if relative > 0 {
        table_offset.checked_add(relative as usize)
    } else {
        table_offset.checked_sub(relative.unsigned_abs() as usize)
    }
    .ok_or_else(|| format!("{context} value offset overflows"))?;
    let mut remaining = frame;
    for _ in 0..=frame {
        let counts = checked_range(data, offset, 2, context)?;
        let valid = counts[0] as usize;
        let total = counts[1] as usize;
        if total == 0 || valid == 0 || valid > total {
            return Err(format!(
                "{context} has invalid RLE counts valid={valid} total={total}"
            ));
        }
        checked_range(data, offset + 2, valid * 2, context)?;
        if remaining < total {
            let sample = remaining.min(valid - 1);
            return Ok(i16_at(data, offset + 2 + sample * 2, context)? as f32 * scale);
        }
        remaining -= total;
        offset = offset
            .checked_add(2 + valid * 2)
            .ok_or_else(|| format!("{context} RLE offset overflows"))?;
    }
    Err(format!("{context} RLE stream does not cover frame {frame}"))
}

fn animation_section<'a>(
    mdl_data: &'a [u8],
    ani_data: Option<&'a [u8]>,
    mdl: &ParsedMdl,
    animation: &MdlAnimation,
    frame: usize,
) -> Result<(&'a [u8], usize, usize), String> {
    let (section, local_frame) = if animation.metadata.section_frame_count == 0 {
        (None, frame)
    } else if animation.metadata.frame_count > animation.metadata.section_frame_count
        && frame == animation.metadata.frame_count - 1
    {
        (Some(frame / animation.metadata.section_frame_count + 1), 0)
    } else {
        let section = frame / animation.metadata.section_frame_count;
        (
            Some(section),
            frame - section * animation.metadata.section_frame_count,
        )
    };
    let (block, index) = if let Some(section) = section {
        let section = animation.sections.get(section).ok_or_else(|| {
            format!(
                "animation {} frame {frame} references missing section {section}",
                animation.metadata.index
            )
        })?;
        (section.block, section.index)
    } else {
        (
            animation.metadata.animation_block,
            animation.animation_index,
        )
    };
    if block == -1 {
        return Err(format!(
            "animation {} references an invalid recompile-required block",
            animation.metadata.index
        ));
    }
    let relative = usize::try_from(index).map_err(|_| {
        format!(
            "animation {} has a negative data offset {index}",
            animation.metadata.index
        )
    })?;
    if block == 0 {
        let offset = animation
            .descriptor_offset
            .checked_add(relative)
            .ok_or_else(|| "inline animation offset overflows".to_owned())?;
        checked_range(mdl_data, offset, 4, "inline animation data")?;
        return Ok((mdl_data, offset, local_frame));
    }
    let block_index =
        usize::try_from(block).map_err(|_| format!("animation block {block} is negative"))?;
    let (start, end) = *mdl
        .animation_blocks
        .get(block_index)
        .ok_or_else(|| format!("animation block {block} is missing"))?;
    let ani = ani_data.ok_or_else(|| format!("animation block {block} requires ANI data"))?;
    if end > ani.len() {
        return Err(format!("animation block {block} exceeds ANI data"));
    }
    let offset = start
        .checked_add(relative)
        .ok_or_else(|| "external animation offset overflows".to_owned())?;
    if offset < start || offset + 4 > end {
        return Err(format!(
            "animation block {block} data offset is out of range"
        ));
    }
    Ok((ani, offset, local_frame))
}

fn decode_animation_frame(
    data: &[u8],
    offset: usize,
    frame: usize,
    bones: &[StudioBone],
    animation_index: usize,
    animation_flags: i32,
) -> Result<DecodedBoneFrame, String> {
    let descriptor_delta = animation_flags & 0x0004 != 0;
    let mut translations: Vec<_> = bones
        .iter()
        .map(|bone| {
            if descriptor_delta {
                [0.0; 3]
            } else {
                bone.position
            }
        })
        .collect();
    let mut rotations: Vec<_> = bones
        .iter()
        .map(|bone| {
            if descriptor_delta {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                bone.quaternion
            }
        })
        .collect();
    let mut cursor = offset;
    let mut visited = BTreeSet::new();
    for _ in 0..=bones.len() {
        let header = checked_range(data, cursor, 4, "animation bone track")?;
        let bone_index = header[0] as usize;
        let flags = header[1];
        let next = u16::from_le_bytes([header[2], header[3]]) as usize;
        if bone_index == 255 {
            return Ok((translations, rotations));
        }
        let bone = bones.get(bone_index).ok_or_else(|| {
            format!("animation {animation_index} references missing bone {bone_index}")
        })?;
        if !visited.insert(bone_index) {
            return Err(format!(
                "animation {animation_index} repeats bone track {bone_index}"
            ));
        }
        let mut payload = cursor + 4;
        let delta = flags & 0x10 != 0;
        let mut rotation = if flags & 0x02 != 0 {
            let (value, bytes) =
                compressed_quaternion(data, payload, false, "animation Quaternion48")?;
            payload += bytes;
            value
        } else if flags & 0x20 != 0 {
            let (value, bytes) =
                compressed_quaternion(data, payload, true, "animation Quaternion64")?;
            payload += bytes;
            value
        } else if flags & 0x08 != 0 {
            let table = payload;
            checked_range(data, table, 6, "animation rotation value table")?;
            let mut euler = [0.0; 3];
            for (axis, value) in euler.iter_mut().enumerate() {
                *value = animation_value(
                    data,
                    table,
                    i16_at(data, table + axis * 2, "animation rotation value offset")?,
                    frame,
                    bone.rotation_scale[axis],
                    "animation rotation values",
                )?;
                if !delta {
                    *value += bone.rotation_euler[axis];
                }
            }
            payload += 6;
            euler_quaternion(euler)
        } else if delta {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            bone.quaternion
        };
        if !delta && bone.flags & 0x0010_0000 != 0 {
            let dot = rotation
                .iter()
                .zip(bone.alignment)
                .map(|(left, right)| left * right)
                .sum::<f32>();
            if dot < 0.0 {
                rotation.iter_mut().for_each(|value| *value = -*value);
            }
        }
        rotations[bone_index] = rotation;

        translations[bone_index] = if flags & 0x01 != 0 {
            compressed_vector48(data, payload, "animation Vector48")?
        } else if flags & 0x04 != 0 {
            let table = payload;
            checked_range(data, table, 6, "animation position value table")?;
            let mut position = [0.0; 3];
            for (axis, value) in position.iter_mut().enumerate() {
                *value = animation_value(
                    data,
                    table,
                    i16_at(data, table + axis * 2, "animation position value offset")?,
                    frame,
                    bone.position_scale[axis],
                    "animation position values",
                )?;
                if !delta {
                    *value += bone.position[axis];
                }
            }
            position
        } else if delta {
            [0.0; 3]
        } else {
            bone.position
        };
        if next == 0 {
            return Ok((translations, rotations));
        }
        cursor = cursor
            .checked_add(next)
            .ok_or_else(|| "animation track offset overflows".to_owned())?;
    }
    Err(format!(
        "animation {animation_index} has more tracks than bones"
    ))
}

fn decode_animations(
    mdl_data: &[u8],
    ani_data: Option<&[u8]>,
    mdl: &ParsedMdl,
) -> Result<Vec<DecodedAnimation>, String> {
    mdl.animations
        .iter()
        .map(|animation| {
            let mut translations = Vec::with_capacity(animation.metadata.frame_count);
            let mut rotations = Vec::with_capacity(animation.metadata.frame_count);
            for frame in 0..animation.metadata.frame_count {
                let (data, offset, local_frame) =
                    animation_section(mdl_data, ani_data, mdl, animation, frame)?;
                let (frame_translations, frame_rotations) = decode_animation_frame(
                    data,
                    offset,
                    local_frame,
                    &mdl.bones,
                    animation.metadata.index,
                    animation.metadata.flags,
                )?;
                translations.push(frame_translations);
                rotations.push(frame_rotations);
            }
            Ok(DecodedAnimation {
                translations,
                rotations,
            })
        })
        .collect()
}

fn parse_vvd(data: &[u8], expected_checksum: i32) -> Result<ParsedVvd, String> {
    checked_range(data, 0, VVD_HEADER_BYTES, "VVD header")?;
    if data.get(..4) != Some(b"IDSV") {
        return Err("VVD signature is not IDSV".to_owned());
    }
    let version = i32_at(data, 4, "VVD version")?;
    if version != STUDIO_MODEL_VVD_VERSION {
        return Err(format!(
            "unsupported VVD version {version}; TF2 contract requires {STUDIO_MODEL_VVD_VERSION}"
        ));
    }
    let checksum = i32_at(data, 8, "VVD checksum")?;
    if checksum != expected_checksum {
        return Err(format!(
            "VVD checksum {checksum:#010x} does not match MDL checksum {expected_checksum:#010x}"
        ));
    }
    let lod_count = usize_i32(data, 12, "VVD LOD count")?;
    if lod_count == 0 || lod_count > MAX_LODS {
        return Err(format!(
            "VVD LOD count {lod_count} is outside 1..={MAX_LODS}"
        ));
    }
    let mut lod_vertex_counts = Vec::with_capacity(lod_count);
    for lod in 0..lod_count {
        let count = usize_i32(data, 16 + lod * 4, "VVD LOD vertex count")?;
        if count > MAX_VERTICES {
            return Err(format!("VVD LOD {lod} vertex count exceeds {MAX_VERTICES}"));
        }
        if lod > 0 && count > lod_vertex_counts[lod - 1] {
            return Err(format!(
                "VVD LOD {lod} has more vertices than the preceding LOD"
            ));
        }
        lod_vertex_counts.push(count);
    }
    let fixup_count = usize_i32(data, 48, "VVD fixup count")?;
    let fixup_offset = usize_i32(data, 52, "VVD fixup offset")?;
    let vertex_offset = usize_i32(data, 56, "VVD vertex data offset")?;
    let tangent_offset = usize_i32(data, 60, "VVD tangent data offset")?;
    table_range(
        data,
        fixup_offset,
        fixup_count,
        VVD_FIXUP_BYTES,
        MAX_VERTICES,
        "VVD fixups",
    )?;
    let source_vertex_count = lod_vertex_counts[0];
    table_range(
        data,
        vertex_offset,
        source_vertex_count,
        VVD_VERTEX_BYTES,
        MAX_VERTICES,
        "VVD vertices",
    )?;
    table_range(
        data,
        tangent_offset,
        source_vertex_count,
        VVD_TANGENT_BYTES,
        MAX_VERTICES,
        "VVD tangents",
    )?;
    let mut source_vertices = Vec::with_capacity(source_vertex_count);
    for index in 0..source_vertex_count {
        let offset = vertex_offset + index * VVD_VERTEX_BYTES;
        let tangent_offset = tangent_offset + index * VVD_TANGENT_BYTES;
        let bone_count = *checked_range(data, offset + 15, 1, "VVD vertex bone count")?
            .first()
            .unwrap();
        if bone_count > 3 {
            return Err(format!(
                "VVD vertex {index} has invalid bone count {bone_count}"
            ));
        }
        let weights = [
            f32_at(data, offset, "VVD vertex weight")?,
            f32_at(data, offset + 4, "VVD vertex weight")?,
            f32_at(data, offset + 8, "VVD vertex weight")?,
        ];
        if weights.iter().any(|value| *value < 0.0 || *value > 1.0001) {
            return Err(format!("VVD vertex {index} has an invalid bone weight"));
        }
        source_vertices.push(VvdVertex {
            weights,
            bones: checked_range(data, offset + 12, 3, "VVD vertex bones")?
                .try_into()
                .unwrap(),
            bone_count,
            position: vec3_at(data, offset + 16, "VVD vertex position")?,
            normal: vec3_at(data, offset + 28, "VVD vertex normal")?,
            uv: [
                f32_at(data, offset + 40, "VVD vertex UV")?,
                f32_at(data, offset + 44, "VVD vertex UV")?,
            ],
            tangent: vec4_at(data, tangent_offset, "VVD vertex tangent")?,
        });
    }
    let mut fixups = Vec::with_capacity(fixup_count);
    let mut destination = 0_usize;
    for index in 0..fixup_count {
        let offset = fixup_offset + index * VVD_FIXUP_BYTES;
        let lod = usize_i32(data, offset, "VVD fixup LOD")?;
        let source = usize_i32(data, offset + 4, "VVD fixup source vertex")?;
        let count = usize_i32(data, offset + 8, "VVD fixup vertex count")?;
        let end = source
            .checked_add(count)
            .ok_or_else(|| "VVD fixup source range overflows".to_owned())?;
        if lod >= lod_count || end > source_vertex_count {
            return Err(format!(
                "VVD fixup {index} has an invalid LOD or source range"
            ));
        }
        fixups.push(VvdFixup {
            lod,
            source,
            destination,
            count,
        });
        destination = destination
            .checked_add(count)
            .ok_or_else(|| "VVD fixup destination range overflows".to_owned())?;
    }
    for (lod, expected_count) in lod_vertex_counts.iter().copied().enumerate() {
        let produced = if fixups.is_empty() {
            expected_count
        } else {
            fixups
                .iter()
                .filter(|fixup| fixup.lod >= lod)
                .map(|fixup| fixup.count)
                .sum()
        };
        if produced != expected_count {
            return Err(format!(
                "VVD fixups produce {produced} vertices for LOD {lod}, expected {expected_count}"
            ));
        }
    }
    Ok(ParsedVvd {
        checksum,
        lod_vertex_counts,
        source_vertices,
        fixups,
    })
}

fn relative_table_offset(base: usize, relative: i32, context: &str) -> Result<usize, String> {
    if relative >= 0 {
        base.checked_add(relative as usize)
    } else {
        base.checked_sub(relative.unsigned_abs() as usize)
    }
    .ok_or_else(|| format!("{context} relative offset overflows"))
}

fn parse_vtx(data: &[u8], expected_checksum: i32) -> Result<ParsedVtx, String> {
    checked_range(data, 0, VTX_HEADER_BYTES, "VTX header")?;
    let version = i32_at(data, 0, "VTX version")?;
    if version != STUDIO_MODEL_VTX_VERSION {
        return Err(format!(
            "unsupported VTX version {version}; TF2 contract requires {STUDIO_MODEL_VTX_VERSION}"
        ));
    }
    let max_bones_per_vertex = i32_at(data, 12, "VTX maximum bones per vertex")?;
    if !(1..=3).contains(&max_bones_per_vertex) {
        return Err(format!(
            "VTX maximum bones per vertex {max_bones_per_vertex} is outside 1..=3"
        ));
    }
    let checksum = i32_at(data, 16, "VTX checksum")?;
    if checksum != expected_checksum {
        return Err(format!(
            "VTX checksum {checksum:#010x} does not match MDL checksum {expected_checksum:#010x}"
        ));
    }
    let lod_count = usize_i32(data, 20, "VTX LOD count")?;
    if lod_count == 0 || lod_count > MAX_LODS {
        return Err(format!(
            "VTX LOD count {lod_count} is outside 1..={MAX_LODS}"
        ));
    }
    let material_replacement_offset = usize_i32(data, 24, "VTX material replacement list offset")?;
    table_range(
        data,
        material_replacement_offset,
        lod_count,
        8,
        MAX_LODS,
        "VTX material replacement lists",
    )?;
    let mut material_replacements = Vec::with_capacity(lod_count);
    for lod in 0..lod_count {
        let offset = material_replacement_offset + lod * 8;
        let count = usize_i32(data, offset, "VTX material replacement count")?;
        let table = relative_table_offset(
            offset,
            i32_at(data, offset + 4, "VTX material replacement offset")?,
            "VTX material replacement table",
        )?;
        table_range(
            data,
            table,
            count,
            6,
            MAX_MATERIALS,
            "VTX material replacements",
        )?;
        let mut replacements = BTreeMap::new();
        for index in 0..count {
            let entry = table + index * 6;
            let material = i16_at(data, entry, "VTX replacement material ID")?;
            if material < 0 {
                return Err(format!(
                    "VTX LOD {lod} has a negative replacement material ID"
                ));
            }
            let name = relative_string(
                data,
                entry,
                i32_at(data, entry + 2, "VTX replacement material name offset")?,
                "VTX replacement material name",
            )?
            .trim_end_matches(".vmt")
            .to_ascii_lowercase();
            if replacements.insert(material as usize, name).is_some() {
                return Err(format!(
                    "VTX LOD {lod} has duplicate replacement for material {material}"
                ));
            }
        }
        material_replacements.push(replacements);
    }

    let body_part_count = usize_i32(data, 28, "VTX body part count")?;
    let body_part_offset = usize_i32(data, 32, "VTX body part offset")?;
    table_range(
        data,
        body_part_offset,
        body_part_count,
        VTX_BODYPART_BYTES,
        MAX_BODY_PARTS,
        "VTX body parts",
    )?;
    let mut body_parts = Vec::with_capacity(body_part_count);
    let mut total_models = 0_usize;
    let mut total_meshes = 0_usize;
    let mut total_indices = 0_usize;
    for body_part_index in 0..body_part_count {
        let offset = body_part_offset + body_part_index * VTX_BODYPART_BYTES;
        let model_count = usize_i32(data, offset, "VTX model count")?;
        total_models = total_models
            .checked_add(model_count)
            .ok_or_else(|| "VTX model count overflows".to_owned())?;
        if total_models > MAX_MODELS {
            return Err(format!("VTX model count exceeds {MAX_MODELS}"));
        }
        let model_offset = relative_table_offset(
            offset,
            i32_at(data, offset + 4, "VTX model offset")?,
            "VTX model table",
        )?;
        table_range(
            data,
            model_offset,
            model_count,
            VTX_MODEL_BYTES,
            MAX_MODELS,
            "VTX models",
        )?;
        let mut models = Vec::with_capacity(model_count);
        for model_index in 0..model_count {
            let offset = model_offset + model_index * VTX_MODEL_BYTES;
            let model_lod_count = usize_i32(data, offset, "VTX model LOD count")?;
            if model_lod_count != lod_count {
                return Err(format!(
                    "VTX body part {body_part_index} model {model_index} has {model_lod_count} LODs; header declares {lod_count}"
                ));
            }
            let lod_offset = relative_table_offset(
                offset,
                i32_at(data, offset + 4, "VTX LOD offset")?,
                "VTX LOD table",
            )?;
            table_range(
                data,
                lod_offset,
                lod_count,
                VTX_LOD_BYTES,
                MAX_LODS,
                "VTX LODs",
            )?;
            let mut lods = Vec::with_capacity(lod_count);
            for lod_index in 0..lod_count {
                let offset = lod_offset + lod_index * VTX_LOD_BYTES;
                let mesh_count = usize_i32(data, offset, "VTX mesh count")?;
                total_meshes = total_meshes
                    .checked_add(mesh_count)
                    .ok_or_else(|| "VTX mesh count overflows".to_owned())?;
                if total_meshes > MAX_MESHES * MAX_LODS {
                    return Err("VTX mesh count exceeds the hard limit".to_owned());
                }
                let mesh_offset = relative_table_offset(
                    offset,
                    i32_at(data, offset + 4, "VTX mesh offset")?,
                    "VTX mesh table",
                )?;
                table_range(
                    data,
                    mesh_offset,
                    mesh_count,
                    VTX_MESH_BYTES,
                    MAX_MESHES,
                    "VTX meshes",
                )?;
                let switch_point = f32_at(data, offset + 8, "VTX LOD switch point")?;
                let mut meshes = Vec::with_capacity(mesh_count);
                for mesh_index in 0..mesh_count {
                    let offset = mesh_offset + mesh_index * VTX_MESH_BYTES;
                    let strip_group_count = usize_i32(data, offset, "VTX strip group count")?;
                    let strip_group_offset = relative_table_offset(
                        offset,
                        i32_at(data, offset + 4, "VTX strip group offset")?,
                        "VTX strip group table",
                    )?;
                    table_range(
                        data,
                        strip_group_offset,
                        strip_group_count,
                        VTX_STRIP_GROUP_BYTES,
                        MAX_MESHES,
                        "VTX strip groups",
                    )?;
                    let mut strip_groups = Vec::with_capacity(strip_group_count);
                    for group_index in 0..strip_group_count {
                        let offset = strip_group_offset + group_index * VTX_STRIP_GROUP_BYTES;
                        let vertex_count = usize_i32(data, offset, "VTX group vertex count")?;
                        let vertex_offset = relative_table_offset(
                            offset,
                            i32_at(data, offset + 4, "VTX group vertex offset")?,
                            "VTX group vertex table",
                        )?;
                        table_range(
                            data,
                            vertex_offset,
                            vertex_count,
                            VTX_VERTEX_BYTES,
                            MAX_VERTICES,
                            "VTX group vertices",
                        )?;
                        let mut source_vertex_ids = Vec::with_capacity(vertex_count);
                        for vertex in 0..vertex_count {
                            source_vertex_ids.push(u16_at(
                                data,
                                vertex_offset + vertex * VTX_VERTEX_BYTES + 4,
                                "VTX original mesh vertex ID",
                            )? as usize);
                        }
                        let index_count = usize_i32(data, offset + 8, "VTX group index count")?;
                        total_indices = total_indices
                            .checked_add(index_count)
                            .ok_or_else(|| "VTX index count overflows".to_owned())?;
                        if total_indices > MAX_INDICES {
                            return Err(format!("VTX index count exceeds {MAX_INDICES}"));
                        }
                        let index_offset = relative_table_offset(
                            offset,
                            i32_at(data, offset + 12, "VTX group index offset")?,
                            "VTX group index table",
                        )?;
                        table_range(
                            data,
                            index_offset,
                            index_count,
                            2,
                            MAX_INDICES,
                            "VTX group indices",
                        )?;
                        let mut group_indices = Vec::with_capacity(index_count);
                        for index in 0..index_count {
                            let vertex =
                                u16_at(data, index_offset + index * 2, "VTX group vertex index")?
                                    as usize;
                            if vertex >= vertex_count {
                                return Err(format!(
                                    "VTX strip group index {vertex} exceeds {vertex_count} vertices"
                                ));
                            }
                            group_indices.push(vertex);
                        }
                        let strip_count = usize_i32(data, offset + 16, "VTX strip count")?;
                        let strip_offset = relative_table_offset(
                            offset,
                            i32_at(data, offset + 20, "VTX strip offset")?,
                            "VTX strip table",
                        )?;
                        table_range(
                            data,
                            strip_offset,
                            strip_count,
                            VTX_STRIP_BYTES,
                            MAX_MESHES,
                            "VTX strips",
                        )?;
                        let mut triangles = Vec::new();
                        for strip_index in 0..strip_count {
                            let strip = strip_offset + strip_index * VTX_STRIP_BYTES;
                            let count = usize_i32(data, strip, "VTX strip index count")?;
                            let first = usize_i32(data, strip + 4, "VTX strip index offset")?;
                            let end = first
                                .checked_add(count)
                                .ok_or_else(|| "VTX strip index range overflows".to_owned())?;
                            if end > group_indices.len() {
                                return Err(format!(
                                    "VTX strip {strip_index} index range exceeds its strip group"
                                ));
                            }
                            let vertex_count_in_strip =
                                usize_i32(data, strip + 8, "VTX strip vertex count")?;
                            let first_vertex =
                                usize_i32(data, strip + 12, "VTX strip vertex offset")?;
                            let vertex_end = first_vertex
                                .checked_add(vertex_count_in_strip)
                                .ok_or_else(|| "VTX strip vertex range overflows".to_owned())?;
                            if vertex_end > vertex_count {
                                return Err(format!(
                                    "VTX strip {strip_index} vertex range exceeds its strip group"
                                ));
                            }
                            let flags = *checked_range(data, strip + 18, 1, "VTX strip flags")?
                                .first()
                                .unwrap();
                            let indices = &group_indices[first..end];
                            match flags {
                                0x01 => {
                                    if !count.is_multiple_of(3) {
                                        return Err(format!(
                                            "VTX triangle-list strip {strip_index} has {count} indices"
                                        ));
                                    }
                                    triangles.extend(
                                        indices
                                            .chunks_exact(3)
                                            .map(|triangle| [triangle[0], triangle[1], triangle[2]])
                                            .filter(|triangle| {
                                                triangle[0] != triangle[1]
                                                    && triangle[1] != triangle[2]
                                                    && triangle[0] != triangle[2]
                                            }),
                                    );
                                }
                                0x02 => {
                                    for triangle_index in 0..count.saturating_sub(2) {
                                        let mut triangle = [
                                            indices[triangle_index],
                                            indices[triangle_index + 1],
                                            indices[triangle_index + 2],
                                        ];
                                        if triangle_index % 2 == 1 {
                                            triangle.swap(0, 1);
                                        }
                                        if triangle[0] != triangle[1]
                                            && triangle[1] != triangle[2]
                                            && triangle[0] != triangle[2]
                                        {
                                            triangles.push(triangle);
                                        }
                                    }
                                }
                                _ => {
                                    return Err(format!(
                                        "VTX strip {strip_index} has unsupported flags {flags:#04x}"
                                    ));
                                }
                            }
                        }
                        strip_groups.push(VtxStripGroup {
                            source_vertex_ids,
                            triangles,
                        });
                    }
                    meshes.push(VtxMesh { strip_groups });
                }
                lods.push(VtxLod {
                    switch_point,
                    meshes,
                });
            }
            models.push(VtxModel { lods });
        }
        body_parts.push(VtxBodyPart { models });
    }
    Ok(ParsedVtx {
        checksum,
        lod_count,
        body_parts,
        material_replacements,
    })
}

#[derive(Default)]
struct PrimitiveBuffers {
    positions: Vec<f32>,
    normals: Vec<f32>,
    tangents: Vec<f32>,
    uv0: Vec<f32>,
    weights: Vec<f32>,
    joints: Vec<u16>,
    indices: Vec<u32>,
    morphs: Vec<MorphBuffers>,
}

#[derive(Default)]
struct MorphBuffers {
    positions: Vec<f32>,
    normals: Vec<f32>,
    wrinkles: Vec<f32>,
}

#[derive(Default)]
struct GlbBuilder {
    binary: Vec<u8>,
    buffer_views: Vec<Value>,
    accessors: Vec<Value>,
}

fn pad4(data: &mut Vec<u8>, byte: u8) {
    while !data.len().is_multiple_of(4) {
        data.push(byte);
    }
}

fn accessor_type(width: usize) -> &'static str {
    match width {
        1 => "SCALAR",
        2 => "VEC2",
        3 => "VEC3",
        4 => "VEC4",
        16 => "MAT4",
        _ => unreachable!("validated accessor width"),
    }
}

impl GlbBuilder {
    fn add_bytes(
        &mut self,
        bytes: &[u8],
        component_type: u32,
        count: usize,
        width: usize,
        target: Option<u32>,
        min_max: Option<(Vec<f32>, Vec<f32>)>,
    ) -> usize {
        pad4(&mut self.binary, 0);
        let byte_offset = self.binary.len();
        self.binary.extend_from_slice(bytes);
        let view = self.buffer_views.len();
        let mut view_json = json!({
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": bytes.len()
        });
        if let Some(target) = target {
            view_json["target"] = json!(target);
        }
        self.buffer_views.push(view_json);
        let accessor = self.accessors.len();
        let mut accessor_json = json!({
            "bufferView": view,
            "componentType": component_type,
            "count": count,
            "type": accessor_type(width)
        });
        if let Some((min, max)) = min_max {
            accessor_json["min"] = json!(min);
            accessor_json["max"] = json!(max);
        }
        self.accessors.push(accessor_json);
        accessor
    }

    fn add_f32(&mut self, values: &[f32], width: usize, target: Option<u32>) -> usize {
        let bytes: Vec<_> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let min_max = if width <= 4 && !values.is_empty() {
            let mut min = vec![f32::INFINITY; width];
            let mut max = vec![f32::NEG_INFINITY; width];
            for row in values.chunks_exact(width) {
                for index in 0..width {
                    min[index] = min[index].min(row[index]);
                    max[index] = max[index].max(row[index]);
                }
            }
            Some((min, max))
        } else {
            None
        };
        self.add_bytes(&bytes, 5126, values.len() / width, width, target, min_max)
    }

    fn add_u16(&mut self, values: &[u16], width: usize, target: Option<u32>) -> usize {
        let bytes: Vec<_> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        self.add_bytes(&bytes, 5123, values.len() / width, width, target, None)
    }

    fn add_u32_indices(&mut self, values: &[u32]) -> usize {
        let bytes: Vec<_> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        self.add_bytes(
            &bytes,
            5125,
            values.len(),
            1,
            Some(34963),
            Some((
                vec![values.iter().copied().min().unwrap_or(0) as f32],
                vec![values.iter().copied().max().unwrap_or(0) as f32],
            )),
        )
    }
}

fn encode_glb(mut document: Value, mut binary: Vec<u8>) -> Result<Vec<u8>, String> {
    pad4(&mut binary, 0);
    document["buffers"] = json!([{ "byteLength": binary.len() }]);
    let mut document = serde_json::to_vec(&document)
        .map_err(|error| format!("failed to serialize StudioModel GLTF: {error}"))?;
    pad4(&mut document, b' ');
    let total = 12_usize
        .checked_add(8 + document.len())
        .and_then(|value| value.checked_add(8 + binary.len()))
        .ok_or_else(|| "StudioModel GLB length overflows".to_owned())?;
    let total = u32::try_from(total).map_err(|_| "StudioModel GLB exceeds 4 GiB".to_owned())?;
    let mut glb = Vec::with_capacity(total as usize);
    glb.extend_from_slice(&0x4654_6c67_u32.to_le_bytes());
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&total.to_le_bytes());
    glb.extend_from_slice(&(document.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    glb.extend_from_slice(&document);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004e_4942_u32.to_le_bytes());
    glb.extend_from_slice(&binary);
    Ok(glb)
}

fn domain(status: StudioFeatureStatus, count: usize, reason: Option<String>) -> StudioDomainStatus {
    StudioDomainStatus {
        status,
        count,
        reason,
    }
}

fn source_file(
    role: &'static str,
    extension: &'static str,
    bytes: &[u8],
    checksum: Option<i32>,
) -> StudioSourceFile {
    StudioSourceFile {
        role,
        extension,
        byte_length: bytes.len(),
        sha256: sha256(bytes),
        checksum,
    }
}

fn package_content_hash(input: &StudioModelInput<'_>) -> String {
    let mut hash = Sha256::new();
    hash.update(STUDIO_MODEL_PACKAGE_VERSION.to_le_bytes());
    hash.update(input.source_path.as_bytes());
    hash.update([0]);
    hash.update((input.skin as u64).to_le_bytes());
    for (role, bytes) in [
        (b"mdl".as_slice(), input.mdl),
        (b"vvd".as_slice(), input.vvd),
        (b"vtx".as_slice(), input.vtx),
    ] {
        hash.update(role);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    for (role, bytes) in [
        (b"ani".as_slice(), input.ani),
        (b"phy".as_slice(), input.phy),
    ] {
        hash.update(role);
        if let Some(bytes) = bytes {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        } else {
            hash.update(0_u64.to_le_bytes());
        }
    }
    format!("{:x}", hash.finalize())
}

fn metadata_package_content_hash(source_path: &str, mdl: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(STUDIO_MODEL_PACKAGE_VERSION.to_le_bytes());
    hash.update(source_path.as_bytes());
    hash.update([0]);
    hash.update(0_u64.to_le_bytes());
    hash.update(b"mdl");
    hash.update((mdl.len() as u64).to_le_bytes());
    hash.update(mdl);
    format!("{:x}", hash.finalize())
}

fn pose_to_bone_matrix(matrix: [f32; 12]) -> [f32; 16] {
    [
        matrix[0], matrix[4], matrix[8], 0.0, matrix[1], matrix[5], matrix[9], 0.0, matrix[2],
        matrix[6], matrix[10], 0.0, matrix[3], matrix[7], matrix[11], 1.0,
    ]
}

fn resolved_material_name(
    mdl: &ParsedMdl,
    vtx: &ParsedVtx,
    selected_skin: usize,
    lod: usize,
    material_slot: usize,
) -> Result<(usize, String), String> {
    let texture_index = if mdl.skins.is_empty() {
        material_slot
    } else {
        *mdl.skins[selected_skin]
            .texture_indices
            .get(material_slot)
            .ok_or_else(|| format!("skin {selected_skin} has no material slot {material_slot}"))?
    };
    let material = mdl.materials.get(texture_index).ok_or_else(|| {
        format!("material slot {material_slot} resolves to missing texture {texture_index}")
    })?;
    Ok((
        texture_index,
        vtx.material_replacements[lod]
            .get(&texture_index)
            .cloned()
            .unwrap_or_else(|| material.name.clone()),
    ))
}

fn add_primitive(
    arrays: &mut GlbBuilder,
    primitive: &PrimitiveBuffers,
    material: usize,
    skinned: bool,
) -> (Value, Vec<Option<usize>>) {
    let mut attributes = serde_json::Map::new();
    attributes.insert(
        "POSITION".to_owned(),
        json!(arrays.add_f32(&primitive.positions, 3, Some(34962))),
    );
    attributes.insert(
        "NORMAL".to_owned(),
        json!(arrays.add_f32(&primitive.normals, 3, Some(34962))),
    );
    attributes.insert(
        "TANGENT".to_owned(),
        json!(arrays.add_f32(&primitive.tangents, 4, Some(34962))),
    );
    attributes.insert(
        "TEXCOORD_0".to_owned(),
        json!(arrays.add_f32(&primitive.uv0, 2, Some(34962))),
    );
    if skinned {
        attributes.insert(
            "WEIGHTS_0".to_owned(),
            json!(arrays.add_f32(&primitive.weights, 4, Some(34962))),
        );
        attributes.insert(
            "JOINTS_0".to_owned(),
            json!(arrays.add_u16(&primitive.joints, 4, Some(34962))),
        );
    }
    let mut wrinkle_accessors = Vec::with_capacity(primitive.morphs.len());
    let targets: Vec<_> = primitive
        .morphs
        .iter()
        .map(|morph| {
            let position = arrays.add_f32(&morph.positions, 3, Some(34962));
            let normal = arrays.add_f32(&morph.normals, 3, Some(34962));
            let wrinkle = morph
                .wrinkles
                .iter()
                .any(|value| *value != 0.0)
                .then(|| arrays.add_f32(&morph.wrinkles, 1, Some(34962)));
            wrinkle_accessors.push(wrinkle);
            json!({ "POSITION": position, "NORMAL": normal })
        })
        .collect();
    let mut output = json!({
        "attributes": attributes,
        "indices": arrays.add_u32_indices(&primitive.indices),
        "material": material,
        "mode": 4
    });
    if !targets.is_empty() {
        output["targets"] = json!(targets);
    }
    (output, wrinkle_accessors)
}

pub fn export_studio_model(input: &StudioModelInput<'_>) -> Result<StudioModelExport, String> {
    let source_path = normalized_model_path(input.source_path)?;
    let mdl = parse_mdl(input.mdl).map_err(|error| format!("{source_path}: {error}"))?;
    let vvd =
        parse_vvd(input.vvd, mdl.checksum).map_err(|error| format!("{source_path}: {error}"))?;
    let vtx =
        parse_vtx(input.vtx, mdl.checksum).map_err(|error| format!("{source_path}: {error}"))?;
    let source_files = {
        let mut files = vec![
            source_file("header", ".mdl", input.mdl, Some(mdl.checksum)),
            source_file("vertices", ".vvd", input.vvd, Some(vvd.checksum)),
            source_file("topology", ".dx90.vtx", input.vtx, Some(vtx.checksum)),
        ];
        if let Some(ani) = input.ani {
            files.push(source_file("externalAnimations", ".ani", ani, None));
        }
        if let Some(phy) = input.phy {
            files.push(source_file("physics", ".phy", phy, None));
        }
        files
    };
    export_studio_model_parts(
        input,
        source_path,
        mdl,
        vvd,
        vtx,
        source_files,
        package_content_hash(input),
        true,
    )
}

pub fn export_studio_metadata_model(
    source_path: &str,
    mdl_bytes: &[u8],
) -> Result<StudioModelExport, String> {
    let source_path = normalized_model_path(source_path)?;
    let mdl = parse_mdl(mdl_bytes).map_err(|error| format!("{source_path}: {error}"))?;
    if !mdl.body_parts.is_empty() {
        return Err(format!(
            "{source_path}: metadata-only export requires a model with no body parts"
        ));
    }
    if mdl.animation_block_count > 0 {
        return Err(format!(
            "{source_path}: metadata-only export cannot consume external animation blocks"
        ));
    }
    let checksum = mdl.checksum;
    let input = StudioModelInput {
        source_path: &source_path,
        mdl: mdl_bytes,
        vvd: &[],
        vtx: &[],
        ani: None,
        phy: None,
        skin: 0,
    };
    export_studio_model_parts(
        &input,
        source_path.clone(),
        mdl,
        ParsedVvd {
            checksum,
            lod_vertex_counts: vec![0],
            source_vertices: Vec::new(),
            fixups: Vec::new(),
        },
        ParsedVtx {
            checksum,
            lod_count: 1,
            body_parts: Vec::new(),
            material_replacements: vec![BTreeMap::new()],
        },
        vec![source_file("header", ".mdl", mdl_bytes, Some(checksum))],
        metadata_package_content_hash(&source_path, mdl_bytes),
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn export_studio_model_parts(
    input: &StudioModelInput<'_>,
    source_path: String,
    mdl: ParsedMdl,
    vvd: ParsedVvd,
    vtx: ParsedVtx,
    source_files: Vec<StudioSourceFile>,
    package_content_hash: String,
    has_geometry_companions: bool,
) -> Result<StudioModelExport, String> {
    if vvd.checksum != mdl.checksum || vtx.checksum != mdl.checksum {
        return Err(format!(
            "{source_path}: StudioModel companion checksums disagree"
        ));
    }
    if vvd.lod_vertex_counts.len() != vtx.lod_count {
        return Err(format!(
            "{source_path}: VVD declares {} LODs while VTX declares {}",
            vvd.lod_vertex_counts.len(),
            vtx.lod_count
        ));
    }
    if mdl.body_parts.len() != vtx.body_parts.len() {
        return Err(format!(
            "{source_path}: MDL declares {} body parts while VTX declares {}",
            mdl.body_parts.len(),
            vtx.body_parts.len()
        ));
    }
    if mdl.animation_block_count > 0 && input.ani.is_none() {
        return Err(format!(
            "{source_path}: {} external animation blocks require an ANI companion; export aborted",
            mdl.animation_block_count
        ));
    }
    let selected_skin = if mdl.skins.is_empty() || input.skin >= mdl.skins.len() {
        0
    } else {
        input.skin
    };

    let mut material_names = Vec::new();
    let mut material_indices = BTreeMap::new();
    for material in &mdl.materials {
        material_indices.insert(material.name.clone(), material_names.len());
        material_names.push(material.name.clone());
    }
    for replacements in &vtx.material_replacements {
        for name in replacements.values() {
            if !material_indices.contains_key(name) {
                material_indices.insert(name.clone(), material_names.len());
                material_names.push(name.clone());
            }
        }
    }
    let materials_json: Vec<_> = material_names
        .iter()
        .map(|name| {
            let candidates = mdl
                .materials
                .iter()
                .find(|material| material.name == *name)
                .map(|material| material.candidates.clone())
                .unwrap_or_else(|| vec![format!("materials/{name}.vmt")]);
            json!({
                "name": name,
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 0.0,
                    "roughnessFactor": 1.0
                },
                "extras": {
                    "sourceMaterial": name,
                    "sourceCandidates": candidates,
                    "resolutionStatus": "referenceOnly"
                }
            })
        })
        .collect();

    let mut arrays = GlbBuilder::default();
    let mut nodes = vec![json!({
        "name": "source_to_gltf",
        "rotation": SOURCE_TO_GLTF_ROTATION,
        "extras": { "coordinateTransform": "Source XYZ to glTF X,Z,-Y" }
    })];
    let mut root_children = Vec::new();
    let mut bone_nodes = Vec::with_capacity(mdl.bones.len());
    for bone in &mdl.bones {
        let node_index = nodes.len();
        bone_nodes.push(node_index);
        nodes.push(json!({
            "name": bone.name,
            "translation": bone.position,
            "rotation": bone.quaternion,
            "extras": {
                "sourceBoneIndex": bone.index,
                "sourceBoneFlags": bone.flags,
                "sourceBoneContents": bone.contents
            }
        }));
    }
    for bone in &mdl.bones {
        let node = bone_nodes[bone.index];
        if bone.parent < 0 {
            root_children.push(node);
        } else {
            let parent = bone_nodes[bone.parent as usize];
            let children = nodes[parent]
                .as_object_mut()
                .unwrap()
                .entry("children")
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .unwrap();
            children.push(json!(node));
        }
    }
    let mut attachment_manifest = mdl.attachments.clone();
    for attachment in &mut attachment_manifest {
        let node = nodes.len();
        attachment.gltf_node = Some(node);
        nodes.push(json!({
            "name": format!("attachment_{}", attachment.name),
            "matrix": pose_to_bone_matrix(attachment.local),
            "extras": {
                "sourceAttachmentIndex": attachment.index,
                "sourceAttachmentName": attachment.name,
                "sourceAttachmentFlags": attachment.flags,
                "sourceAttachmentBone": attachment.bone
            }
        }));
        let parent = bone_nodes[attachment.bone];
        nodes[parent]
            .as_object_mut()
            .unwrap()
            .entry("children")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .unwrap()
            .push(json!(node));
    }
    let skin = if mdl.bones.is_empty() {
        None
    } else {
        let matrices: Vec<f32> = mdl
            .bones
            .iter()
            .flat_map(|bone| pose_to_bone_matrix(bone.pose_to_bone))
            .collect();
        let accessor = arrays.add_f32(&matrices, 16, None);
        Some(json!({
            "name": "Source skeleton",
            "inverseBindMatrices": accessor,
            "joints": bone_nodes,
            "skeleton": bone_nodes[0]
        }))
    };
    let decoded_animations = decode_animations(input.mdl, input.ani, &mdl)
        .map_err(|error| format!("{source_path}: {error}"))?;
    let mut animations_json = Vec::with_capacity(decoded_animations.len());
    let mut animation_manifest = Vec::with_capacity(decoded_animations.len());
    for (animation, decoded) in mdl.animations.iter().zip(decoded_animations) {
        let times: Vec<_> = (0..animation.metadata.frame_count)
            .map(|frame| frame as f32 / animation.metadata.fps)
            .collect();
        let input_accessor = arrays.add_f32(&times, 1, None);
        let mut samplers = Vec::with_capacity(mdl.bones.len() * 2);
        let mut channels = Vec::with_capacity(mdl.bones.len() * 2);
        for (bone, node) in bone_nodes.iter().copied().enumerate() {
            let translations: Vec<_> = decoded
                .translations
                .iter()
                .flat_map(|frame| frame[bone])
                .collect();
            let rotations: Vec<_> = decoded
                .rotations
                .iter()
                .flat_map(|frame| frame[bone])
                .collect();
            for (path, output) in [
                ("translation", arrays.add_f32(&translations, 3, None)),
                ("rotation", arrays.add_f32(&rotations, 4, None)),
            ] {
                let sampler = samplers.len();
                samplers.push(json!({
                    "input": input_accessor,
                    "output": output,
                    "interpolation": "LINEAR"
                }));
                channels.push(json!({
                    "sampler": sampler,
                    "target": { "node": node, "path": path }
                }));
            }
        }
        let gltf_animation = animations_json.len();
        animations_json.push(json!({
            "name": animation.metadata.name,
            "samplers": samplers,
            "channels": channels,
            "extras": {
                "sourceAnimationIndex": animation.metadata.index,
                "sourceFlags": animation.metadata.flags,
                "sourceAnimationBlock": animation.metadata.animation_block,
                "sourceSectionFrameCount": animation.metadata.section_frame_count
            }
        }));
        let mut metadata = animation.metadata.clone();
        metadata.decode_status = if metadata.ik_rule_count == 0
            && metadata.local_hierarchy_count == 0
            && metadata.zero_frame_count == 0
        {
            StudioFeatureStatus::Supported
        } else {
            StudioFeatureStatus::DetectedOnly
        };
        metadata.gltf_animation = Some(gltf_animation);
        metadata.sample_count = metadata
            .frame_count
            .checked_mul(mdl.bones.len())
            .ok_or_else(|| "StudioModel animation sample count overflows".to_owned())?;
        animation_manifest.push(metadata);
    }

    let mut meshes_json = Vec::new();
    let mut body_parts_manifest = Vec::with_capacity(mdl.body_parts.len());
    let mut geometry_vertex_count = 0_usize;
    let mut geometry_index_count = 0_usize;
    for (body_part_index, (mdl_body_part, vtx_body_part)) in
        mdl.body_parts.iter().zip(&vtx.body_parts).enumerate()
    {
        if mdl_body_part.models.len() != vtx_body_part.models.len() {
            return Err(format!(
                "{source_path}: body part {body_part_index} MDL/VTX model counts disagree"
            ));
        }
        let mut models_manifest = Vec::with_capacity(mdl_body_part.models.len());
        for (model_index, (mdl_model, vtx_model)) in mdl_body_part
            .models
            .iter()
            .zip(&vtx_body_part.models)
            .enumerate()
        {
            let mut mesh_lods = Vec::new();
            for (lod_index, lod) in vtx_model.lods.iter().enumerate() {
                if mdl_model.meshes.len() != lod.meshes.len() {
                    return Err(format!(
                        "{source_path}: body part {body_part_index} model {model_index} LOD {lod_index} MDL/VTX mesh counts disagree"
                    ));
                }
                for (mesh_index, (mdl_mesh, vtx_mesh)) in
                    mdl_model.meshes.iter().zip(&lod.meshes).enumerate()
                {
                    let material_slot = mdl_mesh.material as usize;
                    let (texture_index, material_name) = resolved_material_name(
                        &mdl,
                        &vtx,
                        selected_skin,
                        lod_index,
                        material_slot,
                    )?;
                    let material_index = material_indices[&material_name];
                    let mut primitive = PrimitiveBuffers {
                        morphs: (0..mdl_mesh.flexes.len())
                            .map(|_| MorphBuffers::default())
                            .collect(),
                        ..PrimitiveBuffers::default()
                    };
                    for group in &vtx_mesh.strip_groups {
                        let group_base = primitive.positions.len() / 3;
                        for &mesh_vertex in &group.source_vertex_ids {
                            if mesh_vertex >= mdl_mesh.vertex_count {
                                return Err(format!(
                                    "{source_path}: VTX vertex {mesh_vertex} exceeds MDL mesh {mesh_index} vertex count {}",
                                    mdl_mesh.vertex_count
                                ));
                            }
                            let model_vertex =
                                mdl_mesh.vertex_offset.checked_add(mesh_vertex).ok_or_else(
                                    || "StudioModel mesh vertex index overflows".to_owned(),
                                )?;
                            if model_vertex >= mdl_model.vertex_count {
                                return Err(format!(
                                    "{source_path}: mesh {mesh_index} vertex {model_vertex} exceeds model vertex count {}",
                                    mdl_model.vertex_count
                                ));
                            }
                            let global_vertex = mdl_model
                                .vertex_start
                                .checked_add(model_vertex)
                                .ok_or_else(|| {
                                    "StudioModel global vertex index overflows".to_owned()
                                })?;
                            let vertex = vvd.vertex(lod_index, global_vertex).map_err(|error| {
                                format!(
                                    "{source_path}: global vertex {global_vertex} is absent from VVD LOD {lod_index}: {error}"
                                )
                            })?;
                            primitive.positions.extend_from_slice(&vertex.position);
                            primitive.normals.extend_from_slice(&vertex.normal);
                            primitive.tangents.extend_from_slice(&vertex.tangent);
                            primitive.uv0.extend_from_slice(&vertex.uv);
                            for (flex, morph) in mdl_mesh.flexes.iter().zip(&mut primitive.morphs) {
                                let delta = flex.vertices.get(&mesh_vertex);
                                morph.positions.extend_from_slice(
                                    &delta.map(|value| value.position_delta).unwrap_or([0.0; 3]),
                                );
                                morph.normals.extend_from_slice(
                                    &delta.map(|value| value.normal_delta).unwrap_or([0.0; 3]),
                                );
                                morph.wrinkles.push(
                                    delta.and_then(|value| value.wrinkle_delta).unwrap_or(0.0),
                                );
                            }
                            let mut weights = [0.0; 4];
                            let mut joints = [0_u16; 4];
                            if !mdl.bones.is_empty() {
                                let mut sum = 0.0;
                                for influence in 0..vertex.bone_count as usize {
                                    let bone = vertex.bones[influence] as usize;
                                    if bone >= mdl.bones.len() {
                                        return Err(format!(
                                            "{source_path}: VVD vertex references missing bone {bone}"
                                        ));
                                    }
                                    weights[influence] = vertex.weights[influence];
                                    joints[influence] = bone as u16;
                                    sum += weights[influence];
                                }
                                if vertex.bone_count == 0 {
                                    weights[0] = 1.0;
                                } else if (sum - 1.0).abs() > 0.002 {
                                    return Err(format!(
                                        "{source_path}: VVD vertex bone weights sum to {sum}"
                                    ));
                                }
                            }
                            primitive.weights.extend_from_slice(&weights);
                            primitive.joints.extend_from_slice(&joints);
                        }
                        for triangle in &group.triangles {
                            for &index in triangle {
                                primitive
                                    .indices
                                    .push(u32::try_from(group_base + index).map_err(|_| {
                                        "StudioModel GLB vertex index exceeds u32".to_owned()
                                    })?);
                            }
                        }
                    }
                    if primitive.indices.is_empty() {
                        continue;
                    }
                    geometry_vertex_count += primitive.positions.len() / 3;
                    geometry_index_count += primitive.indices.len();
                    let (gltf_primitive, wrinkle_accessors) = add_primitive(
                        &mut arrays,
                        &primitive,
                        material_index,
                        !mdl.bones.is_empty(),
                    );
                    let morph_targets: Vec<_> = mdl_mesh
                        .flexes
                        .iter()
                        .enumerate()
                        .map(|(target, flex)| StudioMorphTarget {
                            target,
                            flex_descriptor: flex.descriptor,
                            flex_pair: flex.pair,
                            thresholds: flex.thresholds,
                            affected_vertices: flex.vertices.len(),
                            vertex_animation_type: match flex.animation_type {
                                0 => "normal",
                                1 => "wrinkle",
                                _ => unreachable!("validated flex animation type"),
                            },
                            wrinkle_accessor: wrinkle_accessors[target],
                        })
                        .collect();
                    let target_names: Vec<_> = mdl_mesh
                        .flexes
                        .iter()
                        .map(|flex| mdl.flex_descriptors[flex.descriptor].name.clone())
                        .collect();
                    let gltf_mesh = meshes_json.len();
                    meshes_json.push(json!({
                        "name": format!("body{body_part_index}_model{model_index}_lod{lod_index}_mesh{mesh_index}"),
                        "primitives": [gltf_primitive],
                        "weights": vec![0.0; morph_targets.len()],
                        "extras": {
                            "sourceBodyPart": body_part_index,
                            "sourceModel": model_index,
                            "sourceLod": lod_index,
                            "sourceMesh": mesh_index,
                            "sourceMaterialSlot": material_slot,
                            "sourceTextureIndex": texture_index,
                            "sourceMaterial": material_name,
                            "lodSwitchPoint": lod.switch_point,
                            "targetNames": target_names,
                            "sourceMorphTargets": morph_targets
                        }
                    }));
                    let gltf_node = nodes.len();
                    let mut node = json!({
                        "name": format!("body{body_part_index}_model{model_index}_lod{lod_index}_mesh{mesh_index}"),
                        "mesh": gltf_mesh,
                        "extras": {
                            "sourceBodyPart": body_part_index,
                            "sourceModel": model_index,
                            "sourceLod": lod_index,
                            "sourceMesh": mesh_index,
                            "defaultBodygroup": model_index == 0,
                            "defaultLod": lod_index == 0
                        }
                    });
                    if skin.is_some() {
                        node["skin"] = json!(0);
                    }
                    nodes.push(node);
                    if model_index == 0 && lod_index == 0 {
                        root_children.push(gltf_node);
                    }
                    mesh_lods.push(StudioMeshLod {
                        lod: lod_index,
                        switch_point: lod.switch_point,
                        gltf_mesh,
                        gltf_node,
                        vertex_count: primitive.positions.len() / 3,
                        index_count: primitive.indices.len(),
                        material_slot,
                        material_index,
                        morph_targets,
                    });
                }
            }
            models_manifest.push(StudioSubModel {
                index: model_index,
                name: mdl_model.name.clone(),
                vertex_count: mdl_model.vertex_count,
                mesh_lods,
            });
        }
        body_parts_manifest.push(StudioBodyPart {
            index: body_part_index,
            name: mdl_body_part.name.clone(),
            base: mdl_body_part.base,
            default_model: 0,
            models: models_manifest,
        });
    }
    nodes[0]["children"] = json!(root_children);

    let geometry_status = if geometry_index_count > 0 {
        StudioFeatureStatus::Supported
    } else {
        StudioFeatureStatus::NotPresent
    };
    let incomplete_animations = animation_manifest
        .iter()
        .filter(|animation| animation.decode_status != StudioFeatureStatus::Supported)
        .count();
    let animation_status = if mdl.animations.is_empty() {
        StudioFeatureStatus::NotPresent
    } else if incomplete_animations > 0 {
        StudioFeatureStatus::DetectedOnly
    } else {
        StudioFeatureStatus::Supported
    };
    let formats = StudioModelFormatMatrix {
        mdl: domain(StudioFeatureStatus::Supported, 1, None),
        vvd: domain(
            if has_geometry_companions {
                StudioFeatureStatus::Supported
            } else {
                StudioFeatureStatus::NotPresent
            },
            usize::from(has_geometry_companions),
            None,
        ),
        vtx: domain(
            if has_geometry_companions {
                StudioFeatureStatus::Supported
            } else {
                StudioFeatureStatus::NotPresent
            },
            usize::from(has_geometry_companions),
            None,
        ),
        ani: domain(
            if input.ani.is_some() {
                StudioFeatureStatus::DetectedOnly
            } else {
                StudioFeatureStatus::NotPresent
            },
            usize::from(input.ani.is_some()),
            input.ani.is_some().then(|| {
                "ANI bytes are provenance-linked but no external blocks are consumed".to_owned()
            }),
        ),
        phy: domain(
            if input.phy.is_some() {
                StudioFeatureStatus::DetectedOnly
            } else {
                StudioFeatureStatus::NotPresent
            },
            usize::from(input.phy.is_some()),
            input
                .phy
                .is_some()
                .then(|| "PHY is linked for the dedicated physics decoder".to_owned()),
        ),
        geometry: domain(geometry_status, geometry_index_count / 3, None),
        skins: domain(StudioFeatureStatus::Supported, mdl.skins.len(), None),
        bodygroups: domain(StudioFeatureStatus::Supported, mdl.body_parts.len(), None),
        lods: domain(
            if has_geometry_companions {
                StudioFeatureStatus::Supported
            } else {
                StudioFeatureStatus::NotPresent
            },
            if has_geometry_companions {
                vtx.lod_count
            } else {
                0
            },
            None,
        ),
        skeleton: domain(
            if mdl.bones.is_empty() {
                StudioFeatureStatus::NotPresent
            } else {
                StudioFeatureStatus::Supported
            },
            mdl.bones.len(),
            None,
        ),
        animations: domain(
            animation_status,
            mdl.animations.len(),
            (incomplete_animations > 0).then(|| {
                format!(
                    "{incomplete_animations} animations retain IK, local-hierarchy, or zero-frame metadata without applying it to GLTF samples"
                )
            }),
        ),
        sequences: domain(
            if mdl.sequences.is_empty() {
                StudioFeatureStatus::NotPresent
            } else {
                StudioFeatureStatus::DetectedOnly
            },
            mdl.sequences.len(),
            (!mdl.sequences.is_empty()).then(|| {
                "sequence identities and blend indices are retained; complete Source sequence-layer evaluation is not emitted"
                    .to_owned()
            }),
        ),
        attachments: domain(StudioFeatureStatus::Supported, mdl.attachments.len(), None),
        flexes: domain(
            if mdl.flex_descriptors.is_empty()
                && mdl.flex_controllers.is_empty()
                && mdl.flex_rules.is_empty()
                && mdl.mesh_flex_count == 0
            {
                StudioFeatureStatus::NotPresent
            } else {
                StudioFeatureStatus::Supported
            },
            mdl.mesh_flex_count,
            None,
        ),
        include_models: domain(
            if mdl.include_models.is_empty() {
                StudioFeatureStatus::NotPresent
            } else {
                StudioFeatureStatus::DetectedOnly
            },
            mdl.include_models.len(),
            (!mdl.include_models.is_empty()).then(|| {
                "include identities are retained for package-level composition".to_owned()
            }),
        ),
    };
    let physics_link = StudioPhysicsLink {
        status: if input.phy.is_some() {
            StudioFeatureStatus::DetectedOnly
        } else {
            StudioFeatureStatus::NotPresent
        },
        byte_length: input.phy.map(<[u8]>::len),
        sha256: input.phy.map(sha256),
    };
    let manifest = StudioModelManifest {
        schema: "bsp-to-glb/studio-model-package",
        schema_version: STUDIO_MODEL_PACKAGE_VERSION,
        source_path,
        package_content_hash,
        selected_skin,
        checksum: mdl.checksum,
        mdl_version: mdl.version,
        mdl_name: mdl.name,
        flags: mdl.flags,
        bounds: mdl.bounds,
        source_files,
        formats,
        materials: mdl.materials,
        skin_families: mdl.skins,
        body_parts: body_parts_manifest,
        bones: mdl.bones,
        animations: animation_manifest,
        sequences: mdl.sequences,
        attachments: attachment_manifest,
        include_models: mdl.include_models,
        flex_descriptors: mdl.flex_descriptors,
        flex_controllers: mdl.flex_controllers,
        flex_rules: mdl.flex_rules,
        physics_link,
    };
    let document = json!({
        "asset": {
            "version": "2.0",
            "generator": concat!("bsp-to-glb StudioModel ", env!("CARGO_PKG_VERSION")),
            "extras": {
                "schema": manifest.schema,
                "schemaVersion": manifest.schema_version,
                "packageContentHash": manifest.package_content_hash,
                "sourcePath": manifest.source_path,
                "sourceChecksum": manifest.checksum
            }
        },
        "scene": 0,
        "scenes": [{ "name": "StudioModel defaults", "nodes": [0] }],
        "nodes": nodes,
        "meshes": meshes_json,
        "materials": materials_json,
        "skins": skin.into_iter().collect::<Vec<_>>(),
        "animations": animations_json,
        "bufferViews": arrays.buffer_views,
        "accessors": arrays.accessors,
        "extras": {
            "defaultBodygroups": "model zero per bodypart",
            "defaultLod": 0,
            "retainedAlternateNodeCount": manifest
                .body_parts
                .iter()
                .flat_map(|body| &body.models)
                .flat_map(|model| &model.mesh_lods)
                .filter(|mesh| mesh.lod != 0)
                .count(),
            "vertexCount": geometry_vertex_count,
            "indexCount": geometry_index_count
        }
    });
    Ok(StudioModelExport {
        glb: encode_glb(document, arrays.binary)?,
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        MDL_BONE_BYTES, MDL_HEADER_BYTES, ParsedVvd, SOURCE_TO_GLTF_ROTATION, StudioFeatureStatus,
        StudioModelInput, VvdFixup, VvdVertex, animation_value, compressed_quaternion,
        export_studio_metadata_model, package_content_hash, parse_mdl,
    };

    fn write_i32(data: &mut [u8], offset: usize, value: i32) {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_f32(data: &mut [u8], offset: usize, value: f32) {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn reads_bone_contents_from_the_v48_contract_offset() {
        let bone_offset = MDL_HEADER_BYTES;
        let name_offset = bone_offset + MDL_BONE_BYTES;
        let mut mdl = vec![0_u8; name_offset + 5];
        mdl[0..4].copy_from_slice(b"IDST");
        write_i32(&mut mdl, 4, 48);
        let mdl_length = mdl.len() as i32;
        write_i32(&mut mdl, 76, mdl_length);
        write_i32(&mut mdl, 156, 1);
        write_i32(&mut mdl, 160, bone_offset as i32);
        write_i32(&mut mdl, bone_offset, (name_offset - bone_offset) as i32);
        write_i32(&mut mdl, bone_offset + 4, -1);
        write_f32(&mut mdl, bone_offset + 56, 1.0);
        write_i32(&mut mdl, bone_offset + 180, 0x0102_0304);
        write_i32(&mut mdl, bone_offset + 184, 0x1122_3344);
        mdl[name_offset..].copy_from_slice(b"root\0");

        let parsed = parse_mdl(&mdl).expect("synthetic public-contract MDL must parse");
        assert_eq!(parsed.bones[0].contents, 0x0102_0304);
    }

    #[test]
    fn accepts_only_source_compatible_tf2_mdl_versions() {
        for version in [44, 45, 46, 48] {
            let mut mdl = vec![0_u8; MDL_HEADER_BYTES];
            mdl[0..4].copy_from_slice(b"IDST");
            write_i32(&mut mdl, 4, version);
            write_i32(&mut mdl, 76, MDL_HEADER_BYTES as i32);
            assert_eq!(parse_mdl(&mdl).unwrap().version, version);
        }

        let mut mdl = vec![0_u8; MDL_HEADER_BYTES];
        mdl[0..4].copy_from_slice(b"IDST");
        write_i32(&mut mdl, 4, 49);
        write_i32(&mut mdl, 76, MDL_HEADER_BYTES as i32);
        assert!(
            parse_mdl(&mdl)
                .err()
                .unwrap()
                .contains("unsupported MDL version 49")
        );
    }

    #[test]
    fn clears_pre_v47_zero_frame_cache_metadata() {
        let animation_offset = MDL_HEADER_BYTES;
        let animation_name_offset = animation_offset + 100;
        let mut mdl = vec![0_u8; animation_name_offset + 5];
        mdl[0..4].copy_from_slice(b"IDST");
        write_i32(&mut mdl, 4, 46);
        let mdl_length = mdl.len() as i32;
        write_i32(&mut mdl, 76, mdl_length);
        write_i32(&mut mdl, 180, 1);
        write_i32(&mut mdl, 184, animation_offset as i32);
        write_i32(
            &mut mdl,
            animation_offset + 4,
            (animation_name_offset - animation_offset) as i32,
        );
        write_f32(&mut mdl, animation_offset + 8, 30.0);
        write_i32(&mut mdl, animation_offset + 16, 1);
        mdl[animation_offset + 88..animation_offset + 90].copy_from_slice(&7_i16.to_le_bytes());
        mdl[animation_offset + 90..animation_offset + 92].copy_from_slice(&9_i16.to_le_bytes());
        write_i32(&mut mdl, animation_offset + 92, 1234);
        mdl[animation_name_offset..].copy_from_slice(b"idle\0");

        let parsed = parse_mdl(&mdl).expect("v46 metadata must be converted before use");
        assert_eq!(parsed.animations[0].metadata.zero_frame_count, 0);
    }

    #[test]
    fn exports_animation_library_mdl_without_invented_geometry_companions() {
        let mut mdl = vec![0_u8; MDL_HEADER_BYTES];
        mdl[0..4].copy_from_slice(b"IDST");
        write_i32(&mut mdl, 4, 48);
        write_i32(&mut mdl, 8, 1234);
        write_i32(&mut mdl, 76, MDL_HEADER_BYTES as i32);

        let export = export_studio_metadata_model("models/test/animations.mdl", &mdl)
            .expect("metadata-only StudioModel must export");
        assert_eq!(export.manifest.source_files.len(), 1);
        assert_eq!(export.manifest.source_files[0].role, "header");
        assert_eq!(
            export.manifest.formats.geometry.status,
            StudioFeatureStatus::NotPresent
        );
        assert_eq!(
            export.manifest.formats.vvd.status,
            StudioFeatureStatus::NotPresent
        );
        assert_eq!(
            export.manifest.formats.vtx.status,
            StudioFeatureStatus::NotPresent
        );
    }

    #[test]
    fn retains_sequence_layer_and_ik_lock_contracts() {
        let sequence_offset = MDL_HEADER_BYTES;
        let layer_offset = sequence_offset + 212;
        let lock_offset = layer_offset + 24;
        let label_offset = lock_offset + 32;
        let activity_offset = label_offset + 5;
        let mut mdl = vec![0_u8; activity_offset + 9];
        mdl[0..4].copy_from_slice(b"IDST");
        write_i32(&mut mdl, 4, 48);
        let mdl_length = mdl.len() as i32;
        write_i32(&mut mdl, 76, mdl_length);
        write_i32(&mut mdl, 188, 1);
        write_i32(&mut mdl, 192, sequence_offset as i32);
        write_i32(
            &mut mdl,
            sequence_offset + 4,
            (label_offset - sequence_offset) as i32,
        );
        write_i32(
            &mut mdl,
            sequence_offset + 8,
            (activity_offset - sequence_offset) as i32,
        );
        write_i32(&mut mdl, sequence_offset + 148, 1);
        write_i32(
            &mut mdl,
            sequence_offset + 152,
            (layer_offset - sequence_offset) as i32,
        );
        mdl[layer_offset..layer_offset + 2].copy_from_slice(&3_i16.to_le_bytes());
        mdl[layer_offset + 2..layer_offset + 4].copy_from_slice(&(-1_i16).to_le_bytes());
        write_i32(&mut mdl, layer_offset + 4, 0x1040);
        write_f32(&mut mdl, layer_offset + 8, 0.1);
        write_f32(&mut mdl, layer_offset + 12, 0.2);
        write_f32(&mut mdl, layer_offset + 16, 0.8);
        write_f32(&mut mdl, layer_offset + 20, 0.9);
        write_i32(&mut mdl, sequence_offset + 164, 1);
        write_i32(
            &mut mdl,
            sequence_offset + 168,
            (lock_offset - sequence_offset) as i32,
        );
        write_i32(&mut mdl, lock_offset, 2);
        write_f32(&mut mdl, lock_offset + 4, 0.75);
        write_f32(&mut mdl, lock_offset + 8, 0.5);
        write_i32(&mut mdl, lock_offset + 12, 7);
        mdl[label_offset..label_offset + 5].copy_from_slice(b"idle\0");
        mdl[activity_offset..activity_offset + 9].copy_from_slice(b"ACT_IDLE\0");

        let sequence = &parse_mdl(&mdl).unwrap().sequences[0];
        assert_eq!(sequence.auto_layers[0].sequence, 3);
        assert_eq!(sequence.auto_layers[0].pose, -1);
        assert_eq!(sequence.auto_layers[0].flags, 0x1040);
        assert_eq!(sequence.ik_locks[0].chain, 2);
        assert_eq!(sequence.ik_locks[0].position_weight, 0.75);
        assert_eq!(sequence.ik_locks[0].local_rotation_weight, 0.5);
        assert_eq!(sequence.ik_locks[0].flags, 7);
    }

    fn vertex(position_x: f32) -> VvdVertex {
        VvdVertex {
            weights: [1.0, 0.0, 0.0],
            bones: [0; 3],
            bone_count: 1,
            position: [position_x, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            tangent: [1.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn remaps_stable_vtx_indices_through_vvd_fixups() {
        let vvd = ParsedVvd {
            checksum: 1,
            lod_vertex_counts: vec![4, 2],
            source_vertices: vec![vertex(10.0), vertex(11.0), vertex(20.0), vertex(21.0)],
            fixups: vec![
                VvdFixup {
                    lod: 0,
                    source: 0,
                    destination: 0,
                    count: 2,
                },
                VvdFixup {
                    lod: 1,
                    source: 2,
                    destination: 2,
                    count: 2,
                },
            ],
        };

        assert_eq!(vvd.vertex(0, 3).unwrap().position[0], 21.0);
        assert_eq!(vvd.vertex(1, 2).unwrap().position[0], 20.0);
        assert!(vvd.vertex(1, 0).err().unwrap().contains("culled"));
    }

    #[test]
    fn decodes_animation_rle_repeat_segments_and_rejects_zero_runs() {
        let values = [6_u8, 0, 0, 0, 0, 0, 2, 4, 10, 0, 20, 0];
        assert_eq!(animation_value(&values, 0, 0, 3, 1.0, "test").unwrap(), 0.0);
        assert_eq!(animation_value(&values, 0, 6, 0, 0.5, "test").unwrap(), 5.0);
        assert_eq!(
            animation_value(&values, 0, 6, 3, 0.5, "test").unwrap(),
            10.0
        );
        assert!(animation_value(&[1, 0, 0], 0, 1, 0, 1.0, "test").is_err());
    }

    #[test]
    fn decodes_public_compressed_identity_quaternion() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&32_768_u16.to_le_bytes());
        bytes.extend_from_slice(&32_768_u16.to_le_bytes());
        bytes.extend_from_slice(&16_384_u16.to_le_bytes());
        assert_eq!(
            compressed_quaternion(&bytes, 0, false, "test").unwrap(),
            ([0.0, 0.0, 0.0, 1.0], 6)
        );
    }

    #[test]
    fn source_root_rotation_maps_y_to_negative_gltf_z() {
        let [x, y, z, w] = SOURCE_TO_GLTF_ROTATION;
        let source_y = [0.0_f32, 1.0, 0.0];
        let dot = x * source_y[0] + y * source_y[1] + z * source_y[2];
        let cross = [
            y * source_y[2] - z * source_y[1],
            z * source_y[0] - x * source_y[2],
            x * source_y[1] - y * source_y[0],
        ];
        let rotated = [
            source_y[0] + 2.0 * (w * cross[0] + y * cross[2] - z * cross[1]),
            source_y[1] + 2.0 * (w * cross[1] + z * cross[0] - x * cross[2]),
            source_y[2] + 2.0 * (w * cross[2] + x * cross[1] - y * cross[0]),
        ];
        assert!(dot.abs() < 1.0e-6);
        assert!(rotated[0].abs() < 1.0e-6);
        assert!(rotated[1].abs() < 1.0e-6);
        assert!((rotated[2] + 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn package_identity_covers_skin_and_every_companion() {
        let base = StudioModelInput {
            source_path: "models/test/model.mdl",
            mdl: b"mdl",
            vvd: b"vvd",
            vtx: b"vtx",
            ani: Some(b"ani"),
            phy: Some(b"phy"),
            skin: 0,
        };
        let repeated = StudioModelInput { ..base };
        assert_eq!(package_content_hash(&base), package_content_hash(&repeated));
        let changed_skin = StudioModelInput { skin: 1, ..base };
        assert_ne!(
            package_content_hash(&base),
            package_content_hash(&changed_skin)
        );
        let changed_phy = StudioModelInput {
            phy: Some(b"changed"),
            ..base
        };
        assert_ne!(
            package_content_hash(&base),
            package_content_hash(&changed_phy)
        );
    }
}
