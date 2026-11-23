mod util;

use capcover::vector::Vectors;
use std::fs;

#[test]
fn loads_fvecs_and_f32() {
    let dim = 3usize;
    let records = [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let mut fvecs_bytes = Vec::new();
    for record in records {
        fvecs_bytes.extend_from_slice(&(dim as i32).to_le_bytes());
        for value in record {
            fvecs_bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    let fvecs_path = util::temp_path("vectors.fvecs");
    fs::write(&fvecs_path, fvecs_bytes).unwrap();
    let vectors = Vectors::load_fvecs(&fvecs_path).unwrap();
    assert_eq!(vectors.dim, 3);
    assert_eq!(vectors.n, 2);
    util::cleanup(&fvecs_path);

    let mut f32_bytes = Vec::new();
    for record in records {
        for value in record {
            f32_bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    let f32_path = util::temp_path("vectors.f32");
    fs::write(&f32_path, f32_bytes).unwrap();
    let vectors = Vectors::load_f32(&f32_path, dim).unwrap();
    assert_eq!(vectors.n, 2);
    util::cleanup(&f32_path);
}

#[test]
fn rejects_truncated_fvecs() {
    let path = util::temp_path("truncated.fvecs");
    fs::write(&path, [3u8, 0, 0, 0, 0, 0]).unwrap();
    assert!(Vectors::load_fvecs(&path).is_err());
    util::cleanup(&path);
}
