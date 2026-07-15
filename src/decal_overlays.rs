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
    Some((texture.width as f32 * scale, texture.height as f32 * scale))
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
) -> (Option<DecalOverlayBasis>, Vec<DecalOverlayFragment>) {
    let query_radius = width.max(height) * 0.5;
    let mut candidates = Vec::new();
    for (face_index, face) in input.faces.iter().copied().enumerate() {
        let Some(model_index) = input.face_owner.get(face_index).copied().flatten() else {
            continue;
        };
        let Some(texinfo) = usize::try_from(face.texinfo)
            .ok()
            .and_then(|index| input.texinfos.get(index))
        else {
            continue;
        };
        if texinfo.flags & NON_DECAL_SURFACES != 0 || face.dispinfo >= 0 {
            continue;
        }
        let Some(plane) = plane_for_face(input.planes, face) else {
            continue;
        };
        let distance = dot(origin, plane.normal) - plane.distance;
        if distance.abs() >= query_radius {
            continue;
        }
        let Some(basis) = decal_basis(plane.normal) else {
            continue;
        };
        let center = projected_point(origin, plane);
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
        let intersects_query = (0..3).all(|axis| {
            let minimum = face_polygon
                .iter()
                .map(|point| point[axis])
                .fold(f32::INFINITY, f32::min);
            let maximum = face_polygon
                .iter()
                .map(|point| point[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            origin[axis] + query_radius >= minimum && origin[axis] - query_radius <= maximum
        });
        if !intersects_query {
            continue;
        }
        let contains_origin = point_in_polygon(center, &face_polygon, plane.normal);
        let polygon = clip_polygon(subject, &face_polygon, plane.normal);
        if let Some(item) = fragment(
            face_index,
            model_index,
            polygon,
            plane,
            face,
            texinfo,
            input.lightmaps,
        ) {
            candidates.push((model_index, contains_origin, distance.abs(), basis, item));
        }
    }
    let Some(chosen_model) = candidates
        .iter()
        .map(|(model, contains, distance, _, _)| (*model, *contains, *distance))
        .min_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then(left.2.total_cmp(&right.2))
                .then(left.0.cmp(&right.0))
        })
        .map(|candidate| candidate.0)
    else {
        return (None, Vec::new());
    };
    let basis = candidates
        .iter()
        .find(|candidate| candidate.0 == chosen_model)
        .map(|candidate| candidate.3.clone());
    let fragments = candidates
        .into_iter()
        .filter_map(|candidate| (candidate.0 == chosen_model).then_some(candidate.4))
        .collect();
    (basis, fragments)
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
        let (basis, fragments) = match (origin, dimensions) {
            (Some(origin), Some((width, height))) => {
                infodecal_fragments(&input, origin, width, height)
            }
            _ => (None, Vec::new()),
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
        } else if fragments.is_empty() {
            (DecalOverlayStatus::Unsupported, "target-face-unresolved")
        } else {
            (DecalOverlayStatus::Handled, "projected-to-compiled-faces")
        };
        let target_model = fragments.first().map(|fragment| fragment.bsp_model_index);
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
            target: target_model.map(|bsp_model_index| DecalOverlayTarget {
                bsp_model_index,
                bsp_face_indices: fragments.iter().map(|item| item.bsp_face_index).collect(),
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
