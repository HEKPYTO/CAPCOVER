use crate::error::{Error, Result};
use crate::format::{angle_decode, checksum, layout, Header, Layout, HEADER_BYTES};
use crate::query::{alpha_from_similarity, EPS};
use crate::vector::{dot, norm, Vectors};
use crate::verify::VerifyReport;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub dim: usize,
    pub n: usize,
    pub k: usize,
    pub seed: u64,
    pub budget_bytes: u64,
    pub file_bytes: u64,
    pub header_bytes: usize,
    pub direction_bytes: usize,
    pub angle_bytes: usize,
    pub angle_padding: usize,
    pub assignment_bytes: usize,
    pub encoding: &'static str,
    pub checksum: u64,
}

pub struct CapCover {
    mmap: Mmap,
    dim: usize,
    n: usize,
    k: usize,
    seed: u64,
    budget_bytes: u64,
    file_bytes: u64,
    checksum: u64,
    layout: Layout,
    cap_norms: Vec<f64>,
    theta: Vec<f64>,
    cos_theta: Vec<f64>,
    sin_theta: Vec<f64>,
}

impl CapCover {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap.len() < HEADER_BYTES {
            return Err(Error::InvalidHeader("file is shorter than the header"));
        }
        let header = Header::decode(&mmap[..HEADER_BYTES])?;
        let dim = usize::try_from(header.dim)
            .map_err(|_| Error::InvalidHeader("dimension does not fit usize"))?;
        let k = usize::try_from(header.k)
            .map_err(|_| Error::InvalidHeader("cap count does not fit usize"))?;
        let n = usize::try_from(header.n)
            .map_err(|_| Error::InvalidHeader("vector count does not fit usize"))?;
        let layout = layout(dim, k, n)?;
        let file_bytes = usize::try_from(header.file_bytes)
            .map_err(|_| Error::InvalidHeader("file length does not fit usize"))?;
        if file_bytes != mmap.len() {
            return Err(Error::InvalidHeader(
                "file length does not match the header",
            ));
        }
        if layout.file_bytes != mmap.len() {
            return Err(Error::InvalidHeader(
                "sections do not match the file length",
            ));
        }
        if header.budget_bytes < header.file_bytes {
            return Err(Error::InvalidHeader("file exceeds its recorded budget"));
        }
        let payload = &mmap[HEADER_BYTES..];
        if checksum(payload) != header.checksum {
            return Err(Error::InvalidHeader("checksum mismatch"));
        }
        let directions: &[f32] = bytemuck::cast_slice(
            &mmap[layout.direction_offset..layout.direction_offset + layout.direction_bytes],
        );
        let angles: &[u16] = bytemuck::cast_slice(
            &mmap[layout.angle_offset..layout.angle_offset + layout.angle_bytes],
        );
        let assignment: &[u32] = bytemuck::cast_slice(
            &mmap[layout.assignment_offset..layout.assignment_offset + layout.assignment_bytes],
        );
        if directions.len() != k * dim {
            return Err(Error::InvalidHeader("direction section has the wrong size"));
        }
        if angles.len() != k {
            return Err(Error::InvalidHeader("angle section has the wrong size"));
        }
        if assignment.len() != n {
            return Err(Error::InvalidHeader(
                "assignment section has the wrong size",
            ));
        }
        let mut cap_norms = Vec::with_capacity(k);
        let mut theta_values = Vec::with_capacity(k);
        let mut cos_theta = Vec::with_capacity(k);
        let mut sin_theta = Vec::with_capacity(k);
        for cap in 0..k {
            let direction = &directions[cap * dim..(cap + 1) * dim];
            let length = norm(direction);
            if !length.is_finite() || (length - 1.0).abs() > 1e-3 {
                return Err(Error::InvalidHeader("cap direction is not unit length"));
            }
            let theta = angle_decode(angles[cap]);
            if !(0.0..=std::f64::consts::PI).contains(&theta) {
                return Err(Error::InvalidHeader("cap angle is outside [0, pi]"));
            }
            cap_norms.push(length);
            theta_values.push(theta);
            cos_theta.push(theta.cos());
            sin_theta.push(theta.sin());
        }
        if assignment.iter().any(|&value| value as usize >= k) {
            return Err(Error::InvalidHeader("assignment references a missing cap"));
        }
        Ok(Self {
            mmap,
            dim,
            n,
            k,
            seed: header.seed,
            budget_bytes: header.budget_bytes,
            file_bytes: header.file_bytes,
            checksum: header.checksum,
            layout,
            cap_norms,
            theta: theta_values,
            cos_theta,
            sin_theta,
        })
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn k(&self) -> usize {
        self.k
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn file_bytes(&self) -> u64 {
        self.file_bytes
    }

    pub fn budget_bytes(&self) -> u64 {
        self.budget_bytes
    }

    pub fn directions(&self) -> &[f32] {
        bytemuck::cast_slice(
            &self.mmap[self.layout.direction_offset
                ..self.layout.direction_offset + self.layout.direction_bytes],
        )
    }

    pub fn angles(&self) -> &[u16] {
        bytemuck::cast_slice(
            &self.mmap
                [self.layout.angle_offset..self.layout.angle_offset + self.layout.angle_bytes],
        )
    }

    pub fn assignment(&self) -> &[u32] {
        bytemuck::cast_slice(
            &self.mmap[self.layout.assignment_offset
                ..self.layout.assignment_offset + self.layout.assignment_bytes],
        )
    }

    pub fn info(&self) -> Info {
        Info {
            dim: self.dim,
            n: self.n,
            k: self.k,
            seed: self.seed,
            budget_bytes: self.budget_bytes,
            file_bytes: self.file_bytes,
            header_bytes: HEADER_BYTES,
            direction_bytes: self.layout.direction_bytes,
            angle_bytes: self.layout.angle_bytes,
            angle_padding: self.layout.angle_padding,
            assignment_bytes: self.layout.assignment_bytes,
            encoding: "f32",
            checksum: self.checksum,
        }
    }

    pub fn may_contain(&self, query: &[f32], similarity: f32) -> Result<bool> {
        let query_norm = self.validate_query(query)?;
        let alpha = alpha_from_similarity(similarity)?;
        if similarity <= -1.0 {
            return Ok(true);
        }
        let directions = self.directions();
        Ok(self.decide(
            directions,
            query,
            query_norm,
            alpha,
            alpha.cos(),
            alpha.sin(),
        ))
    }

    pub fn may_contain_batch(
        &self,
        queries: &Vectors,
        similarity: f32,
        out: &mut [bool],
    ) -> Result<()> {
        if queries.dim != self.dim {
            return Err(Error::InvalidData(format!(
                "query dimension {} does not match index dimension {}",
                queries.dim, self.dim
            )));
        }
        queries.validate()?;
        if out.len() != queries.n {
            return Err(Error::InvalidData(
                "decision buffer length does not match query count".into(),
            ));
        }
        let alpha = alpha_from_similarity(similarity)?;
        if similarity <= -1.0 {
            out.fill(true);
            return Ok(());
        }
        let cos_alpha = alpha.cos();
        let sin_alpha = alpha.sin();
        let directions = self.directions();
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = self.decide(
                directions,
                queries.row(index),
                queries.norm(index),
                alpha,
                cos_alpha,
                sin_alpha,
            );
        }
        Ok(())
    }

