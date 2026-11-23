#![cfg(feature = "usearch")]

mod util;

use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use capcover::index::CapCover;
use capcover::usearch::run;

#[test]
fn usearch_gate_never_skips_a_true_match() {
    let fixture = generate(FixtureKind::Clustered, 400, 16, 4, 40, 8, 0.08).unwrap();
    let budget_bytes = (fixture.vectors.n * fixture.vectors.dim * 4 / 5) as u64;
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes,
            seed: 4,
        },
    )
    .unwrap();
    let path = util::temp_path("usearch.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    let report = run(&index, &fixture.vectors, &fixture.queries, 0.95, 5).unwrap();
    assert_eq!(report.fnr, 0.0);
    assert_eq!(report.gate_recall, 1.0);
    assert_eq!(
        report.tp + report.fp + report.tn + report.fn_,
        fixture.queries.n as u64
    );
    assert!(report.speedup > 0.0);
    assert!(report.usearch_build_s > 0.0);
    util::cleanup(&path);
}
