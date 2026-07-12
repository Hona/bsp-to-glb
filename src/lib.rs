use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;

const LUMP_ENTITIES: usize = 0;
const LUMP_PLANES: usize = 1;
const LUMP_TEXDATA: usize = 2;
const LUMP_VERTEXES: usize = 3;
const LUMP_TEXINFO: usize = 6;
const LUMP_FACES: usize = 7;
const LUMP_EDGES: usize = 12;
const LUMP_SURFEDGES: usize = 13;
const LUMP_MODELS: usize = 14;
const LUMP_VERTNORMALS: usize = 30;
const LUMP_VERTNORMALINDICES: usize = 31;
const LUMP_PRIMITIVES: usize = 37;
const LUMP_PRIMVERTS: usize = 38;
const LUMP_PRIMINDICES: usize = 39;
const LUMP_FACES_HDR: usize = 58;
const LUMP_TEXDATA_STRING_DATA: usize = 43;
const LUMP_TEXDATA_STRING_TABLE: usize = 44;

const FACE_SIZE: usize = 56;
const TEXINFO_SIZE: usize = 72;
const TEXDATA_SIZE: usize = 32;
const MODEL_SIZE: usize = 48;
const PRIMITIVE_SIZE: usize = 10;

const SURF_SKY2D: i32 = 0x0002;
const SURF_SKY: i32 = 0x0004;
const SURF_TRIGGER: i32 = 0x0040;
const SURF_NODRAW: i32 = 0x0080;
const SURF_HINT: i32 = 0x0100;
const SURF_SKIP: i32 = 0x0200;
const SURF_NOLIGHT: i32 = 0x0400;
const NON_RENDERED_SURFACE_FLAGS: i32 =
    SURF_SKY2D | SURF_SKY | SURF_TRIGGER | SURF_NODRAW | SURF_HINT | SURF_SKIP;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStats {
    pub models: usize,
    pub meshes: usize,
    pub primitives: usize,
    pub faces: usize,
    pub triangles: usize,
    pub vertices: usize,
    pub materials: usize,
    pub lightmapped_faces: usize,
    pub displacement_faces: usize,
    pub compiled_primitive_faces: usize,
    pub fan_faces: usize,
    pub compiled_normal_vertices: usize,
    pub compiled_normal_opposed_vertices: usize,
    pub initially_rendered_faces: usize,
}

#[derive(Debug)]
pub struct ExportResult {
    pub glb: Vec<u8>,
    pub stats: ExportStats,
}

#[derive(Clone, Copy)]
struct LumpHeader {
    offset: usize,
    length: usize,
    uncompressed_size: usize,
}

struct Bsp {
    version: i32,
    lumps: Vec<Vec<u8>>,
}

#[derive(Clone, Copy)]
struct Plane {
    normal: [f32; 3],
}

#[derive(Clone)]
struct TexInfo {
    texture_vecs: [[f32; 4]; 2],
    lightmap_vecs: [[f32; 4]; 2],
    flags: i32,
    texdata: i32,
}

#[derive(Clone, Copy)]
struct TexData {
    name_index: i32,
    width: i32,
    height: i32,
}

#[derive(Clone, Copy)]
struct Face {
    plane: usize,
    side: bool,
    first_edge: i32,
    num_edges: i16,
    texinfo: i16,
    dispinfo: i16,
    styles: [u8; 4],
    light_offset: i32,
    lightmap_mins: [i32; 2],
    lightmap_size: [i32; 2],
    num_primitives: u16,
    first_primitive: u16,
}

#[derive(Clone, Copy)]
struct BspPrimitive {
    primitive_type: u8,
    first_index: u16,
    index_count: u16,
    first_vertex: u16,
    vertex_count: u16,
}

