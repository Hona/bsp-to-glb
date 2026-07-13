use bsp_to_glb::export_bsp_with_visibility;
use std::fs;

#[test]
#[ignore = "requires BSP_TO_GLB_HYDROGEN_BSP pointing to jump_hydrogen_rc1_bmv.bsp"]
fn hydrogen_has_complete_visibility_coverage() {
    let path = std::env::var_os("BSP_TO_GLB_HYDROGEN_BSP")
        .expect("set BSP_TO_GLB_HYDROGEN_BSP to the Hydrogen BSP path");
    let bsp = fs::read(path).unwrap();
    let result = export_bsp_with_visibility(&bsp, None).unwrap();
    let visibility = result.visibility.unwrap();

    assert_eq!(visibility.cluster_count, 450);
    assert_eq!(visibility.leaves.len(), 6_248);
    assert_eq!(visibility.relevant_cluster_count, 450);
    assert_eq!(visibility.covered_cluster_count, 450);
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
}
