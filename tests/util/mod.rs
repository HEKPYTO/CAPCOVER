#![allow(dead_code)]

use capcover::vector::{dot, Vectors};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn temp_path(name: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "capcover-test-{}-{}-{}",
        std::process::id(),
        id,
        name
    ))
}

pub fn brute_max(vectors: &Vectors, query: &[f32], query_norm: f64) -> f64 {
    let mut best = -1.0;
    for index in 0..vectors.n {
        let value = dot(query, vectors.row(index)) / (query_norm * vectors.norm(index));
        if value > best {
            best = value;
        }
    }
    best.clamp(-1.0, 1.0)
}

pub fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}