#[derive(Clone, Copy)]
struct Model {
    mins: [f32; 3],
    maxs: [f32; 3],
    origin: [f32; 3],
    first_face: i32,
    num_faces: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LightmapMetadata {
    atlas_width: f32,
    atlas_height: f32,
    #[serde(default)]
    faces: Vec<LightmapFace>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LightmapFace {
    face_index: usize,
    w: f32,
    h: f32,
    atlas_x: f32,
    atlas_y: f32,
    lm_vecs: [[f32; 4]; 2],
    lm_mins_s: f32,
    lm_mins_t: f32,
    #[serde(default)]
    verts: Vec<[f64; 3]>,
}

struct LightmapLookup {
    metadata: LightmapMetadata,
    vertex_sets: Vec<HashSet<[i32; 3]>>,
    by_vertex: HashMap<[i32; 3], Vec<usize>>,
    by_face: HashMap<usize, usize>,
}

#[derive(Default)]
struct PrimitiveData {
    positions: Vec<f32>,
    normals: Vec<f32>,
    uv0: Vec<f32>,
    uv1: Vec<f32>,
    indices: Vec<u32>,
    face_indices: Vec<usize>,
    face_vertex_counts: Vec<usize>,
    face_styles: Vec<[u8; 4]>,
    face_light_offsets: Vec<i32>,
    face_lightmap_mins: Vec<[i32; 2]>,
    face_lightmap_sizes: Vec<[i32; 2]>,
    triangles: usize,
}

fn read_i16(data: &[u8], offset: usize, context: &str) -> Result<i16, String> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| format!("truncated {context} at byte {offset}"))?;
    Ok(i16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u16(data: &[u8], offset: usize, context: &str) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| format!("truncated {context} at byte {offset}"))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_i32(data: &[u8], offset: usize, context: &str) -> Result<i32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated {context} at byte {offset}"))?;
    Ok(i32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(data: &[u8], offset: usize, context: &str) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated {context} at byte {offset}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_f32(data: &[u8], offset: usize, context: &str) -> Result<f32, String> {
    Ok(f32::from_bits(read_u32(data, offset, context)?))
}

fn parse_bsp(data: &[u8]) -> Result<Bsp, String> {
    if data.len() < 1036 || data.get(0..4) != Some(b"VBSP") {
        return Err("input is not a complete Valve BSP file".to_owned());
    }
    let version = read_i32(data, 4, "BSP version")?;
    let mut headers = Vec::with_capacity(64);
    for index in 0..64 {
        let offset = 8 + index * 16;
        let file_offset = read_i32(data, offset, "lump table")?;
        let file_length = read_i32(data, offset + 4, "lump table")?;
        let uncompressed_size = read_i32(data, offset + 12, "lump table")?;
        if file_offset < 0 || file_length < 0 || uncompressed_size < 0 {
            return Err(format!("lump {index} has a negative offset or length"));
        }
        headers.push(LumpHeader {
            offset: file_offset as usize,
            length: file_length as usize,
            uncompressed_size: uncompressed_size as usize,
        });
    }

    let mut lumps = Vec::with_capacity(64);
    for (index, header) in headers.iter().enumerate() {
        if header.length == 0 {
            lumps.push(Vec::new());
            continue;
        }
        let end = header
            .offset
            .checked_add(header.length)
            .ok_or_else(|| format!("lump {index} range overflows"))?;
        let raw = data
            .get(header.offset..end)
            .ok_or_else(|| format!("lump {index} extends past the BSP file"))?;
        if raw.get(0..4) == Some(b"LZMA") {
            if raw.len() < 17 {
                return Err(format!(
                    "compressed lump {index} has a truncated LZMA header"
                ));
            }
            let expected = read_u32(raw, 4, "LZMA size")? as usize;
            let compressed = read_u32(raw, 8, "LZMA size")? as usize;
            let compressed_data = raw
                .get(17..17 + compressed)
                .ok_or_else(|| format!("compressed lump {index} is truncated"))?;
            let mut alone = Vec::with_capacity(13 + compressed);
            alone.extend_from_slice(&raw[12..17]);
            alone.extend_from_slice(&(expected as u64).to_le_bytes());
            alone.extend_from_slice(compressed_data);
            let mut output = Vec::with_capacity(expected);
            lzma_rs::lzma_decompress(&mut Cursor::new(alone), &mut output)
                .map_err(|error| format!("failed to decompress lump {index}: {error}"))?;
            if output.len() != expected
                || (header.uncompressed_size != 0 && output.len() != header.uncompressed_size)
            {
                return Err(format!(
                    "compressed lump {index} size mismatch: decoded {}, expected {expected}",
                    output.len()
                ));
            }
            lumps.push(output);
        } else {
            lumps.push(raw.to_vec());
        }
    }
    Ok(Bsp { version, lumps })
}

fn parse_vec3_lump(data: &[u8], label: &str) -> Result<Vec<[f32; 3]>, String> {
    if !data.len().is_multiple_of(12) {
        return Err(format!(
            "{label} lump length {} is not divisible by 12",
            data.len()
        ));
    }
    (0..data.len() / 12)
        .map(|index| {
            let offset = index * 12;
            Ok([
                read_f32(data, offset, label)?,
                read_f32(data, offset + 4, label)?,
                read_f32(data, offset + 8, label)?,
            ])
        })
        .collect()
}

fn parse_planes(data: &[u8]) -> Result<Vec<Plane>, String> {
    if !data.len().is_multiple_of(20) {
        return Err("plane lump length is not divisible by 20".to_owned());
    }
    (0..data.len() / 20)
        .map(|index| {
            let offset = index * 20;
            Ok(Plane {
                normal: [
                    read_f32(data, offset, "plane")?,
                    read_f32(data, offset + 4, "plane")?,
                    read_f32(data, offset + 8, "plane")?,
                ],
            })
        })
        .collect()
}

fn parse_texinfo(data: &[u8]) -> Result<Vec<TexInfo>, String> {
    if !data.len().is_multiple_of(TEXINFO_SIZE) {
        return Err("texinfo lump length is not divisible by 72".to_owned());
    }
    (0..data.len() / TEXINFO_SIZE)
        .map(|index| {
            let offset = index * TEXINFO_SIZE;
            let mut vectors = [[0.0; 4]; 4];
            for (vector, output) in vectors.iter_mut().enumerate() {
                for (axis, value) in output.iter_mut().enumerate() {
                    *value = read_f32(data, offset + vector * 16 + axis * 4, "texinfo")?;
                }
            }
            Ok(TexInfo {
                texture_vecs: [vectors[0], vectors[1]],
                lightmap_vecs: [vectors[2], vectors[3]],
                flags: read_i32(data, offset + 64, "texinfo")?,
                texdata: read_i32(data, offset + 68, "texinfo")?,
            })
        })
        .collect()
}

fn parse_texdata(data: &[u8]) -> Result<Vec<TexData>, String> {
    if !data.len().is_multiple_of(TEXDATA_SIZE) {
        return Err("texdata lump length is not divisible by 32".to_owned());
    }
    (0..data.len() / TEXDATA_SIZE)
        .map(|index| {
            let offset = index * TEXDATA_SIZE;
            Ok(TexData {
                name_index: read_i32(data, offset + 12, "texdata")?,
                width: read_i32(data, offset + 16, "texdata")?,
                height: read_i32(data, offset + 20, "texdata")?,
            })
        })
        .collect()
}

fn parse_faces(data: &[u8]) -> Result<Vec<Face>, String> {
    if !data.len().is_multiple_of(FACE_SIZE) {
        return Err("face lump length is not divisible by 56".to_owned());
    }
    (0..data.len() / FACE_SIZE)
        .map(|index| {
            let offset = index * FACE_SIZE;
            Ok(Face {
                plane: read_u16(data, offset, "face")? as usize,
                side: data[offset + 2] != 0,
                first_edge: read_i32(data, offset + 4, "face")?,
                num_edges: read_i16(data, offset + 8, "face")?,
                texinfo: read_i16(data, offset + 10, "face")?,
                dispinfo: read_i16(data, offset + 12, "face")?,
                styles: data[offset + 16..offset + 20].try_into().unwrap(),
                light_offset: read_i32(data, offset + 20, "face")?,
                lightmap_mins: [
                    read_i32(data, offset + 28, "face")?,
                    read_i32(data, offset + 32, "face")?,
                ],
                lightmap_size: [
                    read_i32(data, offset + 36, "face")?,
                    read_i32(data, offset + 40, "face")?,
                ],
                num_primitives: read_u16(data, offset + 48, "face")? & 0x7fff,
                first_primitive: read_u16(data, offset + 50, "face")?,
            })
        })
        .collect()
}

fn parse_primitives(data: &[u8]) -> Result<Vec<BspPrimitive>, String> {
    if !data.len().is_multiple_of(PRIMITIVE_SIZE) {
        return Err("primitive lump length is not divisible by 10".to_owned());
    }
    (0..data.len() / PRIMITIVE_SIZE)
        .map(|index| {
            let offset = index * PRIMITIVE_SIZE;
            Ok(BspPrimitive {
                primitive_type: data[offset],
                first_index: read_u16(data, offset + 2, "primitive")?,
                index_count: read_u16(data, offset + 4, "primitive")?,
                first_vertex: read_u16(data, offset + 6, "primitive")?,
                vertex_count: read_u16(data, offset + 8, "primitive")?,
            })
        })
        .collect()
}

fn parse_models(data: &[u8]) -> Result<Vec<Model>, String> {
    if !data.len().is_multiple_of(MODEL_SIZE) {
        return Err("model lump length is not divisible by 48".to_owned());
    }
    (0..data.len() / MODEL_SIZE)
        .map(|index| {
            let offset = index * MODEL_SIZE;
            let vector = |start| -> Result<[f32; 3], String> {
                Ok([
                    read_f32(data, offset + start, "model")?,
                    read_f32(data, offset + start + 4, "model")?,
                    read_f32(data, offset + start + 8, "model")?,
                ])
            };
            Ok(Model {
                mins: vector(0)?,
                maxs: vector(12)?,
                origin: vector(24)?,
                first_face: read_i32(data, offset + 40, "model")?,
                num_faces: read_i32(data, offset + 44, "model")?,
            })
        })
        .collect()
}

fn parse_i32_lump(data: &[u8], label: &str) -> Result<Vec<i32>, String> {
    if !data.len().is_multiple_of(4) {
        return Err(format!("{label} lump length is not divisible by 4"));
    }
    (0..data.len() / 4)
        .map(|index| read_i32(data, index * 4, label))
        .collect()
}

fn parse_u16_lump(data: &[u8], label: &str) -> Result<Vec<u16>, String> {
    if !data.len().is_multiple_of(2) {
        return Err(format!("{label} lump length is not divisible by 2"));
    }
    (0..data.len() / 2)
        .map(|index| read_u16(data, index * 2, label))
        .collect()
}

fn parse_edges(data: &[u8]) -> Result<Vec<[u16; 2]>, String> {
    if !data.len().is_multiple_of(4) {
        return Err("edge lump length is not divisible by 4".to_owned());
    }
    (0..data.len() / 4)
        .map(|index| {
            Ok([
                read_u16(data, index * 4, "edge")?,
                read_u16(data, index * 4 + 2, "edge")?,
            ])
        })
        .collect()
}

fn parse_material_names(
    texdata: &[TexData],
    string_data: &[u8],
    string_table: &[u8],
) -> Result<Vec<String>, String> {
    let offsets = parse_i32_lump(string_table, "texdata string table")?;
    texdata
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let table_index = usize::try_from(item.name_index)
                .map_err(|_| format!("texdata {index} has a negative material string index"))?;
            let offset = *offsets
                .get(table_index)
                .ok_or_else(|| format!("texdata {index} material string index is out of range"))?;
            let start = usize::try_from(offset)
                .map_err(|_| format!("texdata {index} has a negative material string offset"))?;
            let tail = string_data
                .get(start..)
                .ok_or_else(|| format!("texdata {index} material string offset is out of range"))?;
            let end = tail
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(tail.len());
            let name = String::from_utf8_lossy(&tail[..end]).replace('\\', "/");
            if name.is_empty() {
                Err(format!("texdata {index} has an empty material name"))
            } else {
                Ok(name)
            }
        })
        .collect()
}

