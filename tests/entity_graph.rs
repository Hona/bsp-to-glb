use bsp_to_glb::{MAX_ENTITY_KEY_VALUES_PER_ENTITY, export_entity_graph};
use serde_json::{Value, json};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_SIZE: usize = 4 + 4 + 64 * 16 + 4;

fn put_i32(data: &mut [u8], offset: usize, value: i32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn bsp_with_entities(entities: &[u8]) -> Vec<u8> {
    let mut bsp = vec![0; HEADER_SIZE];
    bsp[0..4].copy_from_slice(b"VBSP");
    put_i32(&mut bsp, 4, 20);
    put_i32(&mut bsp, 8, HEADER_SIZE as i32);
    put_i32(&mut bsp, 12, entities.len() as i32);
    bsp.extend_from_slice(entities);
    bsp
}

fn representative_entities() -> Vec<u8> {
    b"\
{\n\
\"classname\" \"worldspawn\"\n\
\"message\" \"C:\\maps\\hydrogen\"\n\
}\n\
{\n\
\"classname\" \"func_brush\"\n\
\"classname\" \"unsupported_duplicate\"\n\
\"targetname\" \"door\"\n\
\"parentname\" \"carrier,attachment\"\n\
\"spawnflags\" \"3\"\n\
\"model\" \"*7\"\n\
\"OnOpen\" \"target\x1bFireUser1\x1bpayload,with,commas\x1b0.25\x1b2\"\n\
\"OnOpen\" \"legacy,Trigger,,1.5,-1\"\n\
\"OnBroken\" \"target,Input,missing\"\n\
\"OnBadDelay\" \"target,Input,,later,1\"\n\
}\n"
    .to_vec()
}

#[test]
fn preserves_ordered_entities_duplicate_keys_identity_and_connections() {
    let graph = export_entity_graph(&bsp_with_entities(&representative_entities())).unwrap();
    let encoded = graph.to_json().unwrap();
    let value: Value = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(value["schema"], "bsp-to-glb.entity-graph");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["sourceBspVersion"], 20);
    assert_eq!(value["inventory"]["entityCount"], 2);
    assert_eq!(value["inventory"]["keyValueCount"], 12);
    assert_eq!(value["inventory"]["connectionCount"], 2);
    assert_eq!(value["inventory"]["malformedConnectionCount"], 2);
    assert_eq!(value["inventory"]["classCounts"]["worldspawn"], 1);
    assert_eq!(value["inventory"]["classCounts"]["func_brush"], 1);
    assert_eq!(value["inventory"]["outputCounts"]["OnOpen"], 2);
    assert_eq!(value["inventory"]["outputCounts"]["OnBroken"], 1);

    let world = &value["entities"][0];
    assert_eq!(world["index"], 0);
    assert_eq!(world["classname"], "worldspawn");
    assert_eq!(world["bspModelIndex"], 0);
    assert_eq!(world["keyValues"][1]["value"], r"C:\maps\hydrogen");

    let brush = &value["entities"][1];
    assert_eq!(brush["index"], 1);
    assert_eq!(brush["classname"], "func_brush");
    assert_eq!(brush["targetname"], "door");
    assert_eq!(brush["parentname"], "carrier,attachment");
    assert_eq!(brush["spawnflags"], "3");
    assert_eq!(brush["model"], "*7");
    assert_eq!(brush["bspModelIndex"], 7);
    assert_eq!(
        &brush["keyValues"].as_array().unwrap()[0..2],
        json!([
            { "key": "classname", "value": "func_brush" },
            { "key": "classname", "value": "unsupported_duplicate" }
        ])
        .as_array()
        .unwrap()
    );
    assert_eq!(
        brush["connections"][0],
        json!({
            "status": "parsed",
            "order": 6,
            "outputName": "OnOpen",
            "target": "target",
            "input": "FireUser1",
            "parameter": "payload,with,commas",
            "delay": 0.25,
            "maxFires": 2
        })
    );
    assert_eq!(
        brush["connections"][1],
        json!({
            "status": "parsed",
            "order": 7,
            "outputName": "OnOpen",
            "target": "legacy",
            "input": "Trigger",
            "parameter": "",
            "delay": 1.5,
            "maxFires": -1
        })
    );
    assert_eq!(brush["connections"][2]["status"], "malformed");
    assert_eq!(brush["connections"][2]["order"], 8);
    assert_eq!(brush["connections"][2]["error"], "fieldCount");
    assert_eq!(brush["connections"][3]["status"], "malformed");
    assert_eq!(brush["connections"][3]["order"], 9);
    assert_eq!(brush["connections"][3]["error"], "invalidDelay");

    let decoded: bsp_to_glb::EntityGraph = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(graph, decoded);
    assert_eq!(encoded, decoded.to_json().unwrap());
}

#[test]
fn does_not_guess_ambiguous_lowercase_metadata_as_an_output() {
    let entities = b"{\n\"classname\" \"logic_relay\"\n\"ontrigger\" \"target,Enable,missing\"\n\"OUTVALUE\" \"target,SetValue,1,0,1\"\n}\n";
    let graph = export_entity_graph(&bsp_with_entities(entities)).unwrap();

    assert_eq!(graph.inventory.connection_count, 1);
    assert_eq!(graph.inventory.malformed_connection_count, 0);
    assert_eq!(graph.entities[0].connections.len(), 1);
    assert_eq!(graph.inventory.output_counts.get("ontrigger"), None);
    assert_eq!(graph.inventory.output_counts["OUTVALUE"], 1);
}

#[test]
fn rejects_entity_property_counts_above_the_public_bound() {
    let mut entities = String::from("{\n\"classname\" \"worldspawn\"\n");
    for index in 0..MAX_ENTITY_KEY_VALUES_PER_ENTITY {
        entities.push_str(&format!("\"key{index}\" \"value\"\n"));
    }
    entities.push_str("}\n");

    let error = export_entity_graph(&bsp_with_entities(entities.as_bytes())).unwrap_err();
    assert!(
        error.contains("entity 0 key/value count exceeds"),
        "unexpected error: {error}"
    );
}

#[test]
fn cli_writes_an_entity_only_sidecar() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("entity-graph-cli-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let bsp_path = directory.join("synthetic.bsp");
    let entities_path = directory.join("synthetic.entities.json");
    fs::write(&bsp_path, bsp_with_entities(&representative_entities())).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bsp-to-glb"))
        .args([
            "--bsp",
            bsp_path.to_str().unwrap(),
            "--entities-out",
            entities_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sidecar: Value = serde_json::from_slice(&fs::read(&entities_path).unwrap()).unwrap();
    assert_eq!(sidecar["schemaVersion"], 1);
    assert_eq!(sidecar["inventory"]["entityCount"], 2);
    assert!(!directory.join("synthetic.glb").exists());

    fs::remove_dir_all(directory).unwrap();
}
