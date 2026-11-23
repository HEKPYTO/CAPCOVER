mod util;

use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use capcover::format::layout;
use capcover::index::CapCover;
use capcover::vector::Vectors;
use std::fs;

#[test]
fn file_layout_matches_info() {
    let fixture = generate(FixtureKind::Duplicates, 300, 24, 5, 10, 4, 0.1).unwrap();
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes: 6000,
            seed: 5,
        },
    )
    .unwrap();
    let path = util::temp_path("layout.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    let info = index.info();
    let expected = layout(info.dim, info.k, info.n).unwrap();
    assert_eq!(
        info.file_bytes as usize,
        fs::metadata(&path).unwrap().len() as usize
    );
    assert_eq!(info.header_bytes, 256);
    assert_eq!(info.direction_bytes, expected.direction_bytes);
    assert_eq!(info.angle_bytes, expected.angle_bytes);
    assert_eq!(info.angle_padding, expected.angle_padding);
    assert_eq!(info.assignment_bytes, expected.assignment_bytes);
    assert_eq!(index.directions().len(), info.k * info.dim);
    assert_eq!(index.angles().len(), info.k);
    assert_eq!(index.assignment().len(), info.n);
    assert!(info.file_bytes <= info.budget_bytes);
    assert_eq!(index.verify(&fixture.vectors, false).unwrap().uncovered, 0);
    assert!(index.covers(fixture.vectors.row(0)).unwrap());
    util::cleanup(&path);
}

#[test]
fn edge_dimensions_build() {
    for dim in [1usize, 1024] {
        let vectors = Vectors::from_data(dim, vec![1.0; dim]).unwrap();
        let budget_bytes = 256 + 4 * dim as u64 + (4 * dim + 2) as u64 + 16;
        let built = build(
            &vectors,
            &BuildConfig {
                budget_bytes,
                seed: 0,
            },
        )
        .unwrap();
        assert_eq!(built.k, 1);
        let path = util::temp_path(&format!("edge-{dim}.ccv"));
        built.save(&path).unwrap();
        let index = CapCover::open(&path).unwrap();
        assert!(index.covers(vectors.row(0)).unwrap());
        assert_eq!(index.verify(&vectors, true).unwrap().uncovered, 0);
        util::cleanup(&path);
    }
}