fn tokenize_entities(text: &str) -> Result<Vec<String>, String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() || bytes[index] == 0 {
            index += 1;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'{' || bytes[index] == b'}' {
            tokens.push((bytes[index] as char).to_string());
            index += 1;
            continue;
        }
        if bytes[index] != b'"' {
            return Err(format!("unexpected entity byte at offset {index}"));
        }
        index += 1;
        let mut value = String::new();
        let mut closed = false;
        while index < bytes.len() {
            match bytes[index] {
                b'"' => {
                    index += 1;
                    closed = true;
                    break;
                }
                b'\\' if index + 1 < bytes.len() => {
                    index += 1;
                    value.push(bytes[index] as char);
                    index += 1;
                }
                byte => {
                    value.push(byte as char);
                    index += 1;
                }
            }
        }
        if !closed {
            return Err("unterminated entity string".to_owned());
        }
        tokens.push(value);
    }
    Ok(tokens)
}

fn parse_entities(data: &[u8]) -> Result<Vec<HashMap<String, String>>, String> {
    let text = String::from_utf8_lossy(data);
    let tokens = tokenize_entities(&text)?;
    let mut entities = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index] != "{" {
            return Err(format!("expected entity opening brace at token {index}"));
        }
        index += 1;
        let mut entity = HashMap::new();
        while index < tokens.len() && tokens[index] != "}" {
            let key = tokens[index].clone();
            let value = tokens
                .get(index + 1)
                .ok_or_else(|| "entity key has no value".to_owned())?
                .clone();
            if value == "{" || value == "}" {
                return Err("entity key has an invalid value".to_owned());
            }
            entity.insert(key, value);
            index += 2;
        }
        if tokens.get(index).map(String::as_str) != Some("}") {
            return Err("entity is missing its closing brace".to_owned());
        }
        index += 1;
        entities.push(entity);
    }
    Ok(entities)
}

