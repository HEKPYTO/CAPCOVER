mod util;

use capcover::bench::run;
use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use capcover::index::CapCover;

#[test]
fn bench_reports_zero_fn() {
    let fixture = generate(FixtureKind::Duplicates, 200, 16, 9, 40, 4, 0.1).unwrap();
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes: 4000,
            seed: 9,
        },
    )
    .unwrap();
    let path = util::temp_path("bench.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    let report = run(&index, &fixture.vectors, &fixture.queries, 0.95, None).unwrap();
    assert_eq!(report.fn_, 0);
    assert_eq!(report.fnr, 0.0);
    assert_eq!(
        report.tp + report.fp + report.tn + report.fn_,
        fixture.queries.n as u64
    );
    assert!((0.0..=1.0).contains(&report.phi));
    assert!(report.baseline_ms > 0.0);
    assert!(report.build_s > 0.0);
    #[cfg(unix)]
    assert!(report.peak_rss_mb > 0.0);
    assert!(report.filter_p50_us >= 0.0);
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"fn\":0"));
    util::cleanup(&path);
}
