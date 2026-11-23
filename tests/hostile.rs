mod util;

use capcover::build::{build, BuildConfig};
use capcover::fixture::{generate, FixtureKind};
use capcover::format::{checksum, layout, Header, HEADER_BYTES};
use capcover::index::CapCover;
use capcover::vector::Vectors;
use std::fs;

#[test]
fn rejects_bad_vectors() {
    assert!(Vectors::from_data(2, vec![f32::NAN, 0.0]).is_err());
    assert!(Vectors::from_data(2, vec![f32::INFINITY, 0.0]).is_err());
    assert!(Vectors::from_data(2, vec![0.0, 0.0]).is_err());
    assert!(Vectors::from_data(2, vec![1.0, 0.0, 0.0]).is_err());
    assert!(Vectors::from_data(0, vec![1.0]).is_err());
    assert!(Vectors::from_data(1025, vec![1.0; 1025]).is_err());
}

#[test]
fn rejects_bad_index_bytes() {
    let fixture = generate(FixtureKind::Clustered, 64, 8, 3, 8, 4, 0.1).unwrap();
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes: 4096,
            seed: 3,
        },
    )
    .unwrap();
    let path = util::temp_path("bad.ccv");
    built.save(&path).unwrap();
    let bytes = fs::read(&path).unwrap();
    let mut bad = bytes.clone();
    bad[0] = b'X';
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    bad[4..6].copy_from_slice(&2u16.to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    bad[6..8].copy_from_slice(&0u16.to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    bad[12..16].copy_from_slice(&(built.n as u32 + 1).to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    bad[48..52].copy_from_slice(&1u32.to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    bad[40..48].copy_from_slice(&(bytes.len() as u64 + 1).to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    bad[32..40].copy_from_slice(&0u64.to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let header = Header::decode(&bytes[..HEADER_BYTES]).unwrap();
    let layout = layout(header.dim as usize, header.k as usize, header.n as usize).unwrap();

    let mut bad = bytes.clone();
    bad[layout.direction_offset..layout.direction_offset + 4]
        .copy_from_slice(&2.0f32.to_le_bytes());
    let sum = checksum(&bad[HEADER_BYTES..]);
    bad[56..64].copy_from_slice(&sum.to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    bad[layout.assignment_offset..layout.assignment_offset + 4]
        .copy_from_slice(&header.k.to_le_bytes());
    let sum = checksum(&bad[HEADER_BYTES..]);
    bad[56..64].copy_from_slice(&sum.to_le_bytes());
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    let mut bad = bytes.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0xff;
    fs::write(&path, &bad).unwrap();
    assert!(CapCover::open(&path).is_err());

    fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(CapCover::open(&path).is_err());
    util::cleanup(&path);
}

#[test]
fn rejects_bad_queries_and_budget() {
    let fixture = generate(FixtureKind::Clustered, 64, 8, 3, 8, 4, 0.1).unwrap();
    assert!(build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes: 1,
            seed: 3,
        },
    )
    .is_err());
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes: 4096,
            seed: 3,
        },
    )
    .unwrap();
    let path = util::temp_path("query.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    assert!(index.may_contain(&[1.0, 0.0], 0.9).is_err());
    assert!(index.may_contain(&[1.0; 8], 1.5).is_err());
    assert!(index.may_contain(&[0.0; 8], 0.9).is_err());
    assert!(index.may_contain(&[f32::NAN; 8], 0.9).is_err());
    let mut decisions = vec![false; fixture.queries.n - 1];
    assert!(index
        .may_contain_batch(&fixture.queries, 0.9, &mut decisions)
        .is_err());
    let mut wrong_count = fixture.vectors.clone();
    wrong_count.n += 1;
    assert!(index.verify(&wrong_count, false).is_err());
    util::cleanup(&path);
}

#[test]
fn rejects_invalid_built_index() {
    let fixture = generate(FixtureKind::Clustered, 64, 8, 3, 8, 4, 0.1).unwrap();
    let mut built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes: 4096,
            seed: 3,
        },
    )
    .unwrap();
    let path = util::temp_path("invalid-built.ccv");

    built.dim = usize::MAX;
    assert!(built.save(&path).is_err());
    built.dim = 8;

    built.k = usize::MAX;
    assert!(built.save(&path).is_err());
    built.k = 3;

    built.directions.pop();
    assert!(built.save(&path).is_err());
    util::cleanup(&path);
}