fn source_to_gltf(value: [f32; 3]) -> [f32; 3] {
    [value[0], value[2], -value[1]]
}

fn dot4(vector: [f32; 4], position: [f32; 3]) -> f32 {
    vector[0] * position[0] + vector[1] * position[1] + vector[2] * position[2] + vector[3]
}

fn quantize_vertex(value: [f32; 3]) -> [i32; 3] {
    quantize_metadata_vertex(value.map(f64::from))
}

fn quantize_metadata_vertex(value: [f64; 3]) -> [i32; 3] {
    // lightmap_data.json is produced with JavaScript Math.round, which rounds
    // negative half values toward +infinity rather than away from zero.
    let round_like_javascript = |number: f64| (number + 0.5).floor() as i32;
    [
        round_like_javascript(value[0] * 1000.0),
        round_like_javascript(value[1] * 1000.0),
        round_like_javascript(value[2] * 1000.0),
    ]
}

impl LightmapLookup {
    fn parse(data: &[u8]) -> Result<Self, String> {
        let metadata: LightmapMetadata = serde_json::from_slice(data)
            .map_err(|error| format!("invalid lightmap metadata: {error}"))?;
        if metadata.atlas_width <= 0.0 || metadata.atlas_height <= 0.0 {
            return Err("lightmap atlas dimensions must be positive".to_owned());
        }
        let mut vertex_sets = Vec::with_capacity(metadata.faces.len());
        let mut by_vertex: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
        let mut by_face = HashMap::new();
        for (index, face) in metadata.faces.iter().enumerate() {
            if face.w <= 0.0 || face.h <= 0.0 {
                return Err(format!(
                    "lightmap face {} has invalid dimensions {}x{}",
                    face.face_index, face.w, face.h
                ));
            }
            let set: HashSet<_> = face
                .verts
                .iter()
                .copied()
                .map(quantize_metadata_vertex)
                .collect();
            for vertex in &set {
                by_vertex.entry(*vertex).or_default().push(index);
            }
            by_face.insert(face.face_index, index);
            vertex_sets.push(set);
        }
        Ok(Self {
            metadata,
            vertex_sets,
            by_vertex,
            by_face,
        })
    }

    fn vectors_match(mapping: &LightmapFace, texinfo: &TexInfo) -> bool {
        let mut error = 0.0;
        for row in 0..2 {
            for axis in 0..4 {
                error += (mapping.lm_vecs[row][axis] - texinfo.lightmap_vecs[row][axis]).abs();
            }
        }
        error < 0.001
    }

    fn find(&self, face_index: usize, positions: &[[f32; 3]], texinfo: &TexInfo) -> Option<usize> {
        let keys: Vec<_> = positions.iter().copied().map(quantize_vertex).collect();
        let accepts = |index: usize| {
            let mapping = &self.metadata.faces[index];
            Self::vectors_match(mapping, texinfo)
                && keys.iter().all(|key| self.vertex_sets[index].contains(key))
        };
        if let Some(index) = self.by_face.get(&face_index).copied()
            && accepts(index)
        {
            return Some(index);
        }
        keys.first()
            .and_then(|key| self.by_vertex.get(key))
            .and_then(|candidates| candidates.iter().copied().find(|index| accepts(*index)))
    }

    fn uv(&self, mapping_index: usize, position: [f32; 3]) -> [f32; 2] {
        let face = &self.metadata.faces[mapping_index];
        let luxel_s = dot4(face.lm_vecs[0], position);
        let luxel_t = dot4(face.lm_vecs[1], position);
        [
            (face.atlas_x + luxel_s - face.lm_mins_s + 0.5) / self.metadata.atlas_width,
            (face.atlas_y + luxel_t - face.lm_mins_t + 0.5) / self.metadata.atlas_height,
        ]
    }
}

fn face_positions(
    face: Face,
    surfedges: &[i32],
    edges: &[[u16; 2]],
    vertices: &[[f32; 3]],
    face_index: usize,
) -> Result<Vec<[f32; 3]>, String> {
    if face.first_edge < 0 || face.num_edges < 3 {
        return Err(format!("face {face_index} has an invalid edge range"));
    }
    let first = face.first_edge as usize;
    let count = face.num_edges as usize;
    let face_surfedges = surfedges
        .get(first..first + count)
        .ok_or_else(|| format!("face {face_index} surfedge range is out of bounds"))?;
    face_surfedges
        .iter()
        .map(|surfedge| {
            let edge_index = surfedge.unsigned_abs() as usize;
            let edge = edges
                .get(edge_index)
                .ok_or_else(|| format!("face {face_index} references missing edge {edge_index}"))?;
            let vertex_index = if *surfedge >= 0 { edge[0] } else { edge[1] } as usize;
            vertices.get(vertex_index).copied().ok_or_else(|| {
                format!("face {face_index} references missing vertex {vertex_index}")
            })
        })
        .collect()
}

