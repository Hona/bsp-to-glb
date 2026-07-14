use bsp_to_glb::{EntityConnection, export_entity_graph};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

fn beef_bsp() -> Option<PathBuf> {
    env::var_os("BSP_TO_GLB_BEEF_BSP")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            [
                PathBuf::from("tests/fixtures/jump_beef.bsp"),
                PathBuf::from("jump_beef.bsp"),
            ]
            .into_iter()
            .find(|path| path.is_file())
        })
}

fn counts(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries
        .iter()
        .map(|(name, count)| ((*name).to_owned(), *count))
        .collect()
}

fn parsed_connections(
    graph: &bsp_to_glb::EntityGraph,
    entity_index: usize,
) -> Vec<(&str, &str, f32)> {
    graph.entities[entity_index]
        .connections
        .iter()
        .filter_map(|connection| match connection {
            EntityConnection::Parsed {
                target,
                input,
                delay,
                ..
            } => Some((target.as_str(), input.as_str(), *delay)),
            EntityConnection::Malformed { .. } => None,
        })
        .collect()
}

#[test]
fn beef_entity_graph_preserves_gameplay_identity_and_links() {
    let Some(path) = beef_bsp() else {
        eprintln!("Beef BSP not present; set BSP_TO_GLB_BEEF_BSP to run this gate");
        return;
    };
    let graph = export_entity_graph(&fs::read(path).unwrap()).unwrap();

    assert_eq!(graph.inventory.entity_count, 361);
    assert_eq!(graph.inventory.key_value_count, 3_674);
    assert_eq!(graph.inventory.connection_count, 66);
    assert_eq!(graph.inventory.malformed_connection_count, 1);
    assert_eq!(graph.inventory.entities_without_classname, 0);
    assert_eq!(
        graph.inventory.class_counts,
        counts(&[
            ("func_brush", 7),
            ("func_button", 5),
            ("func_door", 4),
            ("func_movelinear", 1),
            ("func_regenerate", 22),
            ("func_respawnroom", 3),
            ("game_text", 51),
            ("info_observer_point", 1),
            ("info_player_teamspawn", 10),
            ("info_teleport_destination", 25),
            ("infodecal", 39),
            ("item_ammopack_full", 3),
            ("light", 41),
            ("light_environment", 1),
            ("light_spot", 30),
            ("logic_auto", 1),
            ("prop_dynamic", 33),
            ("team_round_timer", 1),
            ("tf_gamerules", 1),
            ("trigger_hurt", 2),
            ("trigger_multiple", 22),
            ("trigger_teleport", 56),
            ("water_lod_control", 1),
            ("worldspawn", 1),
        ])
    );
    assert_eq!(
        graph.inventory.output_counts,
        counts(&[
            ("OnDamaged", 9),
            ("OnFullyClosed", 1),
            ("OnFullyOpen", 1),
            ("OnLoadGame", 1),
            ("OnMapSpawn", 4),
            ("OnStartTouch", 51),
        ])
    );

    assert_eq!(graph.entities[67].bsp_model_index, Some(26));
    assert_eq!(
        parsed_connections(&graph, 67),
        vec![("plat1", "Close", 0.0), ("plat1", "Open", 0.0)]
    );
    assert_eq!(
        parsed_connections(&graph, 192),
        vec![
            ("door2", "Open", 0.0),
            ("door2tele", "Disable", 0.0),
            ("door2tele", "Enable", 3.0),
        ]
    );
    assert_eq!(
        parsed_connections(&graph, 213),
        vec![("door1", "Open", 0.0)]
    );
    assert_eq!(
        parsed_connections(&graph, 214),
        vec![("door4", "Open", 0.0)]
    );
    assert_eq!(
        parsed_connections(&graph, 215),
        vec![("door3", "Open", 0.0)]
    );
    assert_eq!(
        parsed_connections(&graph, 353),
        parsed_connections(&graph, 192)
    );
    assert_eq!(graph.entities[192].bsp_model_index, Some(77));
    assert_eq!(graph.entities[353].bsp_model_index, Some(122));
    assert!(matches!(
        graph.entities[11].connections.last(),
        Some(EntityConnection::Malformed { .. })
    ));
}
