use foundation_vectors::fusion::{FusionStrategy, fuse};
use foundation_vectors::store::VectorMatch;

fn vm(id: &str, score: f32) -> VectorMatch {
    VectorMatch {
        id: id.into(),
        score,
    }
}

#[test]
fn rrf_fuses_two_lists() {
    let vector = vec![vm("a", 0.9), vm("b", 0.8), vm("c", 0.7)];
    let keyword = vec![vm("b", 5.0), vm("d", 4.0), vm("a", 3.0)];

    let fused = fuse(&vector, &keyword, 4, FusionStrategy::Rrf { k: 60.0 });

    assert!(fused.len() <= 4);
    let ids: Vec<&str> = fused.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
}

#[test]
fn rrf_ranks_shared_items_higher() {
    let vector = vec![vm("shared", 0.9), vm("vec_only", 0.8)];
    let keyword = vec![vm("shared", 5.0), vm("kw_only", 4.0)];

    let fused = fuse(&vector, &keyword, 3, FusionStrategy::Rrf { k: 60.0 });

    assert_eq!(fused[0].id, "shared");
}

#[test]
fn alpha_pure_vector() {
    let vector = vec![vm("a", 0.9), vm("b", 0.7), vm("e", 0.5), vm("h", 0.3)];
    let keyword = vec![vm("c", 5.0), vm("d", 4.0)];

    let fused = fuse(&vector, &keyword, 6, FusionStrategy::Alpha { alpha: 1.0 });

    assert_eq!(fused[0].id, "a");
    assert_eq!(fused[1].id, "b");
    assert_eq!(fused[2].id, "e");
    assert!(fused[0].score > fused[1].score);
    assert!(fused[1].score > fused[2].score);
}

#[test]
fn alpha_pure_keyword() {
    let vector = vec![vm("a", 0.9), vm("b", 0.5)];
    let keyword = vec![vm("c", 5.0), vm("d", 4.0), vm("f", 3.0), vm("g", 2.0)];

    let fused = fuse(&vector, &keyword, 6, FusionStrategy::Alpha { alpha: 0.0 });

    assert_eq!(fused[0].id, "c");
    assert_eq!(fused[1].id, "d");
    assert_eq!(fused[2].id, "f");
    assert!(fused[0].score > fused[1].score);
    assert!(fused[1].score > fused[2].score);
}

#[test]
fn empty_lists_produce_empty() {
    let fused = fuse(&[], &[], 10, FusionStrategy::default());
    assert!(fused.is_empty());
}

#[test]
fn one_empty_list_still_works() {
    let vector = vec![vm("a", 0.9), vm("b", 0.5)];
    let fused = fuse(&vector, &[], 10, FusionStrategy::Rrf { k: 60.0 });
    assert_eq!(fused.len(), 2);
}
