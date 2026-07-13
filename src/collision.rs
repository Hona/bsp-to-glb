use super::{
    Bsp, Entity, Model, entity_property, parse_bsp, parse_entities, parse_models, parse_planes,
    parse_source_vector, read_i16, read_i32, read_u16,
};
use base64::Engine;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};

const LUMP_NODES: usize = 5;
const LUMP_LEAFS: usize = 10;
const LUMP_MODELS: usize = 14;
const LUMP_LEAFBRUSHES: usize = 17;
const LUMP_BRUSHES: usize = 18;
const LUMP_BRUSHSIDES: usize = 19;
const LUMP_PHYSCOLLIDE: usize = 29;
const LUMP_ENTITIES: usize = 0;

const NODE_SIZE: usize = 32;
const BRUSH_SIZE: usize = 12;
const BRUSH_SIDE_SIZE: usize = 8;
const PHYSICS_HEADER_SIZE: usize = 16;
const CONTENTS_PLAYERCLIP: u32 = 0x1_0000;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionStats {
    pub planes: usize,
    pub brushes: usize,
    pub brush_sides: usize,
    pub leaf_brushes: usize,
    pub leaves: usize,
    pub world_model_brushes: usize,
    pub player_clip_brushes: usize,
    pub models: usize,
    pub physics_models: usize,
}

