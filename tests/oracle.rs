mod util;

use capcover::build::{build, BuildConfig};
use capcover::format::{angle_decode, angle_raw};
use capcover::index::CapCover;
use capcover::query::alpha_from_similarity;
use capcover::vector::{cosine, norm, Vectors};

#[test]
fn five_hand_computed_cosines() {
    let cases = [
        (vec![1.0f32, 0.0], vec![1.0f32, 0.0], 1.0f64),
        (vec![1.0, 0.0], vec![0.0, 1.0], 0.0),
        (vec![1.0, 0.0], vec![-1.0, 0.0], -1.0),
        (vec![0.6, 0.8], vec![0.8, 0.6], 0.96),
        (
            vec![1.0, 0.0, 0.0],
            vec![
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
            ],
            std::f32::consts::FRAC_1_SQRT_2 as f64,
        ),
    ];
    for (left, right, expected) in cases {
        let left_norm = norm(&left);
        let right_norm = norm(&right);
        let value = cosine(&left, &right, left_norm, right_norm);
        assert!((value - expected).abs() < 1e-6, "{value} vs {expected}");
    }
}

#[test]
fn threshold_conversion() {
    assert!((alpha_from_similarity(0.95).unwrap() - 0.31756046746904604).abs() < 1e-12);
    assert!((alpha_from_similarity(0.0).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    assert!((alpha_from_similarity(1.0).unwrap()).abs() < 1e-12);
    assert!((alpha_from_similarity(-1.0).unwrap() - std::f64::consts::PI).abs() < 1e-12);
    assert!(alpha_from_similarity(1.5).is_err());
    assert!(alpha_from_similarity(-1.5).is_err());
    assert!(alpha_from_similarity(f32::NAN).is_err());
}

#[test]
fn angle_rounding_is_outward() {
    for theta in [0.0, 0.001, 0.1, 0.31756046746904604, 1.0, 3.0] {
        let raw = angle_raw(theta);
        let decoded = angle_decode(raw);
        assert!(decoded >= theta, "{decoded} < {theta}");
    }
    assert_eq!(angle_raw(std::f64::consts::PI), 65535);
    assert_eq!(angle_decode(65535), std::f64::consts::PI);
}

#[test]
fn expanded_cap_past_pi_accepts_stored_vector() {
    let vectors = Vectors::from_data(2, vec![1.0, 0.0, -1.0, 0.0]).unwrap();
    let built = build(
        &vectors,
        &BuildConfig {
            budget_bytes: 282,
            seed: 0,
        },
    )
    .unwrap();
    assert_eq!(built.k, 1);
    let path = util::temp_path("pi.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    assert!(index.may_contain(&[-1.0, 0.0], 0.95).unwrap());
    assert!(index.may_contain(&[-1.0, 0.0], 1.0).unwrap());
    assert_eq!(index.verify(&vectors, true).unwrap().uncovered, 0);
    util::cleanup(&path);
}

#[test]
fn filter_matches_hand_vectors() {
    let vectors =
        Vectors::from_data(2, vec![1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.6, 0.8, 0.8, 0.6]).unwrap();
    let built = build(
        &vectors,
        &BuildConfig {
            budget_bytes: 1000,
            seed: 0,
        },
    )
    .unwrap();
    let path = util::temp_path("hand.ccv");
    built.save(&path).unwrap();
    let index = CapCover::open(&path).unwrap();
    assert_eq!(index.verify(&vectors, true).unwrap().uncovered, 0);
    let query = [
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ];
    assert!(!index.may_contain(&query, 0.99).unwrap());
    assert!(index.may_contain(&query, 0.95).unwrap());
    assert!(index.may_contain(&[0.6, 0.8], 0.99).unwrap());
    assert!(index.may_contain(&[0.6, 0.8], 1.0).unwrap());
    util::cleanup(&path);
}