    pub fn covers(&self, value: &[f32]) -> Result<bool> {
        let value_norm = self.validate_query(value)?;
        let directions = self.directions();
        Ok((0..self.k).any(|cap| self.covers_cap(directions, value, value_norm, cap)))
    }

    pub fn verify(&self, vectors: &Vectors, full: bool) -> Result<VerifyReport> {
        if vectors.dim != self.dim {
            return Err(Error::InvalidData(format!(
                "vector dimension {} does not match index dimension {}",
                vectors.dim, self.dim
            )));
        }
        if vectors.n != self.n {
            return Err(Error::InvalidData(format!(
                "vector count {} does not match index count {}",
                vectors.n, self.n
            )));
        }
        vectors.validate()?;
        let assignment = self.assignment();
        let directions = self.directions();
        let mut uncovered = 0usize;
        let mut first_uncovered = None;
        for (index, &assigned) in assignment.iter().enumerate() {
            let value = vectors.row(index);
            let value_norm = vectors.norm(index);
            let covered = if full {
                (0..self.k).any(|cap| self.covers_cap(directions, value, value_norm, cap))
            } else {
                self.covers_cap(directions, value, value_norm, assigned as usize)
            };
            if !covered {
                uncovered += 1;
                if first_uncovered.is_none() {
                    first_uncovered = Some(index);
                }
            }
        }
        Ok(VerifyReport {
            n: self.n,
            uncovered,
            first_uncovered,
            full,
        })
    }

    fn validate_query(&self, query: &[f32]) -> Result<f64> {
        if query.len() != self.dim {
            return Err(Error::InvalidData(format!(
                "query dimension {} does not match index dimension {}",
                query.len(),
                self.dim
            )));
        }
        if query.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidData("non-finite query component".into()));
        }
        let length = norm(query);
        if !length.is_finite() || length < 1e-12 {
            return Err(Error::InvalidData("zero or non-finite query norm".into()));
        }
        Ok(length)
    }

    fn decide(
        &self,
        directions: &[f32],
        query: &[f32],
        query_norm: f64,
        alpha: f64,
        cos_alpha: f64,
        sin_alpha: f64,
    ) -> bool {
        for cap in 0..self.k {
            let direction = &directions[cap * self.dim..(cap + 1) * self.dim];
            let cos_value = dot(query, direction) / (query_norm * self.cap_norms[cap]);
            let bound = if self.theta[cap] + alpha >= std::f64::consts::PI {
                -1.0
            } else {
                self.cos_theta[cap] * cos_alpha - self.sin_theta[cap] * sin_alpha
            };
            if cos_value >= bound - EPS {
                return true;
            }
        }
        false
    }

    fn covers_cap(&self, directions: &[f32], value: &[f32], value_norm: f64, cap: usize) -> bool {
        let direction = &directions[cap * self.dim..(cap + 1) * self.dim];
        let cos_value = dot(value, direction) / (value_norm * self.cap_norms[cap]);
        cos_value >= self.cos_theta[cap] - EPS
    }
}
