use super::{Entity, EntityProperty};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENTITY_GRAPH_VERSION: u32 = 1;
pub const MAX_ENTITY_LUMP_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ENTITIES: usize = 16_384;
pub const MAX_ENTITY_KEY_VALUES: usize = 262_144;
pub const MAX_ENTITY_KEY_VALUES_PER_ENTITY: usize = 4_096;
pub const MAX_ENTITY_STRING_BYTES: usize = 16_384;
pub const MAX_ENTITY_CONNECTIONS: usize = 262_144;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityKeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityConnectionError {
    FieldCount,
    EmptyTarget,
    EmptyInput,
    InvalidDelay,
    InvalidMaxFires,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EntityConnection {
    Parsed {
        order: usize,
        output_name: String,
        target: String,
        input: String,
        parameter: String,
        delay: f32,
        max_fires: i32,
    },
    Malformed {
        order: usize,
        output_name: String,
        error: EntityConnectionError,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledEntity {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bsp_model_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targetname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parentname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spawnflags: Option<String>,
    pub key_values: Vec<EntityKeyValue>,
    pub connections: Vec<EntityConnection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityGraphInventory {
    pub entity_count: usize,
    pub key_value_count: usize,
    pub connection_count: usize,
    pub malformed_connection_count: usize,
    pub entities_without_classname: usize,
    pub class_counts: BTreeMap<String, usize>,
    pub output_counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityGraph {
    pub schema: String,
    pub schema_version: u32,
    pub source_bsp_version: i32,
    pub inventory: EntityGraphInventory,
    pub entities: Vec<CompiledEntity>,
}

impl EntityGraph {
    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self)
            .map_err(|error| format!("failed to serialize entity graph sidecar: {error}"))
    }
}

fn property<'a>(entity: &'a Entity, name: &str) -> Option<&'a str> {
    entity
        .iter()
        .find(|property| property.key.eq_ignore_ascii_case(name))
        .map(|property| property.value.as_str())
}

fn conventional_output_name(key: &str) -> bool {
    key.starts_with("On") || key.starts_with("Out")
}

fn parse_connection(order: usize, property: &EntityProperty) -> Option<EntityConnection> {
    let delimiter = if property.value.contains('\u{1b}') {
        '\u{1b}'
    } else {
        ','
    };
    let fields: Vec<_> = property.value.split(delimiter).collect();
    let has_connection_shape = fields.len() == 5
        && fields[3].trim().parse::<f32>().is_ok()
        && fields[4].trim().parse::<i32>().is_ok();
    if delimiter != '\u{1b}' && !conventional_output_name(&property.key) && !has_connection_shape {
        return None;
    }

    let malformed = |error| EntityConnection::Malformed {
        order,
        output_name: property.key.clone(),
        error,
    };
    if fields.len() != 5 {
        return Some(malformed(EntityConnectionError::FieldCount));
    }
    if fields[0].is_empty() {
        return Some(malformed(EntityConnectionError::EmptyTarget));
    }
    if fields[1].is_empty() {
        return Some(malformed(EntityConnectionError::EmptyInput));
    }
    let Ok(delay) = fields[3].trim().parse::<f32>() else {
        return Some(malformed(EntityConnectionError::InvalidDelay));
    };
    if !delay.is_finite() {
        return Some(malformed(EntityConnectionError::InvalidDelay));
    }
    let Ok(max_fires) = fields[4].trim().parse::<i32>() else {
        return Some(malformed(EntityConnectionError::InvalidMaxFires));
    };
    Some(EntityConnection::Parsed {
        order,
        output_name: property.key.clone(),
        target: fields[0].to_owned(),
        input: fields[1].to_owned(),
        parameter: fields[2].to_owned(),
        delay,
        max_fires,
    })
}

pub(crate) fn build_entity_graph(
    bsp_version: i32,
    source_entities: &[Entity],
) -> Result<EntityGraph, String> {
    let mut entities = Vec::with_capacity(source_entities.len());
    let mut key_value_count = 0;
    let mut connection_count = 0;
    let mut malformed_connection_count = 0;
    let mut entities_without_classname = 0;
    let mut class_counts = BTreeMap::new();
    let mut output_counts = BTreeMap::new();
    let mut total_connections = 0;

    for (index, entity) in source_entities.iter().enumerate() {
        key_value_count += entity.len();
        let classname = property(entity, "classname").map(str::to_owned);
        if let Some(classname) = &classname {
            *class_counts.entry(classname.clone()).or_insert(0) += 1;
        } else {
            entities_without_classname += 1;
        }
        let model = property(entity, "model").map(str::to_owned);
        let bsp_model_index = model
            .as_deref()
            .and_then(|value| value.strip_prefix('*'))
            .and_then(|value| value.parse().ok())
            .or_else(|| {
                classname
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case("worldspawn"))
                    .then_some(0)
            });
        let mut connections = Vec::new();
        for (order, property) in entity.iter().enumerate() {
            let Some(connection) = parse_connection(order, property) else {
                continue;
            };
            total_connections += 1;
            if total_connections > MAX_ENTITY_CONNECTIONS {
                return Err(format!(
                    "entity connection count exceeds {MAX_ENTITY_CONNECTIONS}"
                ));
            }
            *output_counts.entry(property.key.clone()).or_insert(0) += 1;
            match connection {
                EntityConnection::Parsed { .. } => connection_count += 1,
                EntityConnection::Malformed { .. } => malformed_connection_count += 1,
            }
            connections.push(connection);
        }
        entities.push(CompiledEntity {
            index,
            classname,
            model,
            bsp_model_index,
            targetname: property(entity, "targetname").map(str::to_owned),
            parentname: property(entity, "parentname").map(str::to_owned),
            spawnflags: property(entity, "spawnflags").map(str::to_owned),
            key_values: entity
                .iter()
                .map(|property| EntityKeyValue {
                    key: property.key.clone(),
                    value: property.value.clone(),
                })
                .collect(),
            connections,
        });
    }

    Ok(EntityGraph {
        schema: "bsp-to-glb.entity-graph".to_owned(),
        schema_version: ENTITY_GRAPH_VERSION,
        source_bsp_version: bsp_version,
        inventory: EntityGraphInventory {
            entity_count: entities.len(),
            key_value_count,
            connection_count,
            malformed_connection_count,
            entities_without_classname,
            class_counts,
            output_counts,
        },
        entities,
    })
}
