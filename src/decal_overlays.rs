use super::{
    Entity, ExtractedLightmaps, Face, Plane, SourceMaterialManifest, SourceMaterialPackage,
    TexInfo, dot, entity_property, face_positions, lightmap_uv,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const DECAL_OVERLAY_SIDECAR_VERSION: u32 = 1;
const MAX_RECORDS: usize = 16_384;
const MAX_FRAGMENTS: usize = 65_536;
const MAX_VERTICES: usize = 1_000_000;
const SURF_NODECALS: i32 = 0x2000;
const SURF_SKY2D: i32 = 0x0002;
const SURF_SKY: i32 = 0x0004;
const SURF_TRIGGER: i32 = 0x0040;
const SURF_NODRAW: i32 = 0x0080;
const SURF_HINT: i32 = 0x0100;
const SURF_SKIP: i32 = 0x0200;
const NON_DECAL_SURFACES: i32 =
    SURF_SKY2D | SURF_SKY | SURF_TRIGGER | SURF_NODRAW | SURF_HINT | SURF_SKIP | SURF_NODECALS;
const DECAL_PLANE_DISTANCE: f32 = 4.0;
const INFODECAL_TRACE_EXTENT: f32 = 5.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DecalOverlayStatus {
    Handled,
    Inert,
    Unsupported,
    Malformed,
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayCoverage {
    pub handled: usize,
    pub inert: usize,
    pub unsupported: usize,
    pub malformed: usize,
    pub unknown: usize,
}

impl DecalOverlayCoverage {
    fn add(&mut self, status: DecalOverlayStatus) {
        match status {
            DecalOverlayStatus::Handled => self.handled += 1,
            DecalOverlayStatus::Inert => self.inert += 1,
            DecalOverlayStatus::Unsupported => self.unsupported += 1,
            DecalOverlayStatus::Malformed => self.malformed += 1,
            DecalOverlayStatus::Unknown => self.unknown += 1,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayInventory {
    pub infodecals: usize,
    pub compiled_overlays: usize,
    pub water_overlays: usize,
    pub records: usize,
    pub fragments: usize,
    pub vertices: usize,
    pub triangles: usize,
    pub materials: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayBasis {
    pub u: [f32; 3],
    pub v: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayUvRange {
    pub u: [f32; 2],
    pub v: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayFade {
    pub minimum_distance_squared: f32,
    pub maximum_distance_squared: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayInitialState {
    pub enabled: bool,
    pub dynamic: bool,
    pub low_priority: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayTarget {
    pub bsp_model_index: usize,
    pub bsp_face_indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayRawKeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayFragment {
    pub bsp_model_index: usize,
    pub bsp_face_index: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub lightmap_uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayRecord {
    pub kind: String,
    pub source_index: usize,
    pub entity_index: Option<usize>,
    pub overlay_id: Option<i32>,
    pub status: DecalOverlayStatus,
    pub reason: String,
    pub material_index: Option<usize>,
    pub material_name: Option<String>,
    pub origin: Option<[f32; 3]>,
    pub basis: Option<DecalOverlayBasis>,
    pub uv_range: Option<DecalOverlayUvRange>,
    pub render_order: u16,
    pub fade: Option<DecalOverlayFade>,
    pub initial_state: DecalOverlayInitialState,
    pub target: Option<DecalOverlayTarget>,
    pub parent_entity_index: Option<usize>,
    pub raw: BTreeMap<String, String>,
    pub raw_key_values: Vec<DecalOverlayRawKeyValue>,
    pub fragments: Vec<DecalOverlayFragment>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlayUnknownLump {
    pub name: String,
    pub version: i32,
    pub byte_length: usize,
    pub bytes_base64: String,
    pub status: DecalOverlayStatus,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalOverlaySidecar {
    pub schema: String,
    pub schema_version: u32,
    pub source_bsp_version: i32,
    pub coordinate_system: String,
    pub inventory: DecalOverlayInventory,
    pub coverage: DecalOverlayCoverage,
    pub records: Vec<DecalOverlayRecord>,
    pub unknown_lumps: Vec<DecalOverlayUnknownLump>,
}

#[derive(Clone, Debug)]
struct OverlaySource {
    kind: &'static str,
    source_index: usize,
    id: i32,
    texinfo: i16,
    faces: Vec<usize>,
    face_indices_valid: bool,
    render_order: u16,
    uv_range: DecalOverlayUvRange,
    plane_points: [[f32; 2]; 4],
    origin: [f32; 3],
    basis: DecalOverlayBasis,
}

#[derive(Clone, Copy)]
struct ClipVertex {
    position: [f32; 3],
    uv: [f32; 2],
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FaceDecalEligibility {
    Accepts,
    Rejects,
    Unknown,
}

struct InfodecalProjection {
    basis: Option<DecalOverlayBasis>,
    target: Option<(usize, usize)>,
    fragments: Vec<DecalOverlayFragment>,
    unknown_receiver: bool,
}

#[derive(Clone, Copy)]
struct ModelTransform {
    origin: [f32; 3],
    rotation: [[f32; 3]; 3],
}

impl ModelTransform {
    fn identity() -> Self {
        Self {
            origin: [0.0; 3],
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    fn from_entity(entity: &Entity) -> Self {
        let [pitch, yaw, roll] = entity_vector(entity, "angles")
            .unwrap_or([0.0; 3])
            .map(f32::to_radians);
        let (sp, cp) = pitch.sin_cos();
        let (sy, cy) = yaw.sin_cos();
        let (sr, cr) = roll.sin_cos();
        Self {
            origin: entity_vector(entity, "origin").unwrap_or([0.0; 3]),
            rotation: [
                [cp * cy, sr * sp * cy - cr * sy, cr * sp * cy + sr * sy],
                [cp * sy, sr * sp * sy + cr * cy, cr * sp * sy - sr * cy],
                [-sp, sr * cp, cr * cp],
            ],
        }
    }

    fn vector_to_world(self, value: [f32; 3]) -> [f32; 3] {
        [
            dot(self.rotation[0], value),
            dot(self.rotation[1], value),
            dot(self.rotation[2], value),
        ]
    }

    fn point_to_world(self, value: [f32; 3]) -> [f32; 3] {
        add(self.origin, self.vector_to_world(value))
    }

    fn point_to_local(self, value: [f32; 3]) -> [f32; 3] {
        let value = sub(value, self.origin);
        [
            value[0] * self.rotation[0][0]
                + value[1] * self.rotation[1][0]
                + value[2] * self.rotation[2][0],
            value[0] * self.rotation[0][1]
                + value[1] * self.rotation[1][1]
                + value[2] * self.rotation[2][1],
            value[0] * self.rotation[0][2]
                + value[1] * self.rotation[1][2]
                + value[2] * self.rotation[2][2],
        ]
    }
}

pub(crate) struct BuildInput<'a> {
    pub bsp_version: i32,
    pub entities: &'a [Entity],
    pub overlay_data: &'a [u8],
    pub overlay_version: i32,
    pub water_overlay_data: &'a [u8],
    pub water_overlay_version: i32,
    pub overlay_fades: &'a [u8],
    pub planes: &'a [Plane],
    pub faces: &'a [Face],
    pub face_owner: &'a [Option<usize>],
    pub texinfos: &'a [TexInfo],
    pub material_names: &'a [String],
    pub material_manifest: &'a SourceMaterialManifest,
    pub material_textures: Option<&'a SourceMaterialPackage>,
    pub surfedges: &'a [i32],
    pub edges: &'a [[u16; 2]],
    pub vertices: &'a [[f32; 3]],
    pub lightmaps: Option<&'a ExtractedLightmaps>,
}

fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_i32(data: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_f32(data: &[u8], offset: usize) -> Option<f32> {
    let value = f32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?);
    value.is_finite().then_some(value)
}

fn vector(data: &[u8], offset: usize) -> Option<[f32; 3]> {
    Some([
        read_f32(data, offset)?,
        read_f32(data, offset + 4)?,
        read_f32(data, offset + 8)?,
    ])
}

fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize(value: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(value, value).sqrt();
    (length > 1e-6 && length.is_finite()).then(|| scale(value, length.recip()))
}

fn entity_vector(entity: &Entity, name: &str) -> Option<[f32; 3]> {
    let values: Vec<_> = entity_property(entity, name)?.split_whitespace().collect();
    if values.len() != 3 {
        return None;
    }
    let parsed = [
        values[0].parse::<f32>().ok()?,
        values[1].parse::<f32>().ok()?,
        values[2].parse::<f32>().ok()?,
    ];
    parsed
        .iter()
        .all(|value| value.is_finite())
        .then_some(parsed)
}

fn raw_entity(entity: &Entity) -> (BTreeMap<String, String>, Vec<DecalOverlayRawKeyValue>) {
    let mut raw = BTreeMap::new();
    let mut ordered = Vec::with_capacity(entity.len());
    for property in entity {
        raw.entry(property.key.clone())
            .or_insert_with(|| property.value.clone());
        ordered.push(DecalOverlayRawKeyValue {
            key: property.key.clone(),
            value: property.value.clone(),
        });
    }
    (raw, ordered)
}

fn parse_overlay_lump(
    data: &[u8],
    version: i32,
    water: bool,
) -> (Vec<OverlaySource>, Option<DecalOverlayUnknownLump>) {
    let (name, kind, record_size, max_faces) = if water {
        ("WATEROVERLAYS", "waterOverlay", 1120, 256)
    } else {
        ("OVERLAYS", "overlay", 352, 64)
    };
    if data.is_empty() {
        return (Vec::new(), None);
    }
    if version != 0 || !data.len().is_multiple_of(record_size) {
        let status = if version != 0 {
            DecalOverlayStatus::Unknown
        } else {
            DecalOverlayStatus::Malformed
        };
        return (
            Vec::new(),
            Some(DecalOverlayUnknownLump {
                name: name.to_owned(),
                version,
                byte_length: data.len(),
                bytes_base64: BASE64.encode(data),
                status,
                reason: if version != 0 {
                    "lump-version-unknown".to_owned()
                } else {
                    "record-size-malformed".to_owned()
                },
            }),
        );
    }

    let expected_records = data.len() / record_size;
    let mut overlays = Vec::with_capacity(expected_records);
    for (source_index, record) in data.chunks_exact(record_size).enumerate() {
        let Some(id) = read_i32(record, 0) else {
            continue;
        };
        let Some(texinfo) = read_i16(record, 4) else {
            continue;
        };
        let Some(packed) = read_u16(record, 6) else {
            continue;
        };
        let face_count = usize::from(packed & 0x3fff);
        if face_count > max_faces {
            continue;
        }
        let raw_faces: Vec<_> = (0..face_count)
            .filter_map(|index| read_i32(record, 8 + index * 4))
            .collect();
        let face_indices_valid =
            raw_faces.len() == face_count && raw_faces.iter().all(|face| *face >= 0);
        let faces = raw_faces
            .into_iter()
            .filter_map(|value| usize::try_from(value).ok())
            .collect();
        let vectors = 8 + max_faces * 4;
        let Some(uv_range) = (|| {
            Some(DecalOverlayUvRange {
                u: [read_f32(record, vectors)?, read_f32(record, vectors + 4)?],
                v: [
                    read_f32(record, vectors + 8)?,
                    read_f32(record, vectors + 12)?,
                ],
            })
        })() else {
            continue;
        };
        let mut points = [[0.0; 2]; 4];
        let mut encoded = [[0.0; 3]; 4];
        let mut valid = true;
        for index in 0..4 {
            if let Some(value) = vector(record, vectors + 16 + index * 12) {
                points[index] = [value[0], value[1]];
                encoded[index] = value;
            } else {
                valid = false;
            }
        }
        let Some(origin) = vector(record, vectors + 64) else {
            continue;
        };
        let Some(normal) = vector(record, vectors + 76).and_then(normalize) else {
            continue;
        };
        if !valid {
            continue;
        }
        let Some(u) = normalize([encoded[0][2], encoded[1][2], encoded[2][2]]) else {
            continue;
        };
        let mut v = cross(normal, u);
        if encoded[3][2] == 1.0 {
            v = scale(v, -1.0);
        }
        let Some(v) = normalize(v) else { continue };
        overlays.push(OverlaySource {
            kind,
            source_index,
            id,
            texinfo,
            faces,
            face_indices_valid,
            render_order: packed >> 14,
            uv_range,
            plane_points: points,
            origin,
            basis: DecalOverlayBasis { u, v, normal },
        });
    }
    let malformed = (overlays.len() != expected_records).then(|| DecalOverlayUnknownLump {
        name: name.to_owned(),
        version,
        byte_length: data.len(),
        bytes_base64: BASE64.encode(data),
        status: DecalOverlayStatus::Malformed,
        reason: "record-fields-malformed".to_owned(),
    });
    (overlays, malformed)
}

pub(crate) fn collect_additional_material_names(
    entities: &[Entity],
    overlay_data: &[u8],
    overlay_version: i32,
    water_overlay_data: &[u8],
    water_overlay_version: i32,
    texinfos: &[TexInfo],
    material_names: &[String],
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen: BTreeSet<String> = material_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    for entity in entities {
        if !entity_property(entity, "classname")
            .is_some_and(|value| value.eq_ignore_ascii_case("infodecal"))
        {
            continue;
        }
        if let Some(name) = entity_property(entity, "texture") {
            let normalized = name.trim().replace('\\', "/");
            if !normalized.is_empty() && seen.insert(normalized.to_ascii_lowercase()) {
                names.push(normalized);
            }
        }
    }
    for source in parse_overlay_lump(overlay_data, overlay_version, false)
        .0
        .into_iter()
        .chain(parse_overlay_lump(water_overlay_data, water_overlay_version, true).0)
    {
        let Some(texinfo) = usize::try_from(source.texinfo)
            .ok()
            .and_then(|index| texinfos.get(index))
        else {
            continue;
        };
        let Some(name) = usize::try_from(texinfo.texdata)
            .ok()
            .and_then(|index| material_names.get(index))
        else {
            continue;
        };
        if seen.insert(name.to_ascii_lowercase()) {
            names.push(name.clone());
        }
    }
    names
}

fn material_index(manifest: &SourceMaterialManifest, name: &str) -> Option<usize> {
    manifest
        .materials
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
        .map(|entry| entry.material_index)
}

fn decal_world_dimension(pixels: u32, scale: f32) -> Option<f32> {
    let world = pixels as f32 * scale;
    (world >= 1.0 && world <= i32::MAX as f32).then(|| world.trunc())
}

fn decal_material_dimensions(
    manifest: &SourceMaterialManifest,
    package: Option<&SourceMaterialPackage>,
    material_index: usize,
) -> Option<(f32, f32)> {
    let material = manifest
        .materials
        .iter()
        .find(|entry| entry.material_index == material_index)?;
    let metadata = material.metadata.as_ref()?;
    let scale = metadata
        .shader
        .inputs
        .get("$decalscale")
        .map(|value| value.parse::<f32>().ok())
        .unwrap_or(Some(1.0))?;
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let source_index = material
        .textures
        .iter()
        .find(|texture| texture.role == "baseTexture")?
        .package_source_index?;
    let source = package?.manifest.sources.get(source_index)?;
    let texture = source.metadata.as_ref()?;
    Some((
        decal_world_dimension(texture.width, scale)?,
        decal_world_dimension(texture.height, scale)?,
    ))
}

fn decal_basis(normal: [f32; 3]) -> Option<DecalOverlayBasis> {
    let normal = normalize(normal)?;
    let (u, v) = if normal[2].abs() > std::f32::consts::FRAC_1_SQRT_2 {
        let v = normalize(cross([1.0, 0.0, 0.0], normal))?;
        (normalize(cross(normal, v))?, v)
    } else {
        let u = normalize(cross(normal, [0.0, 0.0, -1.0]))?;
        (u, normalize(cross(u, normal))?)
    };
    Some(DecalOverlayBasis { u, v, normal })
}

fn plane_for_face(planes: &[Plane], face: Face) -> Option<Plane> {
    planes.get(face.plane).copied()
}

fn model_entity_index(input: &BuildInput<'_>, model_index: usize) -> Option<usize> {
    input
        .entities
        .iter()
        .enumerate()
        .find(|(_, entity)| {
            entity_property(entity, "model")
                .and_then(|value| value.strip_prefix('*'))
                .and_then(|value| value.parse::<usize>().ok())
                == Some(model_index)
        })
        .map(|(index, _)| index)
}

fn model_transform(input: &BuildInput<'_>, model_index: usize) -> ModelTransform {
    if model_index == 0 {
        ModelTransform::identity()
    } else {
        model_entity_index(input, model_index)
            .map(|index| ModelTransform::from_entity(&input.entities[index]))
            .unwrap_or_else(ModelTransform::identity)
    }
}

fn model_is_traceable(input: &BuildInput<'_>, model_index: usize) -> bool {
    if model_index == 0 {
        return true;
    }
    let Some(entity_index) = model_entity_index(input, model_index) else {
        return true;
    };
    let entity = &input.entities[entity_index];
    if entity_property(entity, "effects")
        .and_then(|value| value.parse::<i32>().ok())
        .is_some_and(|effects| effects & 0x20 != 0)
    {
        return false;
    }
    let classname = entity_property(entity, "classname")
        .unwrap_or_default()
        .to_ascii_lowercase();
    !(classname.starts_with("weapon_")
        || classname.starts_with("item_")
        || matches!(
            classname.as_str(),
            "prop_ragdoll" | "prop_dynamic" | "prop_static" | "prop_physics" | "npc_bullseye"
        ))
}

fn segment_face_intersection(
    start: [f32; 3],
    end: [f32; 3],
    polygon: &[[f32; 3]],
    normal: [f32; 3],
) -> Option<f32> {
    let direction = sub(end, start);
    let denominator = dot(direction, normal);
    if denominator.abs() <= 1e-6 {
        return None;
    }
    let amount = dot(sub(polygon.first().copied()?, start), normal) / denominator;
    if !(0.0..=1.0).contains(&amount) {
        return None;
    }
    point_in_polygon(add(start, scale(direction, amount)), polygon, normal).then_some(amount)
}

fn infodecal_target(
    input: &BuildInput<'_>,
    origin: [f32; 3],
) -> Option<(usize, usize, ModelTransform)> {
    let extent = [INFODECAL_TRACE_EXTENT; 3];
    let start = sub(origin, extent);
    let end = add(origin, extent);
    let mut targets = Vec::new();
    for (face_index, face) in input.faces.iter().copied().enumerate() {
        // Exact displacement collision requires its compiled displaced triangles, not its base face.
        if face.dispinfo >= 0 {
            continue;
        }
        let Some(model_index) = input.face_owner.get(face_index).copied().flatten() else {
            continue;
        };
        if !model_is_traceable(input, model_index) {
            continue;
        }
        let Some(plane) = plane_for_face(input.planes, face) else {
            continue;
        };
        let Ok(local_polygon) = face_positions(
            face,
            input.surfedges,
            input.edges,
            input.vertices,
            face_index,
        ) else {
            continue;
        };
        let transform = model_transform(input, model_index);
        let polygon: Vec<_> = local_polygon
            .into_iter()
            .map(|point| transform.point_to_world(point))
            .collect();
        let normal = transform.vector_to_world(plane.normal);
        let Some(amount) = segment_face_intersection(start, end, &polygon, normal) else {
            continue;
        };
        targets.push((amount, model_index, face_index, transform));
    }
    targets
        .into_iter()
        .min_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| (left.1 != 0).cmp(&(right.1 != 0)))
                .then(left.1.cmp(&right.1))
                .then(left.2.cmp(&right.2))
        })
        .map(|(_, model, face, transform)| (model, face, transform))
}

fn face_decal_eligibility(
    input: &BuildInput<'_>,
    face: Face,
    texinfo: &TexInfo,
) -> FaceDecalEligibility {
    if texinfo.flags & NON_DECAL_SURFACES != 0 || face.dispinfo >= 0 {
        return FaceDecalEligibility::Rejects;
    }
    let Some(material_index) = usize::try_from(texinfo.texdata).ok() else {
        return FaceDecalEligibility::Unknown;
    };
    let Some(material) = input
        .material_manifest
        .materials
        .iter()
        .find(|entry| entry.material_index == material_index)
    else {
        return FaceDecalEligibility::Unknown;
    };
    let Some(metadata) = &material.metadata else {
        return FaceDecalEligibility::Unknown;
    };
    if metadata.features.alpha_test
        || metadata
            .shader
            .inputs
            .get("$nodecal")
            .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true"))
    {
        FaceDecalEligibility::Rejects
    } else {
        FaceDecalEligibility::Accepts
    }
}

fn projected_point(point: [f32; 3], plane: Plane) -> [f32; 3] {
    let distance = dot(point, plane.normal) - plane.distance;
    sub(point, scale(plane.normal, distance))
}

fn point_in_polygon(point: [f32; 3], polygon: &[[f32; 3]], normal: [f32; 3]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let orientation = dot(
        cross(sub(polygon[1], polygon[0]), sub(polygon[2], polygon[1])),
        normal,
    );
    polygon.iter().enumerate().all(|(index, start)| {
        let end = polygon[(index + 1) % polygon.len()];
        dot(cross(sub(end, *start), sub(point, *start)), normal) * orientation >= -0.01
    })
}

fn interpolate(left: ClipVertex, right: ClipVertex, amount: f32) -> ClipVertex {
    ClipVertex {
        position: add(
            left.position,
            scale(sub(right.position, left.position), amount),
        ),
        uv: [
            left.uv[0] + (right.uv[0] - left.uv[0]) * amount,
            left.uv[1] + (right.uv[1] - left.uv[1]) * amount,
        ],
    }
}

fn clip_polygon(
    mut subject: Vec<ClipVertex>,
    clip: &[[f32; 3]],
    normal: [f32; 3],
) -> Vec<ClipVertex> {
    if clip.len() < 3 {
        return Vec::new();
    }
    let orientation = dot(cross(sub(clip[1], clip[0]), sub(clip[2], clip[1])), normal).signum();
    for edge_index in 0..clip.len() {
        let start = clip[edge_index];
        let end = clip[(edge_index + 1) % clip.len()];
        let edge = sub(end, start);
        let distance = |point: [f32; 3]| dot(cross(edge, sub(point, start)), normal) * orientation;
        let input = std::mem::take(&mut subject);
        if input.is_empty() {
            break;
        }
        let mut previous = *input.last().expect("non-empty clip input");
        let mut previous_distance = distance(previous.position);
        for current in input {
            let current_distance = distance(current.position);
            let previous_inside = previous_distance >= -0.001;
            let current_inside = current_distance >= -0.001;
            if previous_inside != current_inside {
                let denominator = previous_distance - current_distance;
                if denominator.abs() > 1e-8 {
                    subject.push(interpolate(
                        previous,
                        current,
                        previous_distance / denominator,
                    ));
                }
            }
            if current_inside {
                subject.push(current);
            }
            previous = current;
            previous_distance = current_distance;
        }
    }
    subject
}

fn fragment(
    face_index: usize,
    model_index: usize,
    polygon: Vec<ClipVertex>,
    plane: Plane,
    face: Face,
    texinfo: &TexInfo,
    lightmaps: Option<&ExtractedLightmaps>,
) -> Option<DecalOverlayFragment> {
    if polygon.len() < 3 {
        return None;
    }
    let mut indices = Vec::with_capacity((polygon.len() - 2) * 3);
    for index in 1..polygon.len() - 1 {
        indices.extend_from_slice(&[0, index as u32, index as u32 + 1]);
    }
    if indices
        .chunks_exact(3)
        .find_map(|triangle| {
            let a = polygon[triangle[0] as usize].position;
            let b = polygon[triangle[1] as usize].position;
            let c = polygon[triangle[2] as usize].position;
            let facing = dot(cross(sub(b, a), sub(c, a)), plane.normal);
            (facing.abs() > 1e-8).then_some(facing)
        })
        .is_some_and(|facing| facing < 0.0)
    {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    let lightmap_uvs = polygon
        .iter()
        .map(|vertex| {
            lightmaps
                .and_then(|extracted| {
                    let placement = extracted.by_face.get(&face_index).copied()?;
                    lightmap_uv(
                        placement,
                        &extracted.artifacts.flat,
                        face,
                        texinfo,
                        vertex.position,
                        face_index,
                    )
                    .ok()
                })
                .unwrap_or([0.0, 0.0])
        })
        .collect();
    Some(DecalOverlayFragment {
        bsp_model_index: model_index,
        bsp_face_index: face_index,
        positions: polygon.iter().map(|vertex| vertex.position).collect(),
        normals: vec![plane.normal; polygon.len()],
        uvs: polygon.iter().map(|vertex| vertex.uv).collect(),
        lightmap_uvs,
        indices,
    })
}

fn infodecal_fragments(
    input: &BuildInput<'_>,
    origin: [f32; 3],
    width: f32,
    height: f32,
) -> InfodecalProjection {
    let Some((target_model, target_face, transform)) = infodecal_target(input, origin) else {
        return InfodecalProjection {
            basis: None,
            target: None,
            fragments: Vec::new(),
            unknown_receiver: false,
        };
    };
    let local_origin = transform.point_to_local(origin);
    let target_basis = input
        .faces
        .get(target_face)
        .copied()
        .and_then(|face| plane_for_face(input.planes, face))
        .and_then(|plane| decal_basis(plane.normal));
    let mut fragments = Vec::new();
    let mut unknown_receiver = false;
    for (face_index, face) in input.faces.iter().copied().enumerate() {
        let Some(model_index) = input.face_owner.get(face_index).copied().flatten() else {
            continue;
        };
        if model_index != target_model {
            continue;
        }
        let Some(texinfo) = usize::try_from(face.texinfo)
            .ok()
            .and_then(|index| input.texinfos.get(index))
        else {
            continue;
        };
        let Some(plane) = plane_for_face(input.planes, face) else {
            continue;
        };
        let distance = dot(local_origin, plane.normal) - plane.distance;
        if distance.abs() >= DECAL_PLANE_DISTANCE {
            continue;
        }
        let Some(basis) = decal_basis(plane.normal) else {
            continue;
        };
        let center = projected_point(local_origin, plane);
        let half_u = scale(basis.u, width * 0.5);
        let half_v = scale(basis.v, height * 0.5);
        let subject = vec![
            ClipVertex {
                position: sub(sub(center, half_u), half_v),
                uv: [0.0, 0.0],
            },
            ClipVertex {
                position: add(sub(center, half_v), half_u),
                uv: [1.0, 0.0],
            },
            ClipVertex {
                position: add(add(center, half_u), half_v),
                uv: [1.0, 1.0],
            },
            ClipVertex {
                position: add(sub(center, half_u), half_v),
                uv: [0.0, 1.0],
            },
        ];
        let Ok(face_polygon) = face_positions(
            face,
            input.surfedges,
            input.edges,
            input.vertices,
            face_index,
        ) else {
            continue;
        };
        let polygon = clip_polygon(subject, &face_polygon, plane.normal);
        if polygon.len() < 3 {
            continue;
        }
        match face_decal_eligibility(input, face, texinfo) {
            FaceDecalEligibility::Accepts => {}
            FaceDecalEligibility::Rejects => continue,
            FaceDecalEligibility::Unknown => {
                unknown_receiver = true;
                continue;
            }
        }
        if let Some(item) = fragment(
            face_index,
            model_index,
            polygon,
            plane,
            face,
            texinfo,
            input.lightmaps,
        ) {
            fragments.push(item);
        }
    }
    if unknown_receiver {
        fragments.clear();
    }
    InfodecalProjection {
        basis: target_basis,
        target: Some((target_model, target_face)),
        fragments,
        unknown_receiver,
    }
}

fn overlay_fragments(input: &BuildInput<'_>, source: &OverlaySource) -> Vec<DecalOverlayFragment> {
    let overlay_polygon: Vec<_> = source
        .plane_points
        .iter()
        .enumerate()
        .map(|(index, point)| ClipVertex {
            position: add(
                add(source.origin, scale(source.basis.u, point[0])),
                scale(source.basis.v, point[1]),
            ),
            uv: match index {
                0 => [source.uv_range.u[0], source.uv_range.v[0]],
                1 => [source.uv_range.u[0], source.uv_range.v[1]],
                2 => [source.uv_range.u[1], source.uv_range.v[1]],
                _ => [source.uv_range.u[1], source.uv_range.v[0]],
            },
        })
        .collect();
    let mut fragments = Vec::new();
    for &face_index in &source.faces {
        let Some(face) = input.faces.get(face_index).copied() else {
            continue;
        };
        if face.dispinfo >= 0 {
            continue;
        }
        let Some(model_index) = input.face_owner.get(face_index).copied().flatten() else {
            continue;
        };
        let Some(texinfo) = usize::try_from(face.texinfo)
            .ok()
            .and_then(|index| input.texinfos.get(index))
        else {
            continue;
        };
        let Some(plane) = plane_for_face(input.planes, face) else {
            continue;
        };
        let denominator = dot(plane.normal, source.basis.normal);
        if denominator.abs() < 1e-5 {
            continue;
        }
        let projected: Vec<_> = overlay_polygon
            .iter()
            .map(|vertex| {
                let amount = (plane.distance - dot(plane.normal, vertex.position)) / denominator;
                ClipVertex {
                    position: add(vertex.position, scale(source.basis.normal, amount)),
                    uv: vertex.uv,
                }
            })
            .collect();
        let Ok(face_polygon) = face_positions(
            face,
            input.surfedges,
            input.edges,
            input.vertices,
            face_index,
        ) else {
            continue;
        };
        let polygon = clip_polygon(projected, &face_polygon, plane.normal);
        if let Some(item) = fragment(
            face_index,
            model_index,
            polygon,
            plane,
            face,
            texinfo,
            input.lightmaps,
        ) {
            fragments.push(item);
        }
    }
    fragments
}

fn parent_entity_index(input: &BuildInput<'_>, model_index: usize) -> Option<usize> {
    input
        .entities
        .iter()
        .enumerate()
        .find_map(|(index, entity)| {
            entity_property(entity, "model")
                .is_some_and(|value| value == format!("*{model_index}"))
                .then_some(index)
        })
}

pub(crate) fn build(input: BuildInput<'_>) -> Result<DecalOverlaySidecar, String> {
    let (overlays, overlay_unknown) =
        parse_overlay_lump(input.overlay_data, input.overlay_version, false);
    let (water_overlays, water_unknown) =
        parse_overlay_lump(input.water_overlay_data, input.water_overlay_version, true);
    let infodecal_count = input
        .entities
        .iter()
        .filter(|entity| {
            entity_property(entity, "classname")
                .is_some_and(|value| value.eq_ignore_ascii_case("infodecal"))
        })
        .count();
    if infodecal_count + overlays.len() + water_overlays.len() > MAX_RECORDS {
        return Err(format!(
            "decal/overlay record limit exceeded (maximum {MAX_RECORDS})"
        ));
    }
    let mut records = Vec::new();
    for (entity_index, entity) in input.entities.iter().enumerate() {
        if !entity_property(entity, "classname")
            .is_some_and(|value| value.eq_ignore_ascii_case("infodecal"))
        {
            continue;
        }
        let (raw, raw_key_values) = raw_entity(entity);
        let origin = entity_vector(entity, "origin");
        let material_name = entity_property(entity, "texture")
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        let index = material_name
            .as_deref()
            .and_then(|name| material_index(input.material_manifest, name));
        let dimensions = index.and_then(|material_index| {
            decal_material_dimensions(
                input.material_manifest,
                input.material_textures,
                material_index,
            )
        });
        let InfodecalProjection {
            basis,
            target: traced_target,
            fragments,
            unknown_receiver,
        } = match (origin, dimensions) {
            (Some(origin), Some((width, height))) => {
                infodecal_fragments(&input, origin, width, height)
            }
            _ => InfodecalProjection {
                basis: None,
                target: None,
                fragments: Vec::new(),
                unknown_receiver: false,
            },
        };
        let (status, reason) = if origin.is_none() || material_name.is_none() {
            (DecalOverlayStatus::Malformed, "entity-fields-malformed")
        } else if index.is_none()
            || index.is_some_and(|material_index| {
                input
                    .material_manifest
                    .materials
                    .iter()
                    .find(|entry| entry.material_index == material_index)
                    .and_then(|entry| entry.metadata.as_ref())
                    .is_none()
            })
        {
            (DecalOverlayStatus::Unsupported, "material-unresolved")
        } else if dimensions.is_none() {
            (
                DecalOverlayStatus::Unsupported,
                "material-dimensions-unavailable",
            )
        } else if unknown_receiver {
            (
                DecalOverlayStatus::Unsupported,
                "target-material-eligibility-unresolved",
            )
        } else if fragments.is_empty() && traced_target.is_some() {
            (
                DecalOverlayStatus::Inert,
                "target-surface-does-not-accept-decals",
            )
        } else if fragments.is_empty() {
            (DecalOverlayStatus::Unsupported, "target-face-unresolved")
        } else {
            (DecalOverlayStatus::Handled, "projected-to-compiled-faces")
        };
        let target_model = traced_target.map(|(model, _)| model);
        records.push(DecalOverlayRecord {
            kind: "infodecal".to_owned(),
            source_index: entity_index,
            entity_index: Some(entity_index),
            overlay_id: None,
            status,
            reason: reason.to_owned(),
            material_index: index,
            material_name,
            origin,
            basis,
            uv_range: Some(DecalOverlayUvRange {
                u: [0.0, 1.0],
                v: [0.0, 1.0],
            }),
            render_order: 0,
            fade: None,
            initial_state: DecalOverlayInitialState {
                enabled: entity_property(entity, "targetname").is_none(),
                dynamic: entity_property(entity, "targetname").is_some(),
                low_priority: entity_property(entity, "LowPriority")
                    .is_some_and(|value| value == "1"),
            },
            target: traced_target.map(|(bsp_model_index, traced_face)| DecalOverlayTarget {
                bsp_model_index,
                bsp_face_indices: if fragments.is_empty() {
                    vec![traced_face]
                } else {
                    fragments.iter().map(|item| item.bsp_face_index).collect()
                },
            }),
            parent_entity_index: target_model.and_then(|model| parent_entity_index(&input, model)),
            raw,
            raw_key_values,
            fragments,
        });
    }

    let fade_values: Vec<_> = input
        .overlay_fades
        .chunks_exact(8)
        .filter_map(|record| Some([read_f32(record, 0)?, read_f32(record, 4)?]))
        .collect();
    for source in overlays.iter().chain(&water_overlays) {
        let material_name = usize::try_from(source.texinfo)
            .ok()
            .and_then(|index| input.texinfos.get(index))
            .and_then(|texinfo| usize::try_from(texinfo.texdata).ok())
            .and_then(|index| input.material_names.get(index))
            .cloned();
        let index = material_name
            .as_deref()
            .and_then(|name| material_index(input.material_manifest, name));
        let invalid_face = !source.face_indices_valid
            || source.faces.is_empty()
            || source.faces.iter().any(|face| *face >= input.faces.len());
        let invalid_texinfo = usize::try_from(source.texinfo)
            .ok()
            .is_none_or(|texinfo| texinfo >= input.texinfos.len());
        let fragments = if index.is_some() && !invalid_face && !invalid_texinfo {
            overlay_fragments(&input, source)
        } else {
            Vec::new()
        };
        let (status, reason) = if invalid_face {
            (DecalOverlayStatus::Malformed, "target-face-malformed")
        } else if invalid_texinfo {
            (DecalOverlayStatus::Malformed, "material-index-malformed")
        } else if index.is_none() {
            (DecalOverlayStatus::Unsupported, "material-unresolved")
        } else if fragments.is_empty() {
            (
                DecalOverlayStatus::Unsupported,
                "compiled-fragments-unavailable",
            )
        } else {
            (DecalOverlayStatus::Handled, "clipped-to-compiled-faces")
        };
        let model_index = source
            .faces
            .first()
            .and_then(|face| input.face_owner.get(*face).copied().flatten());
        let fade = (source.kind == "overlay")
            .then(|| fade_values.get(source.source_index).copied())
            .flatten()
            .map(|value| DecalOverlayFade {
                minimum_distance_squared: value[0],
                maximum_distance_squared: value[1],
            });
        records.push(DecalOverlayRecord {
            kind: source.kind.to_owned(),
            source_index: source.source_index,
            entity_index: None,
            overlay_id: Some(source.id),
            status,
            reason: reason.to_owned(),
            material_index: index,
            material_name,
            origin: Some(source.origin),
            basis: Some(source.basis.clone()),
            uv_range: Some(source.uv_range.clone()),
            render_order: source.render_order,
            fade,
            initial_state: DecalOverlayInitialState {
                enabled: true,
                dynamic: false,
                low_priority: false,
            },
            target: model_index.map(|bsp_model_index| DecalOverlayTarget {
                bsp_model_index,
                bsp_face_indices: source.faces.clone(),
            }),
            parent_entity_index: model_index.and_then(|model| parent_entity_index(&input, model)),
            raw: BTreeMap::new(),
            raw_key_values: Vec::new(),
            fragments,
        });
    }

    let unknown_lumps: Vec<_> = [overlay_unknown, water_unknown]
        .into_iter()
        .flatten()
        .collect();
    let mut coverage = DecalOverlayCoverage::default();
    for record in &records {
        coverage.add(record.status);
    }
    for lump in &unknown_lumps {
        coverage.add(lump.status);
    }
    let fragments = records.iter().map(|record| record.fragments.len()).sum();
    let vertices = records
        .iter()
        .flat_map(|record| &record.fragments)
        .map(|fragment| fragment.positions.len())
        .sum();
    let triangles = records
        .iter()
        .flat_map(|record| &record.fragments)
        .map(|fragment| fragment.indices.len() / 3)
        .sum();
    if fragments > MAX_FRAGMENTS || vertices > MAX_VERTICES {
        return Err("decal/overlay geometry limit exceeded".to_owned());
    }
    let material_count = records
        .iter()
        .filter_map(|record| record.material_index)
        .collect::<BTreeSet<_>>()
        .len();
    let inventory = DecalOverlayInventory {
        infodecals: infodecal_count,
        compiled_overlays: if input.overlay_version == 0 {
            input.overlay_data.len() / 352
        } else {
            0
        },
        water_overlays: if input.water_overlay_version == 0 {
            input.water_overlay_data.len() / 1120
        } else {
            0
        },
        records: records.len(),
        fragments,
        vertices,
        triangles,
        materials: material_count,
    };
    Ok(DecalOverlaySidecar {
        schema: "bsp-to-glb.decal-overlays".to_owned(),
        schema_version: DECAL_OVERLAY_SIDECAR_VERSION,
        source_bsp_version: input.bsp_version,
        coordinate_system: "Source XYZ".to_owned(),
        inventory,
        coverage,
        records,
        unknown_lumps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EntityProperty, Face, ManifestResource, MaterialLimitations, Plane, ResourceProvenance,
        SourceMaterialEntry, SourceMaterialManifest, TexInfo, UnsupportedMaterialFeatures,
        VmtFeatures, VmtMaterial, VmtShaderMetadata, VmtTextureInputs,
    };

    #[derive(Default)]
    struct GeometryFixture {
        planes: Vec<Plane>,
        faces: Vec<Face>,
        face_owner: Vec<Option<usize>>,
        texinfos: Vec<TexInfo>,
        surfedges: Vec<i32>,
        edges: Vec<[u16; 2]>,
        vertices: Vec<[f32; 3]>,
        entities: Vec<Entity>,
    }

    impl GeometryFixture {
        fn add_face(
            &mut self,
            model: usize,
            normal: [f32; 3],
            distance: f32,
            positions: [[f32; 3]; 4],
            flags: i32,
            displacement: bool,
        ) {
            let plane = self.planes.len();
            self.planes.push(Plane {
                normal,
                distance,
                plane_type: 0,
            });
            let first_edge = self.surfedges.len() as i32;
            let first_vertex = self.vertices.len();
            self.vertices.extend(positions);
            for index in 0..4 {
                let edge = self.edges.len();
                self.edges.push([
                    (first_vertex + index) as u16,
                    (first_vertex + (index + 1) % 4) as u16,
                ]);
                self.surfedges.push(edge as i32);
            }
            let texinfo = self.texinfos.len() as i16;
            self.texinfos.push(TexInfo {
                texture_vecs: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]],
                lightmap_vecs: [[0.0; 4]; 2],
                flags,
                texdata: 0,
            });
            self.faces.push(Face {
                plane,
                _side: false,
                first_edge,
                num_edges: 4,
                texinfo,
                dispinfo: if displacement { 0 } else { -1 },
                styles: [255; 4],
                light_offset: -1,
                lightmap_mins: [0; 2],
                lightmap_size: [0; 2],
                num_primitives: 0,
                first_primitive: 0,
            });
            self.face_owner.push(Some(model));
        }

        fn add_model_entity(&mut self, model: usize, origin: &str, angles: &str) {
            self.entities.push(vec![
                EntityProperty {
                    key: "classname".to_owned(),
                    value: "func_button".to_owned(),
                },
                EntityProperty {
                    key: "model".to_owned(),
                    value: format!("*{model}"),
                },
                EntityProperty {
                    key: "origin".to_owned(),
                    value: origin.to_owned(),
                },
                EntityProperty {
                    key: "angles".to_owned(),
                    value: angles.to_owned(),
                },
            ]);
        }

        fn input<'a>(&'a self, manifest: &'a SourceMaterialManifest) -> BuildInput<'a> {
            BuildInput {
                bsp_version: 20,
                entities: &self.entities,
                overlay_data: &[],
                overlay_version: 0,
                water_overlay_data: &[],
                water_overlay_version: 0,
                overlay_fades: &[],
                planes: &self.planes,
                faces: &self.faces,
                face_owner: &self.face_owner,
                texinfos: &self.texinfos,
                material_names: &[],
                material_manifest: manifest,
                material_textures: None,
                surfedges: &self.surfedges,
                edges: &self.edges,
                vertices: &self.vertices,
                lightmaps: None,
            }
        }
    }

    fn empty_manifest() -> SourceMaterialManifest {
        SourceMaterialManifest {
            schema_version: 1,
            lookup_policy: String::new(),
            materials: Vec::new(),
            embedded_resources: Vec::new(),
            unresolved_assets: Vec::new(),
            limitations: MaterialLimitations {
                vtf_pixel_conversion: String::new(),
                proxies: String::new(),
                animated_materials: String::new(),
            },
        }
    }

    fn receiving_material_manifest(alpha_test: bool, nodecal: bool) -> SourceMaterialManifest {
        let mut manifest = empty_manifest();
        manifest.materials.push(SourceMaterialEntry {
            material_index: 0,
            name: "fixture/receiving".to_owned(),
            vmt: ManifestResource {
                lookup_path: "materials/fixture/receiving.vmt".to_owned(),
                provenance: ResourceProvenance::BuiltIn,
            },
            dependencies: Vec::new(),
            metadata: Some(VmtMaterial {
                shader: VmtShaderMetadata {
                    name: "LightmappedGeneric".to_owned(),
                    family: "lightmappedGeneric".to_owned(),
                    inputs: if nodecal {
                        [("$nodecal".to_owned(), "1".to_owned())].into()
                    } else {
                        BTreeMap::new()
                    },
                },
                textures: VmtTextureInputs::default(),
                features: VmtFeatures {
                    alpha_test,
                    ..VmtFeatures::default()
                },
                surface_prop: None,
                proxy_definitions: Vec::new(),
                unsupported: UnsupportedMaterialFeatures::default(),
            }),
            textures: Vec::new(),
        });
        manifest
    }

    fn triangle_faces_compiled_normal(fragment: &DecalOverlayFragment) -> bool {
        fragment.indices.chunks_exact(3).all(|triangle| {
            let a = fragment.positions[triangle[0] as usize];
            let b = fragment.positions[triangle[1] as usize];
            let c = fragment.positions[triangle[2] as usize];
            dot(
                cross(sub(b, a), sub(c, a)),
                fragment.normals[triangle[0] as usize],
            ) > 0.0
        })
    }

    #[test]
    fn infodecal_dimensions_use_source_integer_world_units() {
        assert_eq!(decal_world_dimension(127, 0.3), Some(38.0));
        assert_eq!(decal_world_dimension(255, 0.3), Some(76.0));
        assert_eq!(decal_world_dimension(1, 0.5), None);
    }

    #[test]
    fn infodecal_uses_the_first_short_diagonal_trace_surface_to_choose_its_model() {
        let mut fixture = GeometryFixture::default();
        fixture.add_face(
            0,
            [0.0, 0.0, 1.0],
            -2.0,
            [
                [-16.0, -16.0, -2.0],
                [16.0, -16.0, -2.0],
                [16.0, 16.0, -2.0],
                [-16.0, 16.0, -2.0],
            ],
            0,
            false,
        );
        fixture.add_face(
            1,
            [-1.0, 0.0, 0.0],
            0.0,
            [
                [0.0, -16.0, -16.0],
                [0.0, -16.0, 16.0],
                [0.0, 16.0, 16.0],
                [0.0, 16.0, -16.0],
            ],
            0,
            false,
        );

        let manifest = receiving_material_manifest(false, false);
        let InfodecalProjection {
            basis,
            target,
            fragments,
            unknown_receiver,
        } = infodecal_fragments(&fixture.input(&manifest), [0.0; 3], 16.0, 16.0);

        assert_eq!(basis.unwrap().normal, [0.0, 0.0, 1.0]);
        assert_eq!(target, Some((0, 0)));
        assert!(!unknown_receiver);
        assert_eq!(
            fragments
                .iter()
                .map(|fragment| (fragment.bsp_model_index, fragment.bsp_face_index))
                .collect::<Vec<_>>(),
            [(0, 0)]
        );
        assert!(fragments.iter().all(triangle_faces_compiled_normal));
    }

    #[test]
    fn infodecal_applies_only_within_source_plane_distance_and_face_eligibility() {
        let mut fixture = GeometryFixture::default();
        let wall = [
            [0.0, -16.0, -16.0],
            [0.0, -16.0, 16.0],
            [0.0, 16.0, 16.0],
            [0.0, 16.0, -16.0],
        ];
        fixture.add_face(0, [-1.0, 0.0, 0.0], 0.0, wall, 0, false);
        fixture.add_face(
            0,
            [-1.0, 0.0, 0.0],
            -8.0,
            wall.map(|point| [8.0, point[1], point[2]]),
            0,
            false,
        );
        fixture.add_face(0, [-1.0, 0.0, 0.0], 0.0, wall, 0, true);
        fixture.add_face(0, [-1.0, 0.0, 0.0], 0.0, wall, SURF_NODECALS, false);

        let manifest = receiving_material_manifest(false, false);
        let InfodecalProjection {
            basis,
            target,
            fragments,
            unknown_receiver,
        } = infodecal_fragments(&fixture.input(&manifest), [0.0; 3], 16.0, 16.0);

        assert_eq!(basis.unwrap().normal, [-1.0, 0.0, 0.0]);
        assert_eq!(target, Some((0, 0)));
        assert!(!unknown_receiver);
        assert_eq!(
            fragments
                .iter()
                .map(|fragment| fragment.bsp_face_index)
                .collect::<Vec<_>>(),
            [0]
        );
        assert!(fragments.iter().all(triangle_faces_compiled_normal));
    }

    #[test]
    fn infodecal_brush_target_is_traced_in_world_and_emitted_in_model_space() {
        let mut fixture = GeometryFixture::default();
        fixture.add_model_entity(1, "100 20 30", "0 90 0");
        fixture.add_face(
            1,
            [-1.0, 0.0, 0.0],
            0.0,
            [
                [0.0, -16.0, -16.0],
                [0.0, -16.0, 16.0],
                [0.0, 16.0, 16.0],
                [0.0, 16.0, -16.0],
            ],
            0,
            false,
        );
        let manifest = receiving_material_manifest(false, false);

        let InfodecalProjection {
            basis,
            target,
            fragments,
            unknown_receiver,
        } = infodecal_fragments(&fixture.input(&manifest), [100.0, 20.0, 30.0], 16.0, 16.0);

        assert_eq!(basis.unwrap().normal, [-1.0, 0.0, 0.0]);
        assert_eq!(target, Some((1, 0)));
        assert!(!unknown_receiver);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].bsp_model_index, 1);
        assert!(
            fragments[0]
                .positions
                .iter()
                .all(|position| position[0].abs() <= f32::EPSILON)
        );
        assert!(fragments.iter().all(triangle_faces_compiled_normal));
    }

    #[test]
    fn infodecal_retains_a_traced_target_when_the_surface_rejects_decals() {
        let mut fixture = GeometryFixture::default();
        fixture.add_face(
            0,
            [-1.0, 0.0, 0.0],
            0.0,
            [
                [0.0, -16.0, -16.0],
                [0.0, -16.0, 16.0],
                [0.0, 16.0, 16.0],
                [0.0, 16.0, -16.0],
            ],
            SURF_NODECALS,
            false,
        );
        let manifest = receiving_material_manifest(false, false);

        let InfodecalProjection {
            basis,
            target,
            fragments,
            unknown_receiver,
        } = infodecal_fragments(&fixture.input(&manifest), [0.0; 3], 16.0, 16.0);

        assert_eq!(basis.unwrap().normal, [-1.0, 0.0, 0.0]);
        assert_eq!(target, Some((0, 0)));
        assert!(!unknown_receiver);
        assert!(fragments.is_empty());
    }

    #[test]
    fn infodecal_rejects_alpha_tested_and_material_suppressed_receivers() {
        for manifest in [
            receiving_material_manifest(true, false),
            receiving_material_manifest(false, true),
        ] {
            let mut fixture = GeometryFixture::default();
            fixture.add_face(
                0,
                [-1.0, 0.0, 0.0],
                0.0,
                [
                    [0.0, -16.0, -16.0],
                    [0.0, -16.0, 16.0],
                    [0.0, 16.0, 16.0],
                    [0.0, 16.0, -16.0],
                ],
                0,
                false,
            );

            let InfodecalProjection {
                target,
                fragments,
                unknown_receiver,
                ..
            } = infodecal_fragments(&fixture.input(&manifest), [0.0; 3], 16.0, 16.0);

            assert_eq!(target, Some((0, 0)));
            assert!(!unknown_receiver);
            assert!(fragments.is_empty());
        }
    }

    #[test]
    fn infodecal_fails_closed_when_receiver_material_eligibility_is_unknown() {
        let mut fixture = GeometryFixture::default();
        fixture.add_face(
            0,
            [-1.0, 0.0, 0.0],
            0.0,
            [
                [0.0, -16.0, -16.0],
                [0.0, -16.0, 16.0],
                [0.0, 16.0, 16.0],
                [0.0, 16.0, -16.0],
            ],
            0,
            false,
        );
        let manifest = empty_manifest();

        let InfodecalProjection {
            target,
            fragments,
            unknown_receiver,
            ..
        } = infodecal_fragments(&fixture.input(&manifest), [0.0; 3], 16.0, 16.0);

        assert_eq!(target, Some((0, 0)));
        assert!(unknown_receiver);
        assert!(fragments.is_empty());
    }

    #[test]
    fn infodecal_displacement_target_fails_closed_without_compiled_triangles() {
        let mut fixture = GeometryFixture::default();
        fixture.add_face(
            0,
            [0.0, 0.0, 1.0],
            0.0,
            [
                [-16.0, -16.0, 0.0],
                [16.0, -16.0, 0.0],
                [16.0, 16.0, 0.0],
                [-16.0, 16.0, 0.0],
            ],
            0,
            true,
        );
        let manifest = receiving_material_manifest(false, false);

        let InfodecalProjection {
            basis,
            target,
            fragments,
            unknown_receiver,
        } = infodecal_fragments(&fixture.input(&manifest), [0.0; 3], 16.0, 16.0);

        assert!(basis.is_none());
        assert!(target.is_none());
        assert!(!unknown_receiver);
        assert!(fragments.is_empty());
    }

    #[test]
    fn infodecal_clips_across_coplanar_adjacent_faces_at_exact_world_dimensions() {
        let mut fixture = GeometryFixture::default();
        fixture.add_face(
            0,
            [-1.0, 0.0, 0.0],
            0.0,
            [
                [0.0, -96.0, -160.0],
                [0.0, -96.0, 160.0],
                [0.0, 0.0, 160.0],
                [0.0, 0.0, -160.0],
            ],
            0,
            false,
        );
        fixture.add_face(
            0,
            [-1.0, 0.0, 0.0],
            0.0,
            [
                [0.0, 0.0, -160.0],
                [0.0, 0.0, 160.0],
                [0.0, 96.0, 160.0],
                [0.0, 96.0, -160.0],
            ],
            0,
            false,
        );

        let manifest = receiving_material_manifest(false, false);
        let InfodecalProjection {
            basis,
            target,
            fragments,
            unknown_receiver,
        } = infodecal_fragments(&fixture.input(&manifest), [0.0; 3], 128.0, 256.0);

        assert_eq!(basis.as_ref().unwrap().u, [0.0, -1.0, 0.0]);
        assert_eq!(basis.as_ref().unwrap().v, [0.0, 0.0, -1.0]);
        assert_eq!(target, Some((0, 0)));
        assert!(!unknown_receiver);
        assert_eq!(
            fragments
                .iter()
                .map(|fragment| fragment.bsp_face_index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        let positions: Vec<_> = fragments
            .iter()
            .flat_map(|fragment| fragment.positions.iter())
            .collect();
        assert_eq!(
            positions
                .iter()
                .map(|position| position[1])
                .fold(f32::INFINITY, f32::min),
            -64.0
        );
        assert_eq!(
            positions
                .iter()
                .map(|position| position[1])
                .fold(f32::NEG_INFINITY, f32::max),
            64.0
        );
        assert_eq!(
            positions
                .iter()
                .map(|position| position[2])
                .fold(f32::INFINITY, f32::min),
            -128.0
        );
        assert_eq!(
            positions
                .iter()
                .map(|position| position[2])
                .fold(f32::NEG_INFINITY, f32::max),
            128.0
        );
        assert!(fragments.iter().all(triangle_faces_compiled_normal));
    }
}
