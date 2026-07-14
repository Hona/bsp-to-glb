use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

const PHY_HEADER_BYTES: usize = 16;
const MODERN_HEADER_BYTES: usize = 28;
const SURFACE_HEADER_BYTES: usize = 48;
const TREE_NODE_BYTES: usize = 28;
const LEDGE_HEADER_BYTES: usize = 16;
const TRIANGLE_BYTES: usize = 16;
const POINT_BYTES: usize = 16;
const METERS_PER_SOURCE_UNIT: f32 = 0.0254;
const VPHY: u32 = u32::from_le_bytes(*b"VPHY");
const YHPV: u32 = u32::from_le_bytes(*b"YHPV");
const IVPS: u32 = u32::from_le_bytes(*b"IVPS");
const SVPI: u32 = u32::from_le_bytes(*b"SVPI");
const MOPP: u32 = u32::from_le_bytes(*b"MOPP");

#[derive(Clone, Copy, Debug)]
pub struct DecodeLimits {
    pub max_file_bytes: usize,
    pub max_solid_bytes: usize,
    pub max_key_data_bytes: usize,
    pub max_solids: usize,
    pub max_tree_nodes: usize,
    pub max_tree_depth: usize,
    pub max_convexes: usize,
    pub max_triangles: usize,
    pub max_vertices: usize,
    pub max_key_tokens: usize,
    pub max_key_string_bytes: usize,
    pub max_key_depth: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 128 * 1024 * 1024,
            max_solid_bytes: 64 * 1024 * 1024,
            max_key_data_bytes: 8 * 1024 * 1024,
            max_solids: 4096,
            max_tree_nodes: 65_536,
            max_tree_depth: 1024,
            max_convexes: 65_536,
            max_triangles: 1_000_000,
            max_vertices: 3_000_000,
            max_key_tokens: 1_000_000,
            max_key_string_bytes: 64 * 1024,
            max_key_depth: 128,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhyDecodeError(String);

impl Display for PhyDecodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for PhyDecodeError {}

fn error(message: impl Into<String>) -> PhyDecodeError {
    PhyDecodeError(message.into())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhyFile {
    pub header: PhyHeader,
    pub solids: Vec<PhySolid>,
    pub key_data: PhyTypedKeyData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhyCollide {
    pub solids: Vec<PhySolid>,
    pub key_data: PhyTypedKeyData,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhyHeader {
    pub header_size: usize,
    pub id: i32,
    pub solid_count: usize,
    pub checksum: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhySolidEncoding {
    ModernPolygon,
    ModernUnsupported,
    LegacyCompact,
    LegacyUnsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnsupportedShapeKind {
    Mopp,
    Ball,
    Virtual,
    SwappedEndian,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "kind", rename_all = "camelCase")]
pub enum PhyShapeStatus {
    Decoded,
    Unsupported(UnsupportedShapeKind),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhySolid {
    pub solid_index: usize,
    pub byte_length: usize,
    pub encoding: PhySolidEncoding,
    pub status: PhyShapeStatus,
    pub center_of_mass: [f32; 3],
    pub rotation_inertia: [f32; 3],
    pub upper_limit_radius: f32,
    pub max_surface_deviation: u8,
    pub drag_axis_areas: Option<[f32; 3]>,
    pub axis_map_size: Option<usize>,
    pub convexes: Vec<PhyConvex>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhyConvex {
    pub client_data: i32,
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<PhyFace>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhyFace {
    pub indices: [u32; 3],
    pub material_index: u8,
    pub is_virtual: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhyTypedKeyData {
    pub solids: Vec<PhySolidKeyData>,
    pub unknown_blocks: Vec<PhyKeyBlock>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhySolidKeyData {
    pub index: Option<i32>,
    pub name: Option<String>,
    pub parent: Option<String>,
    pub surface_prop: Option<String>,
    pub mass: Option<f32>,
    pub mass_center_override: Option<[f32; 3]>,
    pub damping: Option<f32>,
    pub rotation_damping: Option<f32>,
    pub drag: Option<f32>,
    pub inertia: Option<f32>,
    pub rotation_inertia_limit: Option<f32>,
    pub volume: Option<f32>,
    pub unknown: Vec<PhyKeyValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhyKeyBlock {
    pub name: String,
    pub entries: Vec<PhyKeyValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PhyKeyValue {
    Scalar {
        key: String,
        value: String,
    },
    Block {
        key: String,
        entries: Vec<PhyKeyValue>,
    },
}

fn range<'a>(
    data: &'a [u8],
    offset: usize,
    length: usize,
    context: &str,
) -> Result<&'a [u8], PhyDecodeError> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| error(format!("{context} range overflows")))?;
    data.get(offset..end)
        .ok_or_else(|| error(format!("{context} is truncated")))
}

fn i16_at(data: &[u8], offset: usize, context: &str) -> Result<i16, PhyDecodeError> {
    Ok(i16::from_le_bytes(
        range(data, offset, 2, context)?.try_into().unwrap(),
    ))
}

fn i32_at(data: &[u8], offset: usize, context: &str) -> Result<i32, PhyDecodeError> {
    Ok(i32::from_le_bytes(
        range(data, offset, 4, context)?.try_into().unwrap(),
    ))
}

fn u32_at(data: &[u8], offset: usize, context: &str) -> Result<u32, PhyDecodeError> {
    Ok(u32::from_le_bytes(
        range(data, offset, 4, context)?.try_into().unwrap(),
    ))
}

fn f32_at(data: &[u8], offset: usize, context: &str) -> Result<f32, PhyDecodeError> {
    let value = f32::from_le_bytes(range(data, offset, 4, context)?.try_into().unwrap());
    if !value.is_finite() {
        return Err(error(format!("{context} contains a non-finite float")));
    }
    Ok(value)
}

fn vector_at(data: &[u8], offset: usize, context: &str) -> Result<[f32; 3], PhyDecodeError> {
    Ok([
        f32_at(data, offset, context)?,
        f32_at(data, offset + 4, context)?,
        f32_at(data, offset + 8, context)?,
    ])
}

fn ivp_position_to_source(value: [f32; 3]) -> [f32; 3] {
    let converted = [
        value[0] / METERS_PER_SOURCE_UNIT,
        value[2] / METERS_PER_SOURCE_UNIT,
        -value[1] / METERS_PER_SOURCE_UNIT,
    ];
    converted.map(|component| if component == -0.0 { 0.0 } else { component })
}

fn ivp_inertia_to_source(value: [f32; 3]) -> [f32; 3] {
    [value[0], value[2], value[1]]
}

#[derive(Debug)]
struct SurfaceMetadata {
    center_of_mass: [f32; 3],
    rotation_inertia: [f32; 3],
    upper_limit_radius: f32,
    max_surface_deviation: u8,
    byte_size: usize,
    tree_root: usize,
    legacy_identifier: u32,
}

fn surface_metadata(data: &[u8]) -> Result<SurfaceMetadata, PhyDecodeError> {
    range(data, 0, SURFACE_HEADER_BYTES, "compact surface header")?;
    let packed = u32_at(data, 28, "compact surface size")?;
    let byte_size = (packed >> 8) as usize;
    if byte_size < SURFACE_HEADER_BYTES || byte_size > data.len() {
        return Err(error("compact surface byte range is out of bounds"));
    }
    let root_offset = i32_at(data, 32, "compact surface tree root")?;
    if root_offset < 0 {
        return Err(error("compact surface tree root range is invalid"));
    }
    let tree_root = root_offset as usize;
    range(
        data,
        tree_root,
        TREE_NODE_BYTES,
        "compact surface tree root",
    )?;
    Ok(SurfaceMetadata {
        center_of_mass: ivp_position_to_source(vector_at(data, 0, "center of mass")?),
        rotation_inertia: ivp_inertia_to_source(vector_at(data, 12, "rotation inertia")?),
        upper_limit_radius: f32_at(data, 24, "upper limit radius")? / METERS_PER_SOURCE_UNIT,
        max_surface_deviation: (packed & 0xff) as u8,
        byte_size,
        tree_root,
        legacy_identifier: u32_at(data, 44, "legacy compact surface identifier")?,
    })
}

struct GeometryBudget {
    nodes: usize,
    convexes: usize,
    triangles: usize,
    vertices: usize,
}

fn add_budget(
    value: &mut usize,
    amount: usize,
    maximum: usize,
    label: &str,
) -> Result<(), PhyDecodeError> {
    *value = value
        .checked_add(amount)
        .ok_or_else(|| error(format!("{label} count overflows")))?;
    if *value > maximum {
        return Err(error(format!("{label} limit exceeded")));
    }
    Ok(())
}

fn parse_ledge(
    surface: &[u8],
    offset: usize,
    limits: DecodeLimits,
    budget: &mut GeometryBudget,
) -> Result<PhyConvex, PhyDecodeError> {
    range(surface, offset, LEDGE_HEADER_BYTES, "compact ledge header")?;
    let point_offset = i32_at(surface, offset, "compact ledge point offset")?;
    if point_offset < 0 {
        return Err(error("compact ledge point range is invalid"));
    }
    let client_data = i32_at(surface, offset + 4, "compact ledge client data")?;
    let packed = u32_at(surface, offset + 8, "compact ledge size")?;
    let ledge_size = ((packed >> 8) as usize)
        .checked_mul(16)
        .ok_or_else(|| error("compact ledge byte range overflows"))?;
    if ledge_size < LEDGE_HEADER_BYTES {
        return Err(error("compact ledge byte range is invalid"));
    }
    range(surface, offset, ledge_size, "compact ledge byte range")?;
    let ledge_end = offset
        .checked_add(ledge_size)
        .ok_or_else(|| error("compact ledge byte range overflows"))?;
    let triangle_count = i16_at(surface, offset + 12, "compact ledge triangle count")?;
    if triangle_count < 0 {
        return Err(error("compact ledge has a negative triangle count"));
    }
    let triangle_count = triangle_count as usize;
    add_budget(
        &mut budget.triangles,
        triangle_count,
        limits.max_triangles,
        "triangle",
    )?;
    let triangle_bytes = triangle_count
        .checked_mul(TRIANGLE_BYTES)
        .ok_or_else(|| error("compact triangle range overflows"))?;
    let triangle_start = offset
        .checked_add(LEDGE_HEADER_BYTES)
        .ok_or_else(|| error("compact triangle range overflows"))?;
    let triangle_end = triangle_start
        .checked_add(triangle_bytes)
        .ok_or_else(|| error("compact triangle range overflows"))?;
    if triangle_end > ledge_end {
        return Err(error("compact triangle table escapes its ledge"));
    }
    range(
        surface,
        triangle_start,
        triangle_bytes,
        "compact triangle table",
    )?;

    let mut faces = Vec::with_capacity(triangle_count);
    let mut maximum_point = None;
    for triangle_index in 0..triangle_count {
        let triangle = offset + LEDGE_HEADER_BYTES + triangle_index * TRIANGLE_BYTES;
        let metadata = u32_at(surface, triangle, "compact triangle metadata")?;
        let mut points = [0_u32; 3];
        for (edge_index, point) in points.iter_mut().enumerate() {
            let edge = u32_at(
                surface,
                triangle + 4 + edge_index * 4,
                "compact triangle edge",
            )?;
            *point = edge & 0xffff;
            maximum_point = Some(maximum_point.map_or(*point, |current: u32| current.max(*point)));
        }
        faces.push(PhyFace {
            indices: [points[2], points[1], points[0]],
            material_index: ((metadata >> 24) & 0x7f) as u8,
            is_virtual: metadata & 0x8000_0000 != 0,
        });
    }
    let mut point_remap = BTreeMap::new();
    for face in &faces {
        for point in face.indices {
            point_remap.entry(point).or_insert(0);
        }
    }
    for (index, remapped) in point_remap.values_mut().enumerate() {
        *remapped = index as u32;
    }
    let vertex_count = point_remap.len();
    add_budget(
        &mut budget.vertices,
        vertex_count,
        limits.max_vertices,
        "vertex",
    )?;
    let points_start = offset
        .checked_add(point_offset as usize)
        .ok_or_else(|| error("compact point range overflows"))?;
    let source_point_count = maximum_point.map_or(0, |value| value as usize + 1);
    let points_bytes = source_point_count
        .checked_mul(POINT_BYTES)
        .ok_or_else(|| error("compact point range overflows"))?;
    range(surface, points_start, points_bytes, "compact point range")?;
    let mut vertices = Vec::with_capacity(vertex_count);
    for point_index in point_remap.keys() {
        vertices.push(ivp_position_to_source(vector_at(
            surface,
            points_start + *point_index as usize * POINT_BYTES,
            "compact point",
        )?));
    }
    for face in &mut faces {
        for point in &mut face.indices {
            *point = point_remap[point];
        }
    }
    add_budget(&mut budget.convexes, 1, limits.max_convexes, "convex")?;
    Ok(PhyConvex {
        client_data,
        vertices,
        faces,
    })
}

fn decode_surface(
    data: &[u8],
    metadata: &SurfaceMetadata,
    limits: DecodeLimits,
) -> Result<Vec<PhyConvex>, PhyDecodeError> {
    let surface = range(data, 0, metadata.byte_size, "compact surface")?;
    let mut budget = GeometryBudget {
        nodes: 0,
        convexes: 0,
        triangles: 0,
        vertices: 0,
    };
    let mut pending = vec![(metadata.tree_root, 0_usize, false)];
    let mut active = HashSet::new();
    let mut complete = HashSet::new();
    let mut ledges = HashSet::new();
    let mut convexes = Vec::new();
    while let Some((node_offset, depth, exiting)) = pending.pop() {
        if exiting {
            active.remove(&node_offset);
            complete.insert(node_offset);
            continue;
        }
        if complete.contains(&node_offset) {
            continue;
        }
        if !active.insert(node_offset) {
            return Err(error("compact ledge tree contains a cycle"));
        }
        if depth > limits.max_tree_depth {
            return Err(error("compact ledge tree depth limit exceeded"));
        }
        add_budget(&mut budget.nodes, 1, limits.max_tree_nodes, "tree node")?;
        range(
            surface,
            node_offset,
            TREE_NODE_BYTES,
            "compact ledge tree node",
        )?;
        pending.push((node_offset, depth, true));
        let right_offset = i32_at(surface, node_offset, "compact ledge tree right child")?;
        let ledge_offset = i32_at(surface, node_offset + 4, "compact ledge tree ledge")?;
        if right_offset == 0 {
            if ledge_offset == 0 {
                return Err(error("terminal compact ledge tree node has no ledge"));
            }
            let ledge = node_offset
                .checked_add_signed(ledge_offset as isize)
                .ok_or_else(|| error("compact ledge range overflows"))?;
            if ledges.insert(ledge) {
                convexes.push(parse_ledge(surface, ledge, limits, &mut budget)?);
            }
            continue;
        }
        let left = node_offset
            .checked_add(TREE_NODE_BYTES)
            .ok_or_else(|| error("compact ledge tree left child range overflows"))?;
        let right = node_offset
            .checked_add_signed(right_offset as isize)
            .ok_or_else(|| error("compact ledge tree right child range overflows"))?;
        range(
            surface,
            left,
            TREE_NODE_BYTES,
            "compact ledge tree left child",
        )?;
        range(
            surface,
            right,
            TREE_NODE_BYTES,
            "compact ledge tree right child",
        )?;
        pending.push((left, depth + 1, false));
        pending.push((right, depth + 1, false));
    }
    if convexes.is_empty() {
        return Err(error("compact surface contains no polygon ledges"));
    }
    Ok(convexes)
}

fn unsupported_solid(
    solid_index: usize,
    byte_length: usize,
    encoding: PhySolidEncoding,
    status: UnsupportedShapeKind,
) -> PhySolid {
    PhySolid {
        solid_index,
        byte_length,
        encoding,
        status: PhyShapeStatus::Unsupported(status),
        center_of_mass: [0.0; 3],
        rotation_inertia: [0.0; 3],
        upper_limit_radius: 0.0,
        max_surface_deviation: 0,
        drag_axis_areas: None,
        axis_map_size: None,
        convexes: Vec::new(),
    }
}

fn decoded_solid(
    solid_index: usize,
    byte_length: usize,
    encoding: PhySolidEncoding,
    surface: &[u8],
    drag_axis_areas: Option<[f32; 3]>,
    axis_map_size: Option<usize>,
    limits: DecodeLimits,
) -> Result<PhySolid, PhyDecodeError> {
    let metadata = surface_metadata(surface)?;
    let convexes = decode_surface(surface, &metadata, limits)?;
    Ok(PhySolid {
        solid_index,
        byte_length,
        encoding,
        status: PhyShapeStatus::Decoded,
        center_of_mass: metadata.center_of_mass,
        rotation_inertia: metadata.rotation_inertia,
        upper_limit_radius: metadata.upper_limit_radius,
        max_surface_deviation: metadata.max_surface_deviation,
        drag_axis_areas,
        axis_map_size,
        convexes,
    })
}

fn parse_solid(
    body: &[u8],
    solid_index: usize,
    limits: DecodeLimits,
) -> Result<PhySolid, PhyDecodeError> {
    if body.len() >= 4 && u32_at(body, 0, "solid identifier")? == YHPV {
        return Ok(unsupported_solid(
            solid_index,
            body.len(),
            PhySolidEncoding::ModernUnsupported,
            UnsupportedShapeKind::SwappedEndian,
        ));
    }
    if body.len() >= 8 && u32_at(body, 0, "solid identifier")? == VPHY {
        let version = i16_at(body, 4, "solid version")?;
        if version != 0x0100 {
            return Err(error(format!(
                "solid {solid_index} has unsupported version {version:#x}"
            )));
        }
        let model_type = i16_at(body, 6, "solid model type")?;
        let unsupported = match model_type {
            1 => Some(UnsupportedShapeKind::Mopp),
            2 => Some(UnsupportedShapeKind::Ball),
            3 => Some(UnsupportedShapeKind::Virtual),
            value if value != 0 => Some(UnsupportedShapeKind::Unknown),
            _ => None,
        };
        if let Some(kind) = unsupported {
            return Ok(unsupported_solid(
                solid_index,
                body.len(),
                PhySolidEncoding::ModernUnsupported,
                kind,
            ));
        }
        range(body, 0, MODERN_HEADER_BYTES, "modern polygon header")?;
        let surface_size = i32_at(body, 8, "modern polygon surface size")?;
        let axis_map_size = i32_at(body, 24, "modern polygon axis map size")?;
        if surface_size < 0 || axis_map_size < 0 {
            return Err(error("modern polygon header has a negative byte range"));
        }
        let surface_size = surface_size as usize;
        let axis_map_size = axis_map_size as usize;
        let surface = range(
            body,
            MODERN_HEADER_BYTES,
            surface_size,
            "modern polygon surface",
        )?;
        let axis_start = MODERN_HEADER_BYTES
            .checked_add(surface_size)
            .ok_or_else(|| error("modern polygon axis map range overflows"))?;
        range(body, axis_start, axis_map_size, "modern polygon axis map")?;
        let declared_end = axis_start
            .checked_add(axis_map_size)
            .ok_or_else(|| error("modern polygon byte range overflows"))?;
        if declared_end != body.len() {
            return Err(error("modern polygon solid has trailing bytes"));
        }
        decoded_solid(
            solid_index,
            body.len(),
            PhySolidEncoding::ModernPolygon,
            surface,
            Some(vector_at(body, 12, "modern polygon drag axis areas")?),
            Some(axis_map_size),
            limits,
        )
    } else {
        let metadata = surface_metadata(body)?;
        match metadata.legacy_identifier {
            MOPP => Ok(unsupported_solid(
                solid_index,
                body.len(),
                PhySolidEncoding::LegacyUnsupported,
                UnsupportedShapeKind::Mopp,
            )),
            SVPI => Ok(unsupported_solid(
                solid_index,
                body.len(),
                PhySolidEncoding::LegacyUnsupported,
                UnsupportedShapeKind::SwappedEndian,
            )),
            IVPS | 0 => decoded_solid(
                solid_index,
                body.len(),
                PhySolidEncoding::LegacyCompact,
                body,
                None,
                None,
                limits,
            ),
            _ => Ok(unsupported_solid(
                solid_index,
                body.len(),
                PhySolidEncoding::LegacyUnsupported,
                UnsupportedShapeKind::Unknown,
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Open,
    Close,
}

fn tokenize_key_data(source: &str, limits: DecodeLimits) -> Result<Vec<Token>, PhyDecodeError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'/') {
            cursor += 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            continue;
        }
        let token = match bytes[cursor] {
            b'{' => {
                cursor += 1;
                Token::Open
            }
            b'}' => {
                cursor += 1;
                Token::Close
            }
            b'"' => {
                cursor += 1;
                let mut value = Vec::new();
                while cursor < bytes.len() && bytes[cursor] != b'"' {
                    if bytes[cursor] == b'\\' && bytes.get(cursor + 1) == Some(&b'"') {
                        cursor += 1;
                    }
                    value.push(bytes[cursor]);
                    if value.len() > limits.max_key_string_bytes {
                        return Err(error("keydata string limit exceeded"));
                    }
                    cursor += 1;
                }
                if bytes.get(cursor) != Some(&b'"') {
                    return Err(error("keydata quoted string is truncated"));
                }
                cursor += 1;
                Token::Word(
                    String::from_utf8(value)
                        .map_err(|_| error("keydata quoted string is not valid UTF-8"))?,
                )
            }
            _ => {
                let start = cursor;
                while cursor < bytes.len()
                    && !bytes[cursor].is_ascii_whitespace()
                    && bytes[cursor] != b'{'
                    && bytes[cursor] != b'}'
                {
                    cursor += 1;
                }
                if cursor - start > limits.max_key_string_bytes {
                    return Err(error("keydata string limit exceeded"));
                }
                Token::Word(source[start..cursor].to_owned())
            }
        };
        tokens.push(token);
        if tokens.len() > limits.max_key_tokens {
            return Err(error("keydata token limit exceeded"));
        }
    }
    Ok(tokens)
}

fn key_word(tokens: &[Token], cursor: &mut usize, context: &str) -> Result<String, PhyDecodeError> {
    match tokens.get(*cursor) {
        Some(Token::Word(value)) => {
            *cursor += 1;
            Ok(value.clone())
        }
        _ => Err(error(format!("{context} is truncated"))),
    }
}

fn parse_key_entries(
    tokens: &[Token],
    cursor: &mut usize,
    depth: usize,
    limits: DecodeLimits,
) -> Result<Vec<PhyKeyValue>, PhyDecodeError> {
    if depth > limits.max_key_depth {
        return Err(error("keydata depth limit exceeded"));
    }
    let mut entries = Vec::new();
    loop {
        if matches!(tokens.get(*cursor), Some(Token::Close)) {
            *cursor += 1;
            return Ok(entries);
        }
        let key = key_word(tokens, cursor, "keydata property")?;
        match tokens.get(*cursor) {
            Some(Token::Open) => {
                *cursor += 1;
                entries.push(PhyKeyValue::Block {
                    key,
                    entries: parse_key_entries(tokens, cursor, depth + 1, limits)?,
                });
            }
            Some(Token::Word(value)) => {
                entries.push(PhyKeyValue::Scalar {
                    key,
                    value: value.clone(),
                });
                *cursor += 1;
            }
            _ => return Err(error("keydata value is truncated")),
        }
    }
}

fn parse_key_blocks(
    source: &str,
    limits: DecodeLimits,
) -> Result<Vec<PhyKeyBlock>, PhyDecodeError> {
    let tokens = tokenize_key_data(source, limits)?;
    let mut cursor = 0;
    let mut blocks = Vec::new();
    while cursor < tokens.len() {
        let name = key_word(&tokens, &mut cursor, "keydata block name")?;
        if !matches!(tokens.get(cursor), Some(Token::Open)) {
            return Err(error("keydata block opening brace is truncated"));
        }
        cursor += 1;
        blocks.push(PhyKeyBlock {
            name,
            entries: parse_key_entries(&tokens, &mut cursor, 1, limits)?,
        });
    }
    Ok(blocks)
}

fn scalar(entry: &PhyKeyValue) -> Option<(&str, &str)> {
    match entry {
        PhyKeyValue::Scalar { key, value } => Some((key, value)),
        PhyKeyValue::Block { .. } => None,
    }
}

fn parse_finite(value: &str, key: &str) -> Result<f32, PhyDecodeError> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| error(format!("solid key {key} is not a float")))?;
    if !parsed.is_finite() {
        return Err(error(format!("solid key {key} is not finite")));
    }
    Ok(parsed)
}

fn parse_vector(value: &str, key: &str) -> Result<[f32; 3], PhyDecodeError> {
    let parts: Vec<_> = value.split_ascii_whitespace().collect();
    if parts.len() != 3 {
        return Err(error(format!("solid key {key} is not a vector")));
    }
    Ok([
        parse_finite(parts[0], key)?,
        parse_finite(parts[1], key)?,
        parse_finite(parts[2], key)?,
    ])
}

fn typed_key_data(source: &str, limits: DecodeLimits) -> Result<PhyTypedKeyData, PhyDecodeError> {
    let mut output = PhyTypedKeyData::default();
    for block in parse_key_blocks(source, limits)? {
        if !block.name.eq_ignore_ascii_case("solid") {
            output.unknown_blocks.push(block);
            continue;
        }
        let mut solid = PhySolidKeyData::default();
        for entry in block.entries {
            let Some((key, value)) = scalar(&entry) else {
                solid.unknown.push(entry);
                continue;
            };
            match key.to_ascii_lowercase().as_str() {
                "index" => {
                    solid.index = Some(
                        value
                            .parse()
                            .map_err(|_| error("solid key index is not an integer"))?,
                    )
                }
                "name" => solid.name = Some(value.to_owned()),
                "parent" => solid.parent = Some(value.to_owned()),
                "surfaceprop" => solid.surface_prop = Some(value.to_owned()),
                "mass" => solid.mass = Some(parse_finite(value, key)?),
                "masscenteroverride" => {
                    solid.mass_center_override = Some(parse_vector(value, key)?)
                }
                "damping" => solid.damping = Some(parse_finite(value, key)?),
                "rotdamping" => solid.rotation_damping = Some(parse_finite(value, key)?),
                "drag" => solid.drag = Some(parse_finite(value, key)?),
                "inertia" => solid.inertia = Some(parse_finite(value, key)?),
                "rotinertialimit" => solid.rotation_inertia_limit = Some(parse_finite(value, key)?),
                "volume" => solid.volume = Some(parse_finite(value, key)?),
                _ => solid.unknown.push(entry),
            }
        }
        output.solids.push(solid);
    }
    Ok(output)
}

fn decode_key_data(data: &[u8], limits: DecodeLimits) -> Result<PhyTypedKeyData, PhyDecodeError> {
    if data.len() > limits.max_key_data_bytes {
        return Err(error("PHY keydata byte limit exceeded"));
    }
    let mut end = data.len();
    while end > 0 && data[end - 1] == 0 {
        end -= 1;
    }
    let source =
        std::str::from_utf8(&data[..end]).map_err(|_| error("PHY keydata is not valid UTF-8"))?;
    typed_key_data(source, limits)
}

fn decode_solid_stream(
    data: &[u8],
    solid_count: usize,
    limits: DecodeLimits,
) -> Result<Vec<PhySolid>, PhyDecodeError> {
    if solid_count > limits.max_solids {
        return Err(error("PHY solid limit exceeded"));
    }
    let mut cursor = 0;
    let mut solids = Vec::with_capacity(solid_count);
    for solid_index in 0..solid_count {
        let size = i32_at(data, cursor, "PHY solid size")?;
        if size < 0 || size as usize > limits.max_solid_bytes {
            return Err(error(format!(
                "PHY solid {solid_index} byte limit exceeded"
            )));
        }
        cursor = cursor
            .checked_add(4)
            .ok_or_else(|| error("PHY solid range overflows"))?;
        let body = range(data, cursor, size as usize, "PHY solid body")?;
        solids.push(parse_solid(body, solid_index, limits)?);
        cursor = cursor
            .checked_add(size as usize)
            .ok_or_else(|| error("PHY solid range overflows"))?;
    }
    if cursor != data.len() {
        return Err(error("PHY solid stream has trailing bytes"));
    }
    Ok(solids)
}

pub fn decode_physcollide(
    collision_data: &[u8],
    key_data: &[u8],
    solid_count: usize,
    limits: DecodeLimits,
) -> Result<PhyCollide, PhyDecodeError> {
    if collision_data.len() > limits.max_file_bytes {
        return Err(error("PHYSCOLLIDE byte limit exceeded"));
    }
    Ok(PhyCollide {
        solids: decode_solid_stream(collision_data, solid_count, limits)?,
        key_data: decode_key_data(key_data, limits)?,
    })
}

pub fn decode_phy(data: &[u8], limits: DecodeLimits) -> Result<PhyFile, PhyDecodeError> {
    if data.len() > limits.max_file_bytes {
        return Err(error("PHY file byte limit exceeded"));
    }
    range(data, 0, PHY_HEADER_BYTES, "PHY header")?;
    let header_size = i32_at(data, 0, "PHY header size")?;
    let solid_count = i32_at(data, 8, "PHY solid count")?;
    if header_size < PHY_HEADER_BYTES as i32 || header_size as usize > data.len() {
        return Err(error("PHY header byte range is invalid"));
    }
    if solid_count < 0 || solid_count as usize > limits.max_solids {
        return Err(error("PHY solid limit exceeded"));
    }
    let header = PhyHeader {
        header_size: header_size as usize,
        id: i32_at(data, 4, "PHY identifier")?,
        solid_count: solid_count as usize,
        checksum: i32_at(data, 12, "PHY checksum")?,
    };
    let mut cursor = header.header_size;
    for solid_index in 0..header.solid_count {
        let size = i32_at(data, cursor, "PHY solid size")?;
        if size < 0 || size as usize > limits.max_solid_bytes {
            return Err(error(format!(
                "PHY solid {solid_index} byte limit exceeded"
            )));
        }
        cursor = cursor
            .checked_add(4)
            .and_then(|value| value.checked_add(size as usize))
            .ok_or_else(|| error("PHY solid range overflows"))?;
        if cursor > data.len() {
            return Err(error("PHY solid body is truncated"));
        }
    }
    let collide = decode_physcollide(
        range(
            data,
            header.header_size,
            cursor - header.header_size,
            "PHY solid stream",
        )?,
        range(data, cursor, data.len() - cursor, "PHY keydata")?,
        header.solid_count,
        limits,
    )?;
    Ok(PhyFile {
        header,
        solids: collide.solids,
        key_data: collide.key_data,
    })
}
