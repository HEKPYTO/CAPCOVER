mod util;

use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use capcover::index::CapCover;

#[test]
fn fnr_zero_matrix() {
    let kinds = [
        FixtureKind::Clustered,
        FixtureKind::Duplicates,
        FixtureKind::Uniform,
        FixtureKind::Edge,
    ];
    let seeds = [0u64, 1, 2, 3, 4];
    let thresholds = [0.80f32, 0.90, 0.95, 0.99];
    for kind in kinds {
        for seed in seeds {
            for &similarity in &thresholds {
                let fixture = generate(kind, 600, 32, seed, 40, 8, 0.08).unwrap();
                let budget_bytes = (fixture.vectors.n * fixture.vectors.dim * 4 / 10) as u64;
                let built = build(&fixture.vectors, &BuildConfig { budget_bytes, seed }).unwrap();
                let path = util::temp_path(&format!("zero-{kind:?}-{seed}-{similarity}.ccv"));
                built.save(&path).unwrap();
                let index = CapCover::open(&path).unwrap();
                let report = index.verify(&fixture.vectors, true).unwrap();
                assert_eq!(
                    report.uncovered, 0,
                    "kind={kind:?} seed={seed} s={similarity}"
                );
                let mut stored = vec![false; fixture.vectors.n];
                index
                    .may_contain_batch(&fixture.vectors, similarity, &mut stored)
                    .unwrap();
                assert!(
                    stored.iter().all(|value| *value),
                    "stored vector rejected kind={kind:?} seed={seed} s={similarity}"
                );
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
                        assert!(
                            *decision,
                            "true match rejected kind={kind:?} seed={seed} s={similarity} q={query} best={best}"
                        );
                    }
                }
                util::cleanup(&path);
            }
        }
    }
}