#[derive(Debug)]
pub struct CollisionExportResult {
    pub json: Vec<u8>,
    pub stats: CollisionStats,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPropCollisionInput {
    pub prop_index: usize,
    pub model_name: String,
    pub solid_mode: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CollisionExportInput<'a> {
    /// Static-prop metadata supplied by a GAME_LUMP parser. `None` means that
    /// game-lump data was unavailable, which is distinct from an empty list.
    pub static_props: Option<&'a [StaticPropCollisionInput]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollisionPlane {
    normal: [f32; 3],
    distance: f32,
    plane_type: i32,
}

#[derive(Debug)]
struct Node {
    children: [i32; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Leaf {
    contents: u32,
    cluster: i16,
    area_and_flags: u16,
    first_leaf_brush: usize,
    num_leaf_brushes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Brush {
    first_side: usize,
    num_sides: usize,
    contents: u32,
    player_clip: bool,
    model_indices: Vec<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrushSide {
    plane_index: usize,
    tex_info: i16,
    displacement_info: i16,
    bevel: i16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollisionModel {
    model_index: usize,
    head_node: i32,
    first_render_face: i32,
    num_render_faces: i32,
    source_mins: [f32; 3],
    source_maxs: [f32; 3],
    source_origin: [f32; 3],
    brush_indices: Vec<usize>,
    entity_index: Option<usize>,
    classname: String,
    targetname: Option<String>,
    entity_origin: [f32; 3],
    entity_angles: [f32; 3],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicsBlock {
    model_index: i32,
    data_size: usize,
    key_data_size: usize,
    solid_count: i32,
    raw_block_base64: String,
    collision_data_base64: String,
    key_data_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicsCollision {
    decode_status: &'static str,
    raw_encoding: &'static str,
    raw_lump_base64: String,
    blocks: Vec<PhysicsBlock>,
    terminator_present: bool,
    trailing_data_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollisionSidecar<'a> {
    schema: &'static str,
    version: u32,
    bsp_version: i32,
    coordinate_system: &'static str,
    geometry_source: &'static str,
    render_triangle_substitution: bool,
    stats: &'a CollisionStats,
    planes: Vec<CollisionPlane>,
    brush_sides: Vec<BrushSide>,
    brushes: Vec<Brush>,
    leaf_brushes: Vec<u16>,
    leaves: Vec<Leaf>,
    models: Vec<CollisionModel>,
    physics_collision: PhysicsCollision,
    static_prop_input_available: bool,
    static_props: &'a [StaticPropCollisionInput],
}

fn parse_nodes(data: &[u8]) -> Result<Vec<Node>, String> {
    if !data.len().is_multiple_of(NODE_SIZE) {
        return Err(format!(
            "node lump length {} is not divisible by {NODE_SIZE}",
            data.len()
        ));
    }
    (0..data.len() / NODE_SIZE)
        .map(|index| {
            let offset = index * NODE_SIZE;
            Ok(Node {
                children: [
                    read_i32(data, offset + 4, "node")?,
                    read_i32(data, offset + 8, "node")?,
                ],
            })
        })
        .collect()
}

fn parse_leaves(bsp: &Bsp) -> Result<Vec<Leaf>, String> {
    let version = bsp.lump_versions[LUMP_LEAFS];
    let size = match version {
        0 => 56,
        1 => 32,
        _ => return Err(format!("unsupported leaf lump version {version}")),
    };
    let data = &bsp.lumps[LUMP_LEAFS];
    if !data.len().is_multiple_of(size) {
        return Err(format!(
            "leaf lump version {version} length {} is not divisible by {size}",
            data.len()
        ));
    }
    (0..data.len() / size)
        .map(|index| {
            let offset = index * size;
            Ok(Leaf {
                contents: read_i32(data, offset, "leaf")? as u32,
                cluster: read_i16(data, offset + 4, "leaf")?,
                area_and_flags: read_u16(data, offset + 6, "leaf")?,
                first_leaf_brush: read_u16(data, offset + 24, "leaf")? as usize,
                num_leaf_brushes: read_u16(data, offset + 26, "leaf")? as usize,
            })
        })
        .collect()
}

fn parse_leaf_brushes(data: &[u8]) -> Result<Vec<u16>, String> {
    if !data.len().is_multiple_of(2) {
        return Err("leafbrush lump length is not divisible by 2".to_owned());
    }
    (0..data.len() / 2)
        .map(|index| read_u16(data, index * 2, "leafbrush"))
        .collect()
}

fn parse_brushes(data: &[u8]) -> Result<Vec<Brush>, String> {
    if !data.len().is_multiple_of(BRUSH_SIZE) {
        return Err(format!(
            "brush lump length {} is not divisible by {BRUSH_SIZE}",
            data.len()
        ));
    }
    (0..data.len() / BRUSH_SIZE)
        .map(|index| {
            let offset = index * BRUSH_SIZE;
            let first_side = read_i32(data, offset, "brush")?;
            let num_sides = read_i32(data, offset + 4, "brush")?;
            if first_side < 0 || num_sides < 0 {
                return Err(format!("brush {index} has a negative side range"));
            }
            let contents = read_i32(data, offset + 8, "brush")? as u32;
            Ok(Brush {
                first_side: first_side as usize,
                num_sides: num_sides as usize,
                contents,
                player_clip: contents & CONTENTS_PLAYERCLIP != 0,
                model_indices: Vec::new(),
            })
        })
        .collect()
}

fn parse_brush_sides(data: &[u8]) -> Result<Vec<BrushSide>, String> {
    if !data.len().is_multiple_of(BRUSH_SIDE_SIZE) {
        return Err(format!(
            "brushside lump length {} is not divisible by {BRUSH_SIDE_SIZE}",
            data.len()
        ));
    }
    (0..data.len() / BRUSH_SIDE_SIZE)
        .map(|index| {
            let offset = index * BRUSH_SIDE_SIZE;
            Ok(BrushSide {
                plane_index: read_u16(data, offset, "brushside")? as usize,
                tex_info: read_i16(data, offset + 2, "brushside")?,
                displacement_info: read_i16(data, offset + 4, "brushside")?,
                bevel: read_i16(data, offset + 6, "brushside")?,
            })
        })
        .collect()
}

fn model_brushes(
    model_index: usize,
    head_node: i32,
    nodes: &[Node],
    leaves: &[Leaf],
    leaf_brushes: &[u16],
    brush_count: usize,
) -> Result<Vec<usize>, String> {
    let mut pending = vec![head_node];
    let mut visited_nodes = HashSet::new();
    let mut visited_leaves = HashSet::new();
    let mut brushes = BTreeSet::new();
    while let Some(reference) = pending.pop() {
        if reference >= 0 {
            let node_index = reference as usize;
            let node = nodes.get(node_index).ok_or_else(|| {
                format!("model {model_index} references missing head node {node_index}")
            })?;
            if visited_nodes.insert(node_index) {
                pending.extend(node.children);
            }
            continue;
        }
        let leaf_index = reference
            .checked_add(1)
            .and_then(i32::checked_neg)
            .map(|value| value as usize)
            .ok_or_else(|| format!("model {model_index} has an invalid leaf reference"))?;
        if !visited_leaves.insert(leaf_index) {
            continue;
        }
        let leaf = leaves
            .get(leaf_index)
            .ok_or_else(|| format!("model {model_index} references missing leaf {leaf_index}"))?;
        let end = leaf
            .first_leaf_brush
            .checked_add(leaf.num_leaf_brushes)
            .ok_or_else(|| format!("leaf {leaf_index} leafbrush range overflows"))?;
        let references = leaf_brushes
            .get(leaf.first_leaf_brush..end)
            .ok_or_else(|| format!("leaf {leaf_index} leafbrush range is out of bounds"))?;
        for brush in references {
            let brush = *brush as usize;
            if brush >= brush_count {
                return Err(format!(
                    "leaf {leaf_index} references missing brush {brush}"
                ));
            }
            brushes.insert(brush);
        }
    }
    Ok(brushes.into_iter().collect())
}

fn encode_base64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn parse_physics(data: &[u8]) -> Result<PhysicsCollision, String> {
    let mut cursor = 0;
    let mut blocks = Vec::new();
    let mut terminator_present = false;
    while cursor < data.len() {
        let header_end = cursor
            .checked_add(PHYSICS_HEADER_SIZE)
            .ok_or_else(|| "PHYSCOLLIDE header range overflows".to_owned())?;
        if header_end > data.len() {
            return Err(format!(
                "PHYSCOLLIDE has {} trailing byte(s), shorter than a model header",
                data.len() - cursor
            ));
        }
        let model_index = read_i32(data, cursor, "PHYSCOLLIDE model header")?;
        let data_size = read_i32(data, cursor + 4, "PHYSCOLLIDE model header")?;
        let key_data_size = read_i32(data, cursor + 8, "PHYSCOLLIDE model header")?;
        let solid_count = read_i32(data, cursor + 12, "PHYSCOLLIDE model header")?;
        if model_index == -1 {
            terminator_present = true;
            cursor = header_end;
            break;
        }
        if model_index < 0 || data_size < 0 || key_data_size < 0 || solid_count < 0 {
            return Err(format!(
                "PHYSCOLLIDE model header at byte {cursor} has negative metadata"
            ));
        }
        let data_size = data_size as usize;
        let key_data_size = key_data_size as usize;
        let collision_end = header_end
            .checked_add(data_size)
            .ok_or_else(|| "PHYSCOLLIDE collision range overflows".to_owned())?;
        let block_end = collision_end
            .checked_add(key_data_size)
            .ok_or_else(|| "PHYSCOLLIDE keydata range overflows".to_owned())?;
        if block_end > data.len() {
            return Err(format!(
                "PHYSCOLLIDE model {model_index} payload extends past the lump"
            ));
        }
        blocks.push(PhysicsBlock {
            model_index,
            data_size,
            key_data_size,
            solid_count,
            raw_block_base64: encode_base64(&data[cursor..block_end]),
            collision_data_base64: encode_base64(&data[header_end..collision_end]),
            key_data_base64: encode_base64(&data[collision_end..block_end]),
        });
        cursor = block_end;
    }
    Ok(PhysicsCollision {
        decode_status: "unsupported",
        raw_encoding: "base64",
        raw_lump_base64: encode_base64(data),
        blocks,
        terminator_present,
        trailing_data_base64: encode_base64(&data[cursor..]),
    })
}

fn entity_by_model(entities: &[Entity]) -> HashMap<usize, (usize, &Entity)> {
    let mut output = HashMap::new();
    for (entity_index, entity) in entities.iter().enumerate() {
        let Some(model_value) =
            entity_property(entity, "model").and_then(|value| value.strip_prefix('*'))
        else {
            continue;
        };
        if let Ok(model_index) = model_value.parse::<usize>() {
            output.entry(model_index).or_insert((entity_index, entity));
        }
    }
    if let Some((entity_index, worldspawn)) = entities.iter().enumerate().find(|(_, entity)| {
        entity_property(entity, "classname")
            .is_some_and(|classname| classname.eq_ignore_ascii_case("worldspawn"))
    }) {
        output.insert(0, (entity_index, worldspawn));
    }
    output
}

fn collision_models(
    models: &[Model],
    entities: &[Entity],
    model_brush_indices: Vec<Vec<usize>>,
) -> Vec<CollisionModel> {
    let entities = entity_by_model(entities);
    models
        .iter()
        .zip(model_brush_indices)
        .enumerate()
        .map(|(model_index, (model, brush_indices))| {
            let (entity_index, entity) = entities
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
            let entity_origin = parse_source_vector(
                entity.and_then(|item| entity_property(item, "origin")),
                if model_index == 0 {
                    [0.0; 3]
                } else {
                    model.origin
                },
            );
            CollisionModel {
                model_index,
                head_node: model.head_node,
                first_render_face: model.first_face,
                num_render_faces: model.num_faces,
                source_mins: model.mins,
                source_maxs: model.maxs,
                source_origin: model.origin,
                brush_indices,
                entity_index,
                classname: classname.to_owned(),
                targetname: entity
                    .and_then(|item| entity_property(item, "targetname"))
                    .map(str::to_owned),
                entity_origin,
                entity_angles: parse_source_vector(
                    entity.and_then(|item| entity_property(item, "angles")),
                    [0.0; 3],
                ),
            }
        })
        .collect()
}

pub fn export_collision_sidecar(
    data: &[u8],
    input: &CollisionExportInput<'_>,
) -> Result<CollisionExportResult, String> {
    let bsp = parse_bsp(data)?;
    let source_planes = parse_planes(&bsp.lumps[super::LUMP_PLANES])?;
    let planes: Vec<_> = source_planes
        .iter()
        .map(|plane| CollisionPlane {
            normal: plane.normal,
            distance: plane.distance,
            plane_type: plane.plane_type,
        })
        .collect();
    let nodes = parse_nodes(&bsp.lumps[LUMP_NODES])?;
    let leaves = parse_leaves(&bsp)?;
    let leaf_brushes = parse_leaf_brushes(&bsp.lumps[LUMP_LEAFBRUSHES])?;
    let mut brushes = parse_brushes(&bsp.lumps[LUMP_BRUSHES])?;
    let brush_sides = parse_brush_sides(&bsp.lumps[LUMP_BRUSHSIDES])?;
    let models = parse_models(&bsp.lumps[LUMP_MODELS])?;
    let entities = parse_entities(&bsp.lumps[LUMP_ENTITIES])?;

    for (brush_index, brush) in brushes.iter().enumerate() {
        let end = brush
            .first_side
            .checked_add(brush.num_sides)
            .ok_or_else(|| format!("brush {brush_index} side range overflows"))?;
        let sides = brush_sides
            .get(brush.first_side..end)
            .ok_or_else(|| format!("brush {brush_index} side range is out of bounds"))?;
        for side in sides {
            if side.plane_index >= planes.len() {
                return Err(format!(
                    "brush {brush_index} references missing plane {}",
                    side.plane_index
                ));
            }
        }
    }
    for (leaf_index, leaf) in leaves.iter().enumerate() {
        let end = leaf
            .first_leaf_brush
            .checked_add(leaf.num_leaf_brushes)
            .ok_or_else(|| format!("leaf {leaf_index} leafbrush range overflows"))?;
        let references = leaf_brushes
            .get(leaf.first_leaf_brush..end)
            .ok_or_else(|| format!("leaf {leaf_index} leafbrush range is out of bounds"))?;
        for brush in references {
            if *brush as usize >= brushes.len() {
                return Err(format!(
                    "leaf {leaf_index} references missing brush {brush}"
                ));
            }
        }
    }

    let mut model_brush_indices = Vec::with_capacity(models.len());
    for (model_index, model) in models.iter().enumerate() {
        let owned = model_brushes(
            model_index,
            model.head_node,
            &nodes,
            &leaves,
            &leaf_brushes,
            brushes.len(),
        )?;
        for brush_index in &owned {
            brushes[*brush_index].model_indices.push(model_index);
        }
        model_brush_indices.push(owned);
    }
    let world_model_brushes = model_brush_indices.first().map_or(0, Vec::len);
    let collision_models = collision_models(&models, &entities, model_brush_indices);
    let physics_collision = parse_physics(&bsp.lumps[LUMP_PHYSCOLLIDE])?;
    let static_props = input.static_props.unwrap_or(&[]);
    let stats = CollisionStats {
        planes: planes.len(),
        brushes: brushes.len(),
        brush_sides: brush_sides.len(),
        leaf_brushes: leaf_brushes.len(),
        leaves: leaves.len(),
        world_model_brushes,
        player_clip_brushes: brushes.iter().filter(|brush| brush.player_clip).count(),
        models: collision_models.len(),
        physics_models: physics_collision.blocks.len(),
    };
    let sidecar = CollisionSidecar {
        schema: "bsp-to-glb/collision",
        version: 1,
        bsp_version: bsp.version,
        coordinate_system: "Source XYZ",
        geometry_source: "bspBrushes",
        render_triangle_substitution: false,
        stats: &stats,
        planes,
        brush_sides,
        brushes,
        leaf_brushes,
        leaves,
        models: collision_models,
        physics_collision,
        static_prop_input_available: input.static_props.is_some(),
        static_props,
    };
    let json = serde_json::to_vec(&sidecar)
        .map_err(|error| format!("failed to serialize collision sidecar: {error}"))?;
    Ok(CollisionExportResult { json, stats })
}