fn face_normals(
    face: Face,
    normal_start: usize,
    vertex_normal_indices: &[u16],
    vertex_normals: &[[f32; 3]],
    fallback: [f32; 3],
    face_index: usize,
) -> Result<(Vec<[f32; 3]>, bool), String> {
    if vertex_normal_indices.is_empty() && vertex_normals.is_empty() {
        return Ok((vec![fallback; face.num_edges as usize], false));
    }
    if vertex_normal_indices.is_empty() || vertex_normals.is_empty() {
        return Err("BSP has only one of VERTNORMALS and VERTNORMALINDICES".to_owned());
    }
    let indices = vertex_normal_indices
        .get(normal_start..normal_start + face.num_edges as usize)
        .ok_or_else(|| format!("face {face_index} vertex-normal index range is out of bounds"))?;
    let mut normals = indices
        .iter()
        .map(|normal_index| {
            vertex_normals
                .get(*normal_index as usize)
                .copied()
                .ok_or_else(|| {
                    format!("face {face_index} references missing vertex normal {normal_index}")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if face.side {
        for normal in &mut normals {
            *normal = [-normal[0], -normal[1], -normal[2]];
        }
    }
    Ok((normals, true))
}

fn face_triangle_indices(
    face: Face,
    primitives: &[BspPrimitive],
    primitive_vertices: &[[f32; 3]],
    primitive_indices: &[u16],
    vertex_count: usize,
    face_index: usize,
) -> Result<(Vec<[usize; 3]>, bool), String> {
    if face.num_primitives == 0 {
        return Ok((
            (1..vertex_count - 1)
                .map(|index| [0, index, index + 1])
                .collect(),
            false,
        ));
    }
    let first = face.first_primitive as usize;
    let face_primitives = primitives
        .get(first..first + face.num_primitives as usize)
        .ok_or_else(|| format!("face {face_index} primitive range is out of bounds"))?;
    let mut triangles = Vec::new();
    for (primitive_offset, primitive) in face_primitives.iter().enumerate() {
        let primitive_index = first + primitive_offset;
        if primitive.vertex_count != 0 {
            let first_vertex = primitive.first_vertex as usize;
            if first_vertex + primitive.vertex_count as usize > primitive_vertices.len() {
                return Err(format!(
                    "primitive {primitive_index} PRIMVERTS range is out of bounds"
                ));
            }
            return Err(format!(
                "unsupported PRIMVERTS geometry on face {face_index}, primitive {primitive_index}; export aborted"
            ));
        }
        let first_index = primitive.first_index as usize;
        let indices = primitive_indices
            .get(first_index..first_index + primitive.index_count as usize)
            .ok_or_else(|| format!("primitive {primitive_index} index range is out of bounds"))?;
        let mut push_triangle = |triangle: [u16; 3]| -> Result<(), String> {
            let triangle = triangle.map(usize::from);
            if triangle.iter().any(|index| *index >= vertex_count) {
                return Err(format!(
                    "primitive {primitive_index} on face {face_index} references a vertex outside the face"
                ));
            }
            if triangle[0] != triangle[1]
                && triangle[1] != triangle[2]
                && triangle[0] != triangle[2]
            {
                triangles.push(triangle);
            }
            Ok(())
        };
        match primitive.primitive_type {
            0 => {
                if !indices.len().is_multiple_of(3) {
                    return Err(format!(
                        "triangle-list primitive {primitive_index} has a non-triangular index count"
                    ));
                }
                for triangle in indices.chunks_exact(3) {
                    push_triangle([triangle[0], triangle[1], triangle[2]])?;
                }
            }
            1 => {
                for index in 0..indices.len().saturating_sub(2) {
                    let triangle = if index.is_multiple_of(2) {
                        [indices[index], indices[index + 1], indices[index + 2]]
                    } else {
                        [indices[index + 1], indices[index], indices[index + 2]]
                    };
                    push_triangle(triangle)?;
                }
            }
            primitive_type => {
                return Err(format!(
                    "primitive {primitive_index} has unsupported type {primitive_type}"
                ));
            }
        }
    }
    if triangles.is_empty() {
        return Err(format!(
            "face {face_index} compiled primitives contain no triangles"
        ));
    }
    Ok((triangles, true))
}

fn entity_property<'a>(entity: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    entity
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn parse_source_vector(value: Option<&str>, fallback: [f32; 3]) -> [f32; 3] {
    let Some(value) = value else {
        return fallback;
    };
    let parsed = value
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>();
    match parsed {
        Ok(values) if values.len() == 3 => [values[0], values[1], values[2]],
        _ => fallback,
    }
}

fn source_entity_matrix(origin: [f32; 3], angles: [f32; 3]) -> [f32; 16] {
    let [pitch, yaw, roll] = angles.map(f32::to_radians);
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let (sr, cr) = roll.sin_cos();
    let source_rotation = [
        [cp * cy, sr * sp * cy - cr * sy, cr * sp * cy + sr * sy],
        [cp * sy, sr * sp * sy + cr * cy, cr * sp * sy - sr * cy],
        [-sp, sr * cp, cr * cp],
    ];
    let source_to_gltf_basis = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]];
    let gltf_to_source = [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]];
    let mut intermediate = [[0.0; 3]; 3];
    let mut gltf_rotation = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            for axis in 0..3 {
                intermediate[row][column] +=
                    source_rotation[row][axis] * gltf_to_source[axis][column];
            }
        }
    }
    for row in 0..3 {
        for column in 0..3 {
            for axis in 0..3 {
                gltf_rotation[row][column] +=
                    source_to_gltf_basis[row][axis] * intermediate[axis][column];
            }
        }
    }
    let translation = source_to_gltf(origin);
    [
        gltf_rotation[0][0],
        gltf_rotation[1][0],
        gltf_rotation[2][0],
        0.0,
        gltf_rotation[0][1],
        gltf_rotation[1][1],
        gltf_rotation[2][1],
        0.0,
        gltf_rotation[0][2],
        gltf_rotation[1][2],
        gltf_rotation[2][2],
        0.0,
        translation[0],
        translation[1],
        translation[2],
        1.0,
    ]
}

