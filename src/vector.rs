use crate::error::{Error, Result};
use crate::format::{MAX_DIM, MAX_N};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Vectors {
    pub dim: usize,
    pub n: usize,
    pub data: Vec<f32>,
    pub norms: Vec<f64>,
}

impl Vectors {
    pub fn from_data(dim: usize, mut data: Vec<f32>) -> Result<Self> {
        if dim == 0 || dim > MAX_DIM {
            return Err(Error::InvalidData("dimension must be in 1..=1024".into()));
        }
        if data.len() % dim != 0 {
            return Err(Error::InvalidData(format!(
                "data length {} is not a multiple of dimension {dim}",
                data.len()
            )));
        }
        let n = data.len() / dim;
        if n == 0 {
            return Err(Error::InvalidData("no vectors".into()));
        }
        if n as u64 > MAX_N {
            return Err(Error::InvalidData(format!(
                "vector count {n} exceeds {MAX_N}"
            )));
        }
        let mut norms = Vec::with_capacity(n);
        for row in data.chunks_mut(dim) {
            if row.iter().any(|value| !value.is_finite()) {
                return Err(Error::InvalidData("non-finite vector component".into()));
            }
            let norm = row
                .iter()
                .map(|value| (*value as f64).powi(2))
                .sum::<f64>()
                .sqrt();
            if !norm.is_finite() || norm < 1e-12 {
                return Err(Error::InvalidData("zero or non-finite vector norm".into()));
            }
            let inverse = 1.0 / norm;
            for value in row.iter_mut() {
                *value = (*value as f64 * inverse) as f32;
            }
            let normalized = row
                .iter()
                .map(|value| (*value as f64).powi(2))
                .sum::<f64>()
                .sqrt();
            norms.push(normalized);
        }
        Ok(Self {
            dim,
            n,
            data,
            norms,
        })
    }

    pub fn load_f32(path: &Path, dim: usize) -> Result<Self> {
        if dim == 0 || dim > MAX_DIM {
            return Err(Error::InvalidData("dimension must be in 1..=1024".into()));
        }
        let bytes = fs::read(path)?;
        if bytes.len() % 4 != 0 {
            return Err(Error::InvalidData(
                "f32 file length is not a multiple of 4".into(),
            ));
        }
        let values = bytes.len() / 4;
        if values % dim != 0 {
            return Err(Error::InvalidData(format!(
                "f32 file has {values} values, not a multiple of dimension {dim}"
            )));
        }
        let mut data = Vec::with_capacity(values);
        for chunk in bytes.chunks_exact(4) {
            data.push(f32::from_le_bytes(chunk.try_into().unwrap()));
        }
        Self::from_data(dim, data)
    }

    pub fn load_fvecs(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        let mut offset = 0usize;
        let mut dim = None;
        let mut data = Vec::new();
        while offset < bytes.len() {
            if offset + 4 > bytes.len() {
                return Err(Error::InvalidData("truncated fvecs dimension".into()));
            }
            let record_dim = i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            if record_dim <= 0 {
                return Err(Error::InvalidData(
                    "fvecs dimension must be positive".into(),
                ));
            }
            let record_dim = record_dim as usize;
            if record_dim > MAX_DIM {
                return Err(Error::InvalidData("fvecs dimension exceeds 1024".into()));
            }
            match dim {
                Some(expected) if expected != record_dim => {
                    return Err(Error::InvalidData("inconsistent fvecs dimensions".into()));
                }
                None => dim = Some(record_dim),
                _ => {}
            }
            offset += 4;
            let record_bytes = record_dim
                .checked_mul(4)
                .ok_or_else(|| Error::InvalidData("fvecs record overflows".into()))?;
            if offset + record_bytes > bytes.len() {
                return Err(Error::InvalidData("truncated fvecs record".into()));
            }
            for chunk in bytes[offset..offset + record_bytes].chunks_exact(4) {
                data.push(f32::from_le_bytes(chunk.try_into().unwrap()));
            }
            offset += record_bytes;
        }
        let dim = dim.ok_or_else(|| Error::InvalidData("empty fvecs file".into()))?;
        Self::from_data(dim, data)
    }

    pub fn load(path: &Path, dim: Option<usize>) -> Result<Self> {
        let is_fvecs = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("fvecs"))
            .unwrap_or(false);
        if is_fvecs {
            Self::load_fvecs(path)
        } else {
            let dim = dim
                .ok_or_else(|| Error::InvalidData("dimension is required for .f32 input".into()))?;
            Self::load_f32(path, dim)
        }
    }

    pub fn load_queries(path: &Path, dim: usize) -> Result<Self> {
        let vectors = Self::load(path, Some(dim))?;
        if vectors.dim != dim {
            return Err(Error::InvalidData(format!(
                "query dimension {} does not match index dimension {dim}",
                vectors.dim
            )));
        }
        Ok(vectors)
    }

    pub fn validate(&self) -> Result<()> {
        if self.dim == 0 || self.dim > MAX_DIM {
            return Err(Error::InvalidData("dimension must be in 1..=1024".into()));
        }
        if self.n == 0 || self.n as u64 > MAX_N {
            return Err(Error::InvalidData(
                "vector count is outside 1..=250000".into(),
            ));
        }
        let expected = self
            .n
            .checked_mul(self.dim)
            .ok_or_else(|| Error::InvalidData("vector table size overflows".into()))?;
        if self.data.len() != expected || self.norms.len() != self.n {
            return Err(Error::InvalidData("vector table is inconsistent".into()));
        }
        for (index, row) in self.data.chunks(self.dim).enumerate() {
            if row.iter().any(|value| !value.is_finite()) {
                return Err(Error::InvalidData("non-finite vector component".into()));
            }
            let computed = norm(row);
            if !computed.is_finite() || computed < 1e-12 {
                return Err(Error::InvalidData("zero or non-finite vector norm".into()));
            }
            if self.norms[index] != computed {
                return Err(Error::InvalidData("cached vector norm is stale".into()));
            }
        }
        Ok(())
    }

    pub fn row(&self, index: usize) -> &[f32] {
        let start = index * self.dim;
        &self.data[start..start + self.dim]
    }

    pub fn norm(&self, index: usize) -> f64 {
        self.norms[index]
    }
}

pub fn dot(a: &[f32], b: &[f32]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(left, right)| *left as f64 * *right as f64)
        .sum()
}

pub fn norm(values: &[f32]) -> f64 {
    values
        .iter()
        .map(|value| (*value as f64).powi(2))
        .sum::<f64>()
        .sqrt()
}

pub fn cosine(a: &[f32], b: &[f32], a_norm: f64, b_norm: f64) -> f64 {
    if a_norm <= 0.0 || b_norm <= 0.0 {
        return f64::NAN;
    }
    (dot(a, b) / (a_norm * b_norm)).clamp(-1.0, 1.0)
}
