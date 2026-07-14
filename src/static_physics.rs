use crate::parse_bsp;
use crate::phy::{
    DecodeLimits, PhyCollide, PhyConvex, PhyFace, PhyShapeStatus, PhySolid, PhySolidEncoding,
    PhyTypedKeyData, decode_physcollide,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt::{Display, Formatter};

const LUMP_PHYSCOLLIDE: usize = 29;
const PHYSICS_MODEL_HEADER_BYTES: usize = 16;
const BINARY_MAGIC: &[u8; 8] = b"BSPPHYS\0";
const BINARY_HEADER_BYTES: usize = 64;
const SHAPE_RECORD_BYTES: usize = 72;
const CONVEX_RECORD_BYTES: usize = 20;
const VERTEX_RECORD_BYTES: usize = 12;
const FACE_RECORD_BYTES: usize = 16;
const FLAG_DRAG_AXIS_AREAS: u32 = 1;
const NONE_U32: u32 = u32::MAX;

pub const STATIC_PHYSICS_SCHEMA_VERSION: u32 = 1;
pub const STATIC_PHYSICS_BINARY_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug)]
pub struct StaticPhysicsLimits {
    pub decode: DecodeLimits,
    pub max_models: usize,
    pub max_total_solids: usize,
    pub max_total_shapes: usize,
    pub max_total_convexes: usize,
    pub max_total_vertices: usize,
    pub max_total_faces: usize,
    pub max_binary_bytes: usize,
}

