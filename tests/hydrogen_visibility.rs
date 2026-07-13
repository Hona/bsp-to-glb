use bsp_to_glb::export_bsp_with_visibility;
use std::fs;
use std::time::Instant;

#[test]
#[ignore = "requires BSP_TO_GLB_HYDROGEN_BSP pointing to jump_hydrogen_rc1_bmv.bsp"]
fn hydrogen_has_complete_visibility_coverage() {
    let path = std::env::var_os("BSP_TO_GLB_HYDROGEN_BSP")
        .expect("set BSP_TO_GLB_HYDROGEN_BSP to the Hydrogen BSP path");
    let bsp = fs::read(path).unwrap();
    let started = Instant::now();
    let result = export_bsp_with_visibility(&bsp, None).unwrap();
    let elapsed = started.elapsed();
    let visibility = result.visibility.unwrap();
    let json = visibility.to_json().unwrap();
    let mut v1_shape = serde_json::to_value(&visibility).unwrap();
    let v1_object = v1_shape.as_object_mut().unwrap();
    v1_object.insert("version".to_owned(), 1.into());
    v1_object.remove("planes");
    v1_object.remove("nodes");
    v1_object.remove("worldHeadNode");
    let v1_bytes = serde_json::to_vec(&v1_shape).unwrap().len();
    let growth = json.len() - v1_bytes;

    assert_eq!(visibility.version, 2);
    assert_eq!(visibility.planes.len(), 16_244);
    assert_eq!(visibility.nodes.len(), 6_096);
    assert_eq!(visibility.world_head_node, 0);
    assert_eq!(visibility.cluster_count, 450);
    assert_eq!(visibility.leaves.len(), 6_248);
    let leaf = visibility
        .locate_world_leaf([3266.0, 5716.0, -960.0])
        .unwrap();
    assert_eq!(leaf, 1_120);
    assert_eq!(visibility.leaves[leaf].cluster, 394);
    assert_eq!(visibility.relevant_cluster_count, 435);
    assert_eq!(
        visibility.covered_cluster_count,
        visibility.relevant_cluster_count
    );
    assert_eq!(
        visibility.pvs_words.len(),
        visibility.cluster_count * visibility.cluster_word_count
    );
    assert_eq!(
        visibility.world_face_cluster_words.len(),
        visibility.world_face_indices.len() * visibility.cluster_word_count
    );
    assert_eq!(
        visibility.chunk_cluster_words.len(),
        visibility.chunks.len() * visibility.cluster_word_count
    );
    eprintln!(
        "Hydrogen visibility v2: {} bytes (+{} from v1's {}, {:.1}%) in {:.3?} (planes={}, nodes={})",
        json.len(),
        growth,
        v1_bytes,
        growth as f64 / v1_bytes as f64 * 100.0,
        elapsed,
        visibility.planes.len(),
        visibility.nodes.len()
    );
}