fn entity_initially_rendered(entity: Option<&HashMap<String, String>>, classname: &str) -> bool {
    let Some(entity) = entity else {
        return true;
    };
    let classname = classname.to_ascii_lowercase();
    let render_mode = entity_property(entity, "rendermode")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    let start_disabled = entity_property(entity, "StartDisabled")
        .map(|value| value == "1")
        .unwrap_or(false);
    !classname.starts_with("trigger_")
        && classname != "func_occluder"
        && render_mode != 6
        && render_mode != 10
        && !(classname == "func_brush" && start_disabled)
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn pad4(data: &mut Vec<u8>, byte: u8) {
    while !data.len().is_multiple_of(4) {
        data.push(byte);
    }
}

fn append_f32(binary: &mut Vec<u8>, values: &[f32]) -> (usize, usize) {
    pad4(binary, 0);
    let offset = binary.len();
    for value in values {
        binary.extend_from_slice(&value.to_le_bytes());
    }
    (offset, binary.len() - offset)
}

fn append_u32(binary: &mut Vec<u8>, values: &[u32]) -> (usize, usize) {
    pad4(binary, 0);
    let offset = binary.len();
    for value in values {
        binary.extend_from_slice(&value.to_le_bytes());
    }
    (offset, binary.len() - offset)
}

fn min_max(values: &[f32], width: usize) -> (Vec<f32>, Vec<f32>) {
    let mut min = vec![f32::INFINITY; width];
    let mut max = vec![f32::NEG_INFINITY; width];
    for item in values.chunks_exact(width) {
        for axis in 0..width {
            min[axis] = min[axis].min(item[axis]);
            max[axis] = max[axis].max(item[axis]);
        }
    }
    (min, max)
}

struct GlbArrays {
    binary: Vec<u8>,
    buffer_views: Vec<Value>,
    accessors: Vec<Value>,
}

impl GlbArrays {
    fn add_f32(&mut self, values: &[f32], width: usize, target: u32) -> usize {
        let (offset, length) = append_f32(&mut self.binary, values);
        let view = self.buffer_views.len();
        self.buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": length,
            "target": target
        }));
        let (min, max) = min_max(values, width);
        let accessor = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": 5126,
            "count": values.len() / width,
            "type": if width == 3 { "VEC3" } else { "VEC2" },
            "min": min,
            "max": max
        }));
        accessor
    }

    fn add_indices(&mut self, values: &[u32]) -> usize {
        let (offset, length) = append_u32(&mut self.binary, values);
        let view = self.buffer_views.len();
        self.buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": length,
            "target": 34963
        }));
        let accessor = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": 5125,
            "count": values.len(),
            "type": "SCALAR",
            "min": [values.iter().copied().min().unwrap_or(0)],
            "max": [values.iter().copied().max().unwrap_or(0)]
        }));
        accessor
    }
}

fn encode_glb(mut document: Value, mut binary: Vec<u8>) -> Result<Vec<u8>, String> {
    pad4(&mut binary, 0);
    document["buffers"] = json!([{ "byteLength": binary.len() }]);
    let mut json_bytes = serde_json::to_vec(&document)
        .map_err(|error| format!("failed to serialize GLTF JSON: {error}"))?;
    pad4(&mut json_bytes, b' ');
    let total_length = 12 + 8 + json_bytes.len() + 8 + binary.len();
    let total_u32 = u32::try_from(total_length).map_err(|_| "GLB exceeds 4 GiB".to_owned())?;
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(&0x4654_6c67_u32.to_le_bytes());
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&total_u32.to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004e_4942_u32.to_le_bytes());
    glb.extend_from_slice(&binary);
    Ok(glb)
}

