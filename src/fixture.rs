use crate::error::{Error, Result};
use crate::format::{MAX_DIM, MAX_N};
use crate::rng::SplitMix64;
use crate::vector::Vectors;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureKind {
    Clustered,
    Duplicates,
    Uniform,
    Edge,
}

pub struct Fixture {
    pub vectors: Vectors,
    pub queries: Vectors,
}

pub fn generate(
    kind: FixtureKind,
    n: usize,
    dim: usize,
    seed: u64,
    query_count: usize,
    clusters: usize,
    sigma: f64,
) -> Result<Fixture> {
    if n == 0 {
        return Err(Error::InvalidData(
            "fixture needs at least one vector".into(),
        ));
    }
    if n as u64 > MAX_N {
        return Err(Error::InvalidData(format!(
            "vector count {n} exceeds {MAX_N}"
        )));
    }
    if query_count == 0 {
        return Err(Error::InvalidData(
            "fixture needs at least one query".into(),
        ));
    }
    if dim == 0 || dim > MAX_DIM {
        return Err(Error::InvalidData("dimension must be in 1..=1024".into()));
    }
    if clusters == 0 {
        return Err(Error::InvalidData("cluster count must be positive".into()));
    }
    if !sigma.is_finite() || sigma < 0.0 {
        return Err(Error::InvalidData(
            "sigma must be finite and non-negative".into(),
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let mut vectors = Vec::with_capacity(n * dim);
    let mut queries = Vec::with_capacity(query_count * dim);
    match kind {
        FixtureKind::Clustered => {
            let global = random_unit(&mut rng, dim);
            let centers: Vec<Vec<f32>> = (0..clusters)
                .map(|_| {
                    let mut center = global.clone();
                    for component in center.iter_mut() {
                        *component = (*component as f64 + 0.7 * rng.normal()) as f32;
                    }
                    normalize(&mut center);
                    center
                })
                .collect();
            let noise = sigma / (dim as f64).sqrt();
            for index in 0..n {
                let center = &centers[index % clusters];
                let mut value = Vec::with_capacity(dim);
                for component in center {
                    value.push((*component as f64 + noise * rng.normal()) as f32);
                }
                normalize(&mut value);
                vectors.extend_from_slice(&value);
            }
            for index in 0..query_count {
                if index % 5 < 3 {
                    let center = &centers[(index * 7 + 3) % clusters];
                    let mut value = Vec::with_capacity(dim);
                    for component in center {
                        value.push((*component as f64 + noise * rng.normal()) as f32);
                    }
                    normalize(&mut value);
                    queries.extend_from_slice(&value);
                } else {
                    queries.extend_from_slice(&random_unit(&mut rng, dim));
                }
            }
        }
        FixtureKind::Duplicates => {
            let distinct = (n / 5).max(1);
            let base: Vec<Vec<f32>> = (0..distinct).map(|_| random_unit(&mut rng, dim)).collect();
            let noise = 0.01 / (dim as f64).sqrt();
            for index in 0..n {
                vectors.extend_from_slice(&base[index % distinct]);
            }
            for index in 0..query_count {
                if index % 2 == 0 {
                    let source = &base[(index * 7 + 1) % distinct];
                    let mut value = Vec::with_capacity(dim);
                    for component in source {
                        value.push((*component as f64 + noise * rng.normal()) as f32);
                    }
                    normalize(&mut value);
                    queries.extend_from_slice(&value);
                } else {
                    queries.extend_from_slice(&random_unit(&mut rng, dim));
                }
            }
        }
        FixtureKind::Uniform => {
            for _ in 0..n {
                vectors.extend_from_slice(&random_unit(&mut rng, dim));
            }
            for _ in 0..query_count {
                queries.extend_from_slice(&random_unit(&mut rng, dim));
            }
        }
        FixtureKind::Edge => {
            for index in 0..n {
                let mut value = vec![0f32; dim];
                value[index % dim] = 1.0;
                if index >= dim {
                    value[0] += 0.05;
                }
                normalize(&mut value);
                vectors.extend_from_slice(&value);
            }
            for index in 0..query_count {
                let mut value = vectors[(index % n) * dim..(index % n + 1) * dim].to_vec();
                if index % 2 == 1 {
                    for component in value.iter_mut() {
                        *component = -*component;
                    }
                }
                normalize(&mut value);
                queries.extend_from_slice(&value);
            }
        }
    }
    Ok(Fixture {
        vectors: Vectors::from_data(dim, vectors)?,
        queries: Vectors::from_data(dim, queries)?,
    })
}

pub fn write_f32(path: &Path, vectors: &Vectors) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    for index in 0..vectors.n {
        for value in vectors.row(index) {
            writer.write_all(&value.to_le_bytes())?;
        }
    }
    writer.flush()?;
    Ok(())
}

fn random_unit(rng: &mut SplitMix64, dim: usize) -> Vec<f32> {
    let mut value: Vec<f32> = (0..dim).map(|_| rng.normal() as f32).collect();
    normalize(&mut value);
    value
}

fn normalize(value: &mut [f32]) {
    let length = value
        .iter()
        .map(|component| (*component as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    if !length.is_finite() || length < 1e-12 {
        value.fill(0.0);
        value[0] = 1.0;
        return;
    }
    for component in value.iter_mut() {
        *component = (*component as f64 / length) as f32;
    }
}
