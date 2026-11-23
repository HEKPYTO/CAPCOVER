mod util;

use capcover::bench::run;
use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use capcover::index::CapCover;

#[test]
fn clustered_phi_gate() {
    let fixture = generate(FixtureKind::Clustered, 5000, 64, 7, 200, 32, 0.05).unwrap();
    let budget_bytes = (fixture.vectors.n * fixture.vectors.dim * 4 / 50) as u64;
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes,
            seed: 7,
        },
    )
    .unwrap();
    let path = util::temp_path("quality.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    let report = run(&index, &fixture.vectors, &fixture.queries, 0.95, None).unwrap();
    assert_eq!(report.fn_, 0);
    assert!(report.phi >= 0.25, "phi {}", report.phi);
    assert!(report.build_s > 0.0);
    util::cleanup(&path);
}
