mod util;

use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use std::fs;

#[test]
fn same_seed_same_bytes() {
    let fixture = generate(FixtureKind::Clustered, 500, 32, 11, 20, 8, 0.08).unwrap();
    let config = BuildConfig {
        budget_bytes: 8192,
        seed: 11,
    };
    let first = build(&fixture.vectors, &config).unwrap();
    let second = build(&fixture.vectors, &config).unwrap();
    let first_path = util::temp_path("first.ccv");
    let second_path = util::temp_path("second.ccv");
    first.save(&first_path).unwrap();
    second.save(&second_path).unwrap();
    assert_eq!(
        fs::read(&first_path).unwrap(),
        fs::read(&second_path).unwrap()
    );
    util::cleanup(&first_path);
    util::cleanup(&second_path);
}