pub fn export_bsp(data: &[u8], lightmap_json: Option<&[u8]>) -> Result<ExportResult, String> {
    let bsp = parse_bsp(data)?;
    let planes = parse_planes(&bsp.lumps[LUMP_PLANES])?;
    let vertices = parse_vec3_lump(&bsp.lumps[LUMP_VERTEXES], "vertex")?;
    let edges = parse_edges(&bsp.lumps[LUMP_EDGES])?;
    let surfedges = parse_i32_lump(&bsp.lumps[LUMP_SURFEDGES], "surfedge")?;
    let vertex_normals = parse_vec3_lump(&bsp.lumps[LUMP_VERTNORMALS], "vertex normal")?;
    let vertex_normal_indices =
        parse_u16_lump(&bsp.lumps[LUMP_VERTNORMALINDICES], "vertex-normal index")?;
    let primitives = parse_primitives(&bsp.lumps[LUMP_PRIMITIVES])?;
    let primitive_vertices = parse_vec3_lump(&bsp.lumps[LUMP_PRIMVERTS], "primitive vertex")?;
    let primitive_indices = parse_u16_lump(&bsp.lumps[LUMP_PRIMINDICES], "primitive index")?;
    let face_lump = if bsp.lumps[LUMP_FACES].is_empty() {
        &bsp.lumps[LUMP_FACES_HDR]
    } else {
        &bsp.lumps[LUMP_FACES]
    };
    let faces = parse_faces(face_lump)?;
    let mut normal_index_cursor = 0;
    let face_normal_starts: Vec<_> = faces
        .iter()
        .map(|face| {
            let start = normal_index_cursor;
            normal_index_cursor += face.num_edges.max(0) as usize;
            start
        })
        .collect();
    let texinfos = parse_texinfo(&bsp.lumps[LUMP_TEXINFO])?;
    let texdata = parse_texdata(&bsp.lumps[LUMP_TEXDATA])?;
    let material_names = parse_material_names(
        &texdata,
        &bsp.lumps[LUMP_TEXDATA_STRING_DATA],
        &bsp.lumps[LUMP_TEXDATA_STRING_TABLE],
    )?;
    let models = parse_models(&bsp.lumps[LUMP_MODELS])?;
    let entities = parse_entities(&bsp.lumps[LUMP_ENTITIES])?;
    let lightmaps = lightmap_json.map(LightmapLookup::parse).transpose()?;

    if models.is_empty() {
        return Err("BSP contains no brush models".to_owned());
    }

    let mut face_owner = vec![None; faces.len()];
    for (model_index, model) in models.iter().enumerate() {
        if model.first_face < 0 || model.num_faces < 0 {
            return Err(format!("model {model_index} has a negative face range"));
        }
        let start = model.first_face as usize;
        let end = start
            .checked_add(model.num_faces as usize)
            .ok_or_else(|| format!("model {model_index} face range overflows"))?;
        if end > faces.len() {
            return Err(format!("model {model_index} face range is out of bounds"));
        }
        for owner in &mut face_owner[start..end] {
            if owner.replace(model_index).is_some() {
                return Err(format!(
                    "model {model_index} overlaps another model's face range"
                ));
            }
        }
    }

    let displacements: Vec<_> = faces
        .iter()
        .enumerate()
        .filter_map(|(index, face)| {
            (face_owner[index].is_some() && face.dispinfo >= 0).then_some(index)
        })
        .collect();
    if !displacements.is_empty() {
        let samples = displacements
            .iter()
            .take(8)
            .map(|index| format!("face {index}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "unsupported displacement geometry detected on {} face(s): {samples}; export aborted without dropping geometry",
            displacements.len()
        ));
    }

    let mut entity_by_model: HashMap<usize, (usize, &HashMap<String, String>)> = HashMap::new();
    for (entity_index, entity) in entities.iter().enumerate() {
        let Some(model_value) = entity
            .get("model")
            .and_then(|value| value.strip_prefix('*'))
        else {
            continue;
        };
        if let Ok(model_index) = model_value.parse::<usize>() {
            entity_by_model
                .entry(model_index)
                .or_insert((entity_index, entity));
        }
    }
    if let Some((entity_index, worldspawn)) = entities
        .iter()
        .enumerate()
        .find(|(_, entity)| entity.get("classname").map(String::as_str) == Some("worldspawn"))
    {
        entity_by_model.insert(0, (entity_index, worldspawn));
    }

    let materials: Vec<_> = material_names
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "doubleSided": false,
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 0.0,
                    "roughnessFactor": 1.0
                },
                "extras": { "sourceMaterial": name }
            })
        })
        .collect();
    let mut arrays = GlbArrays {
        binary: Vec::new(),
        buffer_views: Vec::new(),
        accessors: Vec::new(),
    };
    let mut meshes_json = Vec::new();
    let mut nodes_json = Vec::new();
    let mut stats = ExportStats {
        models: models.len(),
        materials: material_names.len(),
        displacement_faces: displacements.len(),
        ..ExportStats::default()
    };

    for (model_index, model) in models.iter().enumerate() {
        let (entity_index, entity) = entity_by_model
            .get(&model_index)
            .map(|(index, entity)| (Some(*index), Some(*entity)))
            .unwrap_or((None, None));
        let classname = entity
            .and_then(|item| entity_property(item, "classname"))
            .unwrap_or(if model_index == 0 {
                "worldspawn"
            } else {
                "brush_model"
            });
        let targetname = entity.and_then(|item| entity_property(item, "targetname"));
        let entity_rendered = entity_initially_rendered(entity, classname);
        let origin = parse_source_vector(
            entity.and_then(|item| entity_property(item, "origin")),
            if model_index == 0 {
                [0.0; 3]
            } else {
                model.origin
            },
        );
        let angles = parse_source_vector(
            entity.and_then(|item| entity_property(item, "angles")),
            [0.0; 3],
        );
        let mut groups: BTreeMap<(usize, bool, bool, bool, i32), PrimitiveData> = BTreeMap::new();
        let start = model.first_face as usize;
        let end = start + model.num_faces as usize;
        for (face_index, face) in faces.iter().copied().enumerate().take(end).skip(start) {
            let texinfo_index = usize::try_from(face.texinfo)
                .map_err(|_| format!("face {face_index} has no texinfo"))?;
            let texinfo = texinfos.get(texinfo_index).ok_or_else(|| {
                format!("face {face_index} references missing texinfo {texinfo_index}")
            })?;
            let material_index = usize::try_from(texinfo.texdata)
                .map_err(|_| format!("face {face_index} has a negative texdata index"))?;
            let texture = texdata.get(material_index).ok_or_else(|| {
                format!("face {face_index} references missing texdata {material_index}")
            })?;
            if texture.width <= 0 || texture.height <= 0 {
                return Err(format!("face {face_index} has invalid texture dimensions"));
            }
            let source_positions = face_positions(face, &surfedges, &edges, &vertices, face_index)?;
            let compiled_lightmap = face.light_offset >= 0
                && face.styles[0] != 255
                && face.lightmap_size[0] >= 0
                && face.lightmap_size[1] >= 0
                && texinfo.flags & (SURF_SKY | SURF_NODRAW | SURF_NOLIGHT) == 0;
            let mapping = compiled_lightmap
                .then(|| {
                    lightmaps
                        .as_ref()
                        .and_then(|lookup| lookup.find(face_index, &source_positions, texinfo))
                })
                .flatten();
            let has_lightmap = mapping.is_some();
            let surface_rendered = texinfo.flags & NON_RENDERED_SURFACE_FLAGS == 0;
            let (mut triangles, compiled_triangulation) = face_triangle_indices(
                face,
                &primitives,
                &primitive_vertices,
                &primitive_indices,
                source_positions.len(),
                face_index,
            )?;
            let primitive = groups
                .entry((
                    material_index,
                    has_lightmap,
                    surface_rendered,
                    compiled_triangulation,
                    texinfo.flags,
                ))
                .or_default();
            let base_vertex = (primitive.positions.len() / 3) as u32;
            let plane_normal = planes
                .get(face.plane)
                .ok_or_else(|| {
                    format!("face {face_index} references missing plane {}", face.plane)
                })?
                .normal;
            let outward_normal = if face.side {
                [-plane_normal[0], -plane_normal[1], -plane_normal[2]]
            } else {
                plane_normal
            };
            let (source_normals, has_compiled_normals) = face_normals(
                face,
                face_normal_starts[face_index],
                &vertex_normal_indices,
                &vertex_normals,
                outward_normal,
                face_index,
            )?;
            let summed_normal = source_normals.iter().fold([0.0; 3], |sum, normal| {
                [sum[0] + normal[0], sum[1] + normal[1], sum[2] + normal[2]]
            });
            let winding_normal = if dot(summed_normal, summed_normal) > 1e-12 {
                source_to_gltf(summed_normal)
            } else {
                source_to_gltf(outward_normal)
            };
            let gltf_positions: Vec<_> = source_positions
                .iter()
                .copied()
                .map(source_to_gltf)
                .collect();
            for ((source, gltf), source_normal) in source_positions
                .iter()
                .zip(&gltf_positions)
                .zip(&source_normals)
            {
                primitive.positions.extend_from_slice(gltf);
                primitive
                    .normals
                    .extend_from_slice(&source_to_gltf(*source_normal));
                primitive
                    .uv0
                    .push(dot4(texinfo.texture_vecs[0], *source) / texture.width as f32);
                primitive
                    .uv0
                    .push(dot4(texinfo.texture_vecs[1], *source) / texture.height as f32);
                if let (Some(lookup), Some(mapping_index)) = (&lightmaps, mapping) {
                    primitive
                        .uv1
                        .extend_from_slice(&lookup.uv(mapping_index, *source));
                }
            }
            for triangle in &mut triangles {
                let a = gltf_positions[triangle[0]];
                let b = gltf_positions[triangle[1]];
                let c = gltf_positions[triangle[2]];
                if dot(cross(sub(b, a), sub(c, a)), winding_normal) < 0.0 {
                    triangle.swap(1, 2);
                }
                primitive
                    .indices
                    .extend(triangle.map(|index| base_vertex + index as u32));
                primitive.triangles += 1;
            }
            primitive.face_indices.push(face_index);
            primitive.face_vertex_counts.push(source_positions.len());
            primitive.face_styles.push(face.styles);
            primitive.face_light_offsets.push(face.light_offset);
            primitive.face_lightmap_mins.push(face.lightmap_mins);
            primitive.face_lightmap_sizes.push(face.lightmap_size);
            stats.faces += 1;
            stats.vertices += source_positions.len();
            stats.triangles += triangles.len();
            if has_lightmap {
                stats.lightmapped_faces += 1;
            }
            if compiled_triangulation {
                stats.compiled_primitive_faces += 1;
            } else {
                stats.fan_faces += 1;
            }
            if has_compiled_normals {
                stats.compiled_normal_vertices += source_positions.len();
                stats.compiled_normal_opposed_vertices += source_normals
                    .iter()
                    .filter(|normal| dot(**normal, outward_normal) < -1e-4)
                    .count();
            }
            if entity_rendered && surface_rendered {
                stats.initially_rendered_faces += 1;
            }
        }

        let mut primitives_json = Vec::new();
        for (
            (material_index, has_lightmap, surface_rendered, compiled_triangulation, surface_flags),
            primitive,
        ) in groups
        {
            if primitive.indices.is_empty() {
                continue;
            }
            let position = arrays.add_f32(&primitive.positions, 3, 34962);
            let normal = arrays.add_f32(&primitive.normals, 3, 34962);
            let uv0 = arrays.add_f32(&primitive.uv0, 2, 34962);
            let mut attributes = serde_json::Map::new();
            attributes.insert("POSITION".to_owned(), json!(position));
            attributes.insert("NORMAL".to_owned(), json!(normal));
            attributes.insert("TEXCOORD_0".to_owned(), json!(uv0));
            if has_lightmap {
                let uv1 = arrays.add_f32(&primitive.uv1, 2, 34962);
                attributes.insert("TEXCOORD_1".to_owned(), json!(uv1));
            }
            let indices = arrays.add_indices(&primitive.indices);
            primitives_json.push(json!({
                "attributes": attributes,
                "indices": indices,
                "material": material_index,
                "mode": 4,
                "extras": {
                    "bspModelIndex": model_index,
                    "bspFaceIndices": primitive.face_indices,
                    "bspFaceVertexCounts": primitive.face_vertex_counts,
                    "bspFaceStyles": primitive.face_styles,
                    "bspFaceLightOffsets": primitive.face_light_offsets,
                    "bspFaceLightmapMins": primitive.face_lightmap_mins,
                    "bspFaceLightmapSizes": primitive.face_lightmap_sizes,
                    "bspTriangleCount": primitive.triangles,
                    "hasLightmap": has_lightmap,
                    "surfaceFlags": surface_flags,
                    "surfaceInitiallyRendered": surface_rendered,
                    "initiallyRendered": entity_rendered && surface_rendered,
                    "triangulation": if compiled_triangulation { "compiled" } else { "fan" }
                }
            }));
            stats.primitives += 1;
        }

        let mesh_index = meshes_json.len();
        let node_name = if model_index == 0 {
            "worldspawn".to_owned()
        } else if let Some(target) = targetname {
            format!("model_{model_index}_{classname}_{target}")
        } else {
            format!("model_{model_index}_{classname}")
        };
        meshes_json.push(json!({
            "name": node_name,
            "primitives": primitives_json,
            "extras": {
                "bspModelIndex": model_index,
                "firstFace": model.first_face,
                "numFaces": model.num_faces
            }
        }));
        nodes_json.push(json!({
            "name": node_name,
            "mesh": mesh_index,
            "matrix": source_entity_matrix(origin, angles),
            "extras": {
                "bspModelIndex": model_index,
                "entityIndex": entity_index,
                "classname": classname,
                "targetname": targetname,
                "model": entity.and_then(|item| entity_property(item, "model")),
                "startDisabled": entity.and_then(|item| entity_property(item, "StartDisabled")),
                "solid": entity.and_then(|item| entity_property(item, "solid")),
                "rendermode": entity.and_then(|item| entity_property(item, "rendermode")),
                "initiallyRendered": entity_rendered,
                "sourceOrigin": model.origin,
                "entityOrigin": origin,
                "entityAngles": angles,
                "sourceMins": model.mins,
                "sourceMaxs": model.maxs
            }
        }));
        stats.meshes += 1;
    }

    let document = json!({
        "asset": {
            "version": "2.0",
            "generator": "bsp-to-glb 0.1.0",
            "extras": {
                "source": "compiled Valve BSP",
                "bspVersion": bsp.version,
                "coordinateTransform": "Source XYZ to glTF X,Z,-Y"
            }
        },
        "scene": 0,
        "scenes": [{ "name": "BSP models", "nodes": (0..nodes_json.len()).collect::<Vec<_>>() }],
        "nodes": nodes_json,
        "meshes": meshes_json,
        "materials": materials,
        "bufferViews": arrays.buffer_views,
        "accessors": arrays.accessors
    });
    let glb = encode_glb(document, arrays.binary)?;
    Ok(ExportResult { glb, stats })
}
