mod util;

use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use capcover::index::CapCover;
use capcover::vector::Vectors;

#[test]
fn low_similarity_true_matches_accepted() {
    let fixture = generate(FixtureKind::Clustered, 200, 8, 3, 20, 4, 0.1).unwrap();
    let budget_bytes = (fixture.vectors.n * fixture.vectors.dim * 4 / 5) as u64;
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes,
            seed: 3,
        },
    )
    .unwrap();
    let path = util::temp_path("similarity.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    for similarity in [1.0f32, 0.5, 0.0, -0.5, -0.99, -1.0] {
        let mut decisions = vec![false; fixture.queries.n];
        index
            .may_contain_batch(&fixture.queries, similarity, &mut decisions)
            .unwrap();
        for (query, decision) in decisions.iter().enumerate() {
            let best = util::brute_max(
                &fixture.vectors,
                fixture.queries.row(query),
                fixture.queries.norm(query),
            );
            if best >= similarity as f64 - 1e-12 {
                assert!(*decision, "s={similarity} q={query} best={best}");
            }
        }
    }
    util::cleanup(&path);

    let antipodal = Vectors::from_data(2, vec![1.0, 0.0, -1.0, 0.0]).unwrap();
    let built = build(
        &antipodal,
        &BuildConfig {
            budget_bytes: 282,
            seed: 0,
        },
    )
    .unwrap();
    assert_eq!(built.k, 1);
    let path = util::temp_path("antipodal.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    for similarity in [1.0f32, 0.5, 0.0, -0.5, -0.99, -1.0] {
        assert!(
            index.may_contain(&[0.0, 1.0], similarity).unwrap(),
            "s={similarity}"
        );
    }
    util::cleanup(&path);
}