impl Default for StaticPhysicsLimits {
    fn default() -> Self {
        Self {
            decode: DecodeLimits::default(),
            max_models: 4096,
            max_total_solids: 65_536,
            max_total_shapes: 65_536,
            max_total_convexes: 65_536,
            max_total_vertices: 3_000_000,
            max_total_faces: 1_000_000,
            max_binary_bytes: 128 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticPhysicsError(String);

impl Display for StaticPhysicsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for StaticPhysicsError {}

fn error(message: impl Into<String>) -> StaticPhysicsError {
    StaticPhysicsError(message.into())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPhysicsCounts {
    pub shapes: usize,
    pub convexes: usize,
    pub vertices: usize,
    pub faces: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedShapeBundle {
    pub binary: Vec<u8>,
    pub counts: StaticPhysicsCounts,
    pub solid_shape_indices: Vec<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedStaticShape {
    pub source_id: i32,
    pub solid: PhySolid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedShapeBundle {
    pub version: u32,
    pub shapes: Vec<DecodedStaticShape>,
    pub counts: StaticPhysicsCounts,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPhysicsBinaryArtifact {
    pub format: String,
    pub version: u32,
    pub byte_length: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPhysicsSolidManifest {
    pub solid_index: usize,
    pub status: PhyShapeStatus,
    pub shape_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPhysicsModelManifest {
    pub model_index: i32,
    pub key_data: PhyTypedKeyData,
    pub solids: Vec<StaticPhysicsSolidManifest>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPhysicsExportStats {
    pub models: usize,
    pub solids: usize,
    pub decoded_solids: usize,
    pub unsupported_solids: usize,
    pub shapes: usize,
    pub convexes: usize,
    pub vertices: usize,
    pub faces: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPhysicsManifest {
    pub schema: String,
    pub schema_version: u32,
    pub source_bsp_version: i32,
    pub coordinate_system: String,
    pub binary: StaticPhysicsBinaryArtifact,
    pub stats: StaticPhysicsExportStats,
    pub models: Vec<StaticPhysicsModelManifest>,
}

impl StaticPhysicsManifest {
    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self)
            .map_err(|error| format!("failed to serialize static physics manifest: {error}"))
    }
}

#[derive(Debug)]
pub struct StaticPhysicsExportResult {
    pub manifest: StaticPhysicsManifest,
    pub binary: Vec<u8>,
}

#[derive(Clone, Copy)]
struct ShapeInput<'a> {
    source_id: i32,
    solid: &'a PhySolid,
}

fn checked_u32(value: usize, label: &str) -> Result<u32, StaticPhysicsError> {
    u32::try_from(value).map_err(|_| error(format!("{label} exceeds the binary format")))
}

fn add_count(
    value: &mut usize,
    amount: usize,
    maximum: usize,
    label: &str,
) -> Result<(), StaticPhysicsError> {
    *value = value
        .checked_add(amount)
        .ok_or_else(|| error(format!("{label} count overflows")))?;
    if *value > maximum {
        return Err(error(format!("{label} limit exceeded")));
    }
    Ok(())
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_f32(output: &mut Vec<u8>, value: f32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn encoding_value(encoding: PhySolidEncoding) -> Result<u32, StaticPhysicsError> {
    match encoding {
        PhySolidEncoding::ModernPolygon => Ok(0),
        PhySolidEncoding::LegacyCompact => Ok(1),
        _ => Err(error(
            "unsupported solid cannot be encoded as a static shape",
        )),
    }
}

fn encode_shapes(
    inputs: &[ShapeInput<'_>],
    limits: StaticPhysicsLimits,
) -> Result<(Vec<u8>, StaticPhysicsCounts), StaticPhysicsError> {
    let mut counts = StaticPhysicsCounts {
        shapes: inputs.len(),
        ..StaticPhysicsCounts::default()
    };
    if counts.shapes > limits.max_total_shapes {
        return Err(error("shape limit exceeded"));
    }
    for input in inputs {
        add_count(
            &mut counts.convexes,
            input.solid.convexes.len(),
            limits.max_total_convexes,
            "convex",
        )?;
        for convex in &input.solid.convexes {
            add_count(
                &mut counts.vertices,
                convex.vertices.len(),
                limits.max_total_vertices,
                "vertex",
            )?;
            add_count(
                &mut counts.faces,
                convex.faces.len(),
                limits.max_total_faces,
                "face",
            )?;
            for face in &convex.faces {
                if face
                    .indices
                    .iter()
                    .any(|index| *index as usize >= convex.vertices.len())
                {
                    return Err(error("face references a vertex outside its convex"));
                }
            }
        }
    }

    let shape_offset = BINARY_HEADER_BYTES;
    let convex_offset = shape_offset
        .checked_add(counts.shapes * SHAPE_RECORD_BYTES)
        .ok_or_else(|| error("shape table range overflows"))?;
    let vertex_offset = convex_offset
        .checked_add(counts.convexes * CONVEX_RECORD_BYTES)
        .ok_or_else(|| error("convex table range overflows"))?;
    let face_offset = vertex_offset
        .checked_add(counts.vertices * VERTEX_RECORD_BYTES)
        .ok_or_else(|| error("vertex table range overflows"))?;
    let total_bytes = face_offset
        .checked_add(counts.faces * FACE_RECORD_BYTES)
        .ok_or_else(|| error("face table range overflows"))?;
    if total_bytes > limits.max_binary_bytes {
        return Err(error("static physics binary byte limit exceeded"));
    }

    let mut shapes = Vec::with_capacity(counts.shapes * SHAPE_RECORD_BYTES);
    let mut convexes = Vec::with_capacity(counts.convexes * CONVEX_RECORD_BYTES);
    let mut vertices = Vec::with_capacity(counts.vertices * VERTEX_RECORD_BYTES);
    let mut faces = Vec::with_capacity(counts.faces * FACE_RECORD_BYTES);
    let mut first_convex = 0_usize;
    let mut first_vertex = 0_usize;
    let mut first_face = 0_usize;

    for input in inputs {
        push_i32(&mut shapes, input.source_id);
        push_u32(
            &mut shapes,
            checked_u32(input.solid.solid_index, "solid index")?,
        );
        push_u32(&mut shapes, encoding_value(input.solid.encoding)?);
        push_u32(&mut shapes, checked_u32(first_convex, "first convex")?);
        push_u32(
            &mut shapes,
            checked_u32(input.solid.convexes.len(), "shape convex count")?,
        );
        push_u32(
            &mut shapes,
            u32::from(input.solid.drag_axis_areas.is_some()) * FLAG_DRAG_AXIS_AREAS,
        );
        for value in input.solid.center_of_mass {
            push_f32(&mut shapes, value);
        }
        for value in input.solid.rotation_inertia {
            push_f32(&mut shapes, value);
        }
        push_f32(&mut shapes, input.solid.upper_limit_radius);
        for value in input.solid.drag_axis_areas.unwrap_or([0.0; 3]) {
            push_f32(&mut shapes, value);
        }
        push_u32(
            &mut shapes,
            input
                .solid
                .axis_map_size
                .map(|value| checked_u32(value, "axis map size"))
                .transpose()?
                .unwrap_or(NONE_U32),
        );
        push_u32(&mut shapes, u32::from(input.solid.max_surface_deviation));

        for convex in &input.solid.convexes {
            push_u32(&mut convexes, checked_u32(first_vertex, "first vertex")?);
            push_u32(
                &mut convexes,
                checked_u32(convex.vertices.len(), "convex vertex count")?,
            );
            push_u32(&mut convexes, checked_u32(first_face, "first face")?);
            push_u32(
                &mut convexes,
                checked_u32(convex.faces.len(), "convex face count")?,
            );
            push_i32(&mut convexes, convex.client_data);
            for vertex in &convex.vertices {
                for value in vertex {
                    if !value.is_finite() {
                        return Err(error("vertex contains a non-finite value"));
                    }
                    push_f32(&mut vertices, *value);
                }
            }
            for face in &convex.faces {
                for index in face.indices {
                    push_u32(&mut faces, index);
                }
                push_u32(
                    &mut faces,
                    u32::from(face.material_index) | (u32::from(face.is_virtual) << 8),
                );
            }
            first_vertex += convex.vertices.len();
            first_face += convex.faces.len();
        }
        first_convex += input.solid.convexes.len();
    }

    let mut output = Vec::with_capacity(total_bytes);
    output.extend_from_slice(BINARY_MAGIC);
    push_u32(&mut output, STATIC_PHYSICS_BINARY_VERSION);
    push_u32(&mut output, BINARY_HEADER_BYTES as u32);
    push_u32(&mut output, checked_u32(total_bytes, "binary byte length")?);
    push_u32(&mut output, checked_u32(counts.shapes, "shape count")?);
    push_u32(&mut output, checked_u32(counts.convexes, "convex count")?);
    push_u32(&mut output, checked_u32(counts.vertices, "vertex count")?);
    push_u32(&mut output, checked_u32(counts.faces, "face count")?);
    push_u32(&mut output, checked_u32(shape_offset, "shape offset")?);
    push_u32(&mut output, checked_u32(convex_offset, "convex offset")?);
    push_u32(&mut output, checked_u32(vertex_offset, "vertex offset")?);
    push_u32(&mut output, checked_u32(face_offset, "face offset")?);
    output.resize(BINARY_HEADER_BYTES, 0);
    output.extend_from_slice(&shapes);
    output.extend_from_slice(&convexes);
    output.extend_from_slice(&vertices);
    output.extend_from_slice(&faces);
    debug_assert_eq!(output.len(), total_bytes);
    Ok((output, counts))
}

pub fn encode_phy_shape_bundle(
    source_id: i32,
    solids: &[PhySolid],
    limits: StaticPhysicsLimits,
) -> Result<EncodedShapeBundle, StaticPhysicsError> {
    if solids.len() > limits.max_total_solids {
        return Err(error("solid limit exceeded"));
    }
    let mut inputs = Vec::new();
    let mut solid_shape_indices = Vec::with_capacity(solids.len());
    for solid in solids {
        if solid.status == PhyShapeStatus::Decoded {
            solid_shape_indices.push(Some(checked_u32(inputs.len(), "shape index")?));
            inputs.push(ShapeInput { source_id, solid });
        } else {
            solid_shape_indices.push(None);
        }
    }
    let (binary, counts) = encode_shapes(&inputs, limits)?;
    Ok(EncodedShapeBundle {
        binary,
        counts,
        solid_shape_indices,
    })
}

fn read_range<'a>(
    data: &'a [u8],
    offset: usize,
    length: usize,
    label: &str,
) -> Result<&'a [u8], StaticPhysicsError> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| error(format!("{label} range overflows")))?;
    data.get(offset..end)
        .ok_or_else(|| error(format!("{label} is truncated")))
}

fn read_u32(data: &[u8], offset: usize, label: &str) -> Result<u32, StaticPhysicsError> {
    Ok(u32::from_le_bytes(
        read_range(data, offset, 4, label)?.try_into().unwrap(),
    ))
}

fn read_i32(data: &[u8], offset: usize, label: &str) -> Result<i32, StaticPhysicsError> {
    Ok(i32::from_le_bytes(
        read_range(data, offset, 4, label)?.try_into().unwrap(),
    ))
}

fn read_f32(data: &[u8], offset: usize, label: &str) -> Result<f32, StaticPhysicsError> {
    let value = f32::from_le_bytes(read_range(data, offset, 4, label)?.try_into().unwrap());
    if !value.is_finite() {
        return Err(error(format!("{label} contains a non-finite value")));
    }
    Ok(value)
}

fn read_vector(data: &[u8], offset: usize, label: &str) -> Result<[f32; 3], StaticPhysicsError> {
    Ok([
        read_f32(data, offset, label)?,
        read_f32(data, offset + 4, label)?,
        read_f32(data, offset + 8, label)?,
    ])
}

fn checked_table(
    data: &[u8],
    offset: usize,
    count: usize,
    stride: usize,
    expected_end: usize,
    label: &str,
) -> Result<(), StaticPhysicsError> {
    let length = count
        .checked_mul(stride)
        .ok_or_else(|| error(format!("{label} range overflows")))?;
    let end = offset
        .checked_add(length)
        .ok_or_else(|| error(format!("{label} range overflows")))?;
    if end != expected_end {
        return Err(error(format!("{label} range is not canonical")));
    }
    read_range(data, offset, length, label)?;
    Ok(())
}

pub fn decode_shape_bundle(
    data: &[u8],
    limits: StaticPhysicsLimits,
) -> Result<DecodedShapeBundle, StaticPhysicsError> {
    if data.len() > limits.max_binary_bytes {
        return Err(error("static physics binary byte limit exceeded"));
    }
    read_range(data, 0, BINARY_HEADER_BYTES, "static physics header")?;
    if read_range(data, 0, 8, "static physics magic")? != BINARY_MAGIC {
        return Err(error("static physics binary magic is invalid"));
    }
    let version = read_u32(data, 8, "static physics version")?;
    if version != STATIC_PHYSICS_BINARY_VERSION {
        return Err(error("static physics binary version is unsupported"));
    }
    if read_u32(data, 12, "static physics header size")? as usize != BINARY_HEADER_BYTES {
        return Err(error("static physics header size is invalid"));
    }
    if read_u32(data, 16, "static physics byte length")? as usize != data.len() {
        return Err(error("static physics byte length does not match"));
    }
    let counts = StaticPhysicsCounts {
        shapes: read_u32(data, 20, "shape count")? as usize,
        convexes: read_u32(data, 24, "convex count")? as usize,
        vertices: read_u32(data, 28, "vertex count")? as usize,
        faces: read_u32(data, 32, "face count")? as usize,
    };
    if counts.shapes > limits.max_total_shapes
        || counts.convexes > limits.max_total_convexes
        || counts.vertices > limits.max_total_vertices
        || counts.faces > limits.max_total_faces
    {
        return Err(error("static physics binary count limit exceeded"));
    }
    let shape_offset = read_u32(data, 36, "shape table offset")? as usize;
    let convex_offset = read_u32(data, 40, "convex table offset")? as usize;
    let vertex_offset = read_u32(data, 44, "vertex table offset")? as usize;
    let face_offset = read_u32(data, 48, "face table offset")? as usize;
    if shape_offset != BINARY_HEADER_BYTES {
        return Err(error("shape table offset is not canonical"));
    }
    checked_table(
        data,
        shape_offset,
        counts.shapes,
        SHAPE_RECORD_BYTES,
        convex_offset,
        "shape table",
    )?;
    checked_table(
        data,
        convex_offset,
        counts.convexes,
        CONVEX_RECORD_BYTES,
        vertex_offset,
        "convex table",
    )?;
    checked_table(
        data,
        vertex_offset,
        counts.vertices,
        VERTEX_RECORD_BYTES,
        face_offset,
        "vertex table",
    )?;
    checked_table(
        data,
        face_offset,
        counts.faces,
        FACE_RECORD_BYTES,
        data.len(),
        "face table",
    )?;

    let mut shapes = Vec::with_capacity(counts.shapes);
    for shape_index in 0..counts.shapes {
        let offset = shape_offset + shape_index * SHAPE_RECORD_BYTES;
        let source_id = read_i32(data, offset, "shape source id")?;
        let solid_index = read_u32(data, offset + 4, "shape solid index")? as usize;
        let encoding = match read_u32(data, offset + 8, "shape encoding")? {
            0 => PhySolidEncoding::ModernPolygon,
            1 => PhySolidEncoding::LegacyCompact,
            _ => return Err(error("shape encoding is unsupported")),
        };
        let first_convex = read_u32(data, offset + 12, "shape first convex")? as usize;
        let convex_count = read_u32(data, offset + 16, "shape convex count")? as usize;
        let convex_end = first_convex
            .checked_add(convex_count)
            .ok_or_else(|| error("shape convex range overflows"))?;
        if convex_end > counts.convexes {
            return Err(error("shape convex range is out of bounds"));
        }
        let flags = read_u32(data, offset + 20, "shape flags")?;
        if flags & !FLAG_DRAG_AXIS_AREAS != 0 {
            return Err(error("shape flags are unsupported"));
        }
        let center_of_mass = read_vector(data, offset + 24, "shape center of mass")?;
        let rotation_inertia = read_vector(data, offset + 36, "shape rotation inertia")?;
        let upper_limit_radius = read_f32(data, offset + 48, "shape upper radius")?;
        let drag = read_vector(data, offset + 52, "shape drag axis areas")?;
        let axis_map = read_u32(data, offset + 64, "shape axis map size")?;
        let deviation = read_u32(data, offset + 68, "shape surface deviation")?;
        if deviation > u8::MAX as u32 {
            return Err(error("shape surface deviation is invalid"));
        }

        let mut shape_convexes = Vec::with_capacity(convex_count);
        for convex_index in first_convex..convex_end {
            let convex_record = convex_offset + convex_index * CONVEX_RECORD_BYTES;
            let first_vertex = read_u32(data, convex_record, "convex first vertex")? as usize;
            let vertex_count = read_u32(data, convex_record + 4, "convex vertex count")? as usize;
            let first_face = read_u32(data, convex_record + 8, "convex first face")? as usize;
            let face_count = read_u32(data, convex_record + 12, "convex face count")? as usize;
            let vertex_end = first_vertex
                .checked_add(vertex_count)
                .ok_or_else(|| error("convex vertex range overflows"))?;
            let face_end = first_face
                .checked_add(face_count)
                .ok_or_else(|| error("convex face range overflows"))?;
            if vertex_end > counts.vertices || face_end > counts.faces {
                return Err(error("convex geometry range is out of bounds"));
            }
            let mut shape_vertices = Vec::with_capacity(vertex_count);
            for vertex_index in first_vertex..vertex_end {
                shape_vertices.push(read_vector(
                    data,
                    vertex_offset + vertex_index * VERTEX_RECORD_BYTES,
                    "vertex",
                )?);
            }
            let mut shape_faces = Vec::with_capacity(face_count);
            for face_index in first_face..face_end {
                let face_record = face_offset + face_index * FACE_RECORD_BYTES;
                let indices = [
                    read_u32(data, face_record, "face index")?,
                    read_u32(data, face_record + 4, "face index")?,
                    read_u32(data, face_record + 8, "face index")?,
                ];
                if indices.iter().any(|index| *index as usize >= vertex_count) {
                    return Err(error("face references a vertex outside its convex"));
                }
                let metadata = read_u32(data, face_record + 12, "face metadata")?;
                if metadata & !0x17f != 0 {
                    return Err(error("face metadata is invalid"));
                }
                shape_faces.push(PhyFace {
                    indices,
                    material_index: (metadata & 0x7f) as u8,
                    is_virtual: metadata & 0x100 != 0,
                });
            }
            shape_convexes.push(PhyConvex {
                client_data: read_i32(data, convex_record + 16, "convex client data")?,
                vertices: shape_vertices,
                faces: shape_faces,
            });
        }
        shapes.push(DecodedStaticShape {
            source_id,
            solid: PhySolid {
                solid_index,
                byte_length: 0,
                encoding,
                status: PhyShapeStatus::Decoded,
                center_of_mass,
                rotation_inertia,
                upper_limit_radius,
                max_surface_deviation: deviation as u8,
                drag_axis_areas: (flags & FLAG_DRAG_AXIS_AREAS != 0).then_some(drag),
                axis_map_size: (axis_map != NONE_U32).then_some(axis_map as usize),
                convexes: shape_convexes,
            },
        });
    }
    Ok(DecodedShapeBundle {
        version,
        shapes,
        counts,
    })
}

fn parse_model_blocks(
    data: &[u8],
    limits: StaticPhysicsLimits,
) -> Result<Vec<(i32, PhyCollide)>, StaticPhysicsError> {
    let mut cursor = 0_usize;
    let mut output = Vec::new();
    let mut total_solids = 0_usize;
    let mut terminator = false;
    while cursor < data.len() {
        let header = read_range(
            data,
            cursor,
            PHYSICS_MODEL_HEADER_BYTES,
            "PHYSCOLLIDE model header",
        )?;
        let model_index = i32::from_le_bytes(header[0..4].try_into().unwrap());
        let collision_size = i32::from_le_bytes(header[4..8].try_into().unwrap());
        let key_size = i32::from_le_bytes(header[8..12].try_into().unwrap());
        let solid_count = i32::from_le_bytes(header[12..16].try_into().unwrap());
        cursor += PHYSICS_MODEL_HEADER_BYTES;
        if model_index == -1 {
            if collision_size != -1 || key_size != 0 || solid_count != 0 {
                return Err(error("PHYSCOLLIDE terminator metadata is invalid"));
            }
            terminator = true;
            break;
        }
        if model_index < 0 || collision_size < 0 || key_size < 0 || solid_count < 0 {
            return Err(error("PHYSCOLLIDE model metadata is negative"));
        }
        if output.len() >= limits.max_models {
            return Err(error("PHYSCOLLIDE model limit exceeded"));
        }
        total_solids = total_solids
            .checked_add(solid_count as usize)
            .ok_or_else(|| error("PHYSCOLLIDE solid count overflows"))?;
        if total_solids > limits.max_total_solids {
            return Err(error("PHYSCOLLIDE solid limit exceeded"));
        }
        let collision = read_range(
            data,
            cursor,
            collision_size as usize,
            "PHYSCOLLIDE collision data",
        )?;
        cursor = cursor
            .checked_add(collision_size as usize)
            .ok_or_else(|| error("PHYSCOLLIDE collision range overflows"))?;
        let keys = read_range(data, cursor, key_size as usize, "PHYSCOLLIDE keydata")?;
        cursor = cursor
            .checked_add(key_size as usize)
            .ok_or_else(|| error("PHYSCOLLIDE keydata range overflows"))?;
        let decoded = decode_physcollide(collision, keys, solid_count as usize, limits.decode)
            .map_err(|decode| error(format!("PHYSCOLLIDE model {model_index}: {decode}")))?;
        output.push((model_index, decoded));
    }
    if !terminator {
        return Err(error("PHYSCOLLIDE terminator is missing"));
    }
    if cursor != data.len() {
        return Err(error("PHYSCOLLIDE has trailing bytes after its terminator"));
    }
    Ok(output)
}

pub fn export_bsp_static_physics(
    data: &[u8],
    limits: StaticPhysicsLimits,
) -> Result<StaticPhysicsExportResult, String> {
    let bsp = parse_bsp(data)?;
    let decoded = parse_model_blocks(&bsp.lumps[LUMP_PHYSCOLLIDE], limits)
        .map_err(|decode| decode.to_string())?;
    let mut shape_inputs = Vec::new();
    let mut models = Vec::with_capacity(decoded.len());
    let mut solids = 0_usize;
    let mut unsupported_solids = 0_usize;
    for (model_index, collide) in &decoded {
        let mut solid_manifests = Vec::with_capacity(collide.solids.len());
        for solid in &collide.solids {
            solids += 1;
            let shape_index = if solid.status == PhyShapeStatus::Decoded {
                let index = u32::try_from(shape_inputs.len())
                    .map_err(|_| "static physics shape index exceeds u32".to_owned())?;
                shape_inputs.push(ShapeInput {
                    source_id: *model_index,
                    solid,
                });
                Some(index)
            } else {
                unsupported_solids += 1;
                None
            };
            solid_manifests.push(StaticPhysicsSolidManifest {
                solid_index: solid.solid_index,
                status: solid.status,
                shape_index,
            });
        }
        models.push(StaticPhysicsModelManifest {
            model_index: *model_index,
            key_data: collide.key_data.clone(),
            solids: solid_manifests,
        });
    }
    let (binary, counts) =
        encode_shapes(&shape_inputs, limits).map_err(|encode| encode.to_string())?;
    let sha256 = format!("{:x}", Sha256::digest(&binary));
    let stats = StaticPhysicsExportStats {
        models: models.len(),
        solids,
        decoded_solids: counts.shapes,
        unsupported_solids,
        shapes: counts.shapes,
        convexes: counts.convexes,
        vertices: counts.vertices,
        faces: counts.faces,
    };
    Ok(StaticPhysicsExportResult {
        manifest: StaticPhysicsManifest {
            schema: "bsp-to-glb/static-physics".to_owned(),
            schema_version: STATIC_PHYSICS_SCHEMA_VERSION,
            source_bsp_version: bsp.version,
            coordinate_system: "Source XYZ".to_owned(),
            binary: StaticPhysicsBinaryArtifact {
                format: "bsp-to-glb/static-physics-binary".to_owned(),
                version: STATIC_PHYSICS_BINARY_VERSION,
                byte_length: binary.len(),
                sha256,
            },
            stats,
            models,
        },
        binary,
    })
}
