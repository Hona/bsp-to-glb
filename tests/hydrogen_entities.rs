use bsp_to_glb::export_entity_graph;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

fn hydrogen_bsp() -> Option<PathBuf> {
    env::var_os("BSP_TO_GLB_HYDROGEN_BSP")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            [
                PathBuf::from("tests/fixtures/jump_hydrogen_rc1_bmv.bsp"),
                PathBuf::from("jump_hydrogen_rc1_bmv.bsp"),
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

#[test]
fn hydrogen_entity_graph_preserves_the_compiled_inventory() {
    let Some(path) = hydrogen_bsp() else {
        eprintln!("Hydrogen BSP not present; set BSP_TO_GLB_HYDROGEN_BSP to run this gate");
        return;
    };
    let graph = export_entity_graph(&fs::read(path).unwrap()).unwrap();

    assert_eq!(graph.inventory.entity_count, 366);
    assert_eq!(graph.inventory.key_value_count, 6_927);
    assert_eq!(graph.inventory.connection_count, 196);
    assert_eq!(graph.inventory.malformed_connection_count, 0);
    assert_eq!(graph.inventory.entities_without_classname, 0);
    assert_eq!(
        graph.inventory.class_counts,
        counts(&[
            ("ambient_generic", 1),
            ("filter_tf_class", 2),
            ("func_brush", 100),
            ("func_illusionary", 20),
            ("func_regenerate", 1),
            ("game_text", 1),
            ("info_player_teamspawn", 1),
            ("info_teleport_destination", 16),
            ("light", 32),
            ("light_environment", 1),
            ("light_spot", 136),
            ("logic_case", 5),
            ("logic_relay", 8),
            ("logic_timer", 4),
            ("math_counter", 5),
            ("prop_dynamic", 1),
            ("team_control_point", 1),
            ("team_control_point_master", 1),
            ("trigger_capture_area", 1),
            ("trigger_catapult", 1),
            ("trigger_hurt", 1),
            ("trigger_multiple", 4),
            ("trigger_teleport", 22),
            ("worldspawn", 1),
        ])
    );
    assert_eq!(
        graph.inventory.output_counts,
        counts(&[
            ("OnCapTeam1", 1),
            ("OnCapTeam2", 1),
            ("OnCase01", 10),
            ("OnCase02", 10),
            ("OnCase03", 10),
            ("OnCase04", 10),
            ("OnCase05", 10),
            ("OnCase06", 10),
            ("OnCase07", 10),
            ("OnCase08", 8),
            ("OnCase09", 8),
            ("OnCase10", 8),
            ("OnHitMax", 6),
            ("OnStartTouch", 5),
            ("OnTimer", 16),
            ("OnTrigger", 68),
            ("OutValue", 5),
        ])
    );
    assert_eq!(graph.entities.len(), 366);
    assert_eq!(graph.entities[0].index, 0);
    assert_eq!(graph.entities[0].classname.as_deref(), Some("worldspawn"));
    assert_eq!(graph.entities[0].bsp_model_index, Some(0));
}
