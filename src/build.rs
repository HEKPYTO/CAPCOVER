use crate::error::{Error, Result};
use crate::format::{angle_raw, checksum, layout, Header, HEADER_BYTES, MAX_DIM, MAX_N};
use crate::rng::SplitMix64;
use crate::vector::{cosine, dot, norm, Vectors};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct BuildConfig {
    pub budget_bytes: u64,
    pub seed: u64,
}

#[derive(Clone, Debug)]
pub struct BuiltIndex {
    pub dim: usize,
    pub n: usize,
    pub k: usize,
    pub seed: u64,
    pub budget_bytes: u64,
    pub directions: Vec<f32>,
    pub angles: Vec<u16>,
    pub assignment: Vec<u32>,
}

impl BuiltIndex {
    pub fn file_bytes(&self) -> Result<usize> {
        Ok(layout(self.dim, self.k, self.n)?.file_bytes)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if self.dim == 0 || self.dim > MAX_DIM || self.n == 0 || self.n as u64 > MAX_N {
            return Err(Error::InvalidData("index shape is out of bounds".into()));
        }
        if self.k == 0 || self.k > self.n {
            return Err(Error::InvalidData("cap count is out of bounds".into()));
        }
        if self.directions.len() != self.k * self.dim {
            return Err(Error::InvalidData(
                "direction section has the wrong size".into(),
            ));
        }
        if self.angles.len() != self.k {
            return Err(Error::InvalidData(
                "angle section has the wrong size".into(),
            ));
        }
        if self.assignment.len() != self.n {
            return Err(Error::InvalidData(
                "assignment section has the wrong size".into(),
            ));
        }
        let layout = layout(self.dim, self.k, self.n)?;
        if layout.file_bytes as u64 > self.budget_bytes {
            return Err(Error::BudgetTooSmall {
                budget: self.budget_bytes,
                minimum: layout.file_bytes as u64,
            });
        }
        let payload_len = layout.file_bytes - HEADER_BYTES;
        let mut payload = vec![0u8; payload_len];
        let direction_start = layout.direction_offset - HEADER_BYTES;
        for (index, value) in self.directions.iter().enumerate() {
            let offset = direction_start + index * 4;
            payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let angle_start = layout.angle_offset - HEADER_BYTES;
        for (index, value) in self.angles.iter().enumerate() {
            let offset = angle_start + index * 2;
            payload[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        let assignment_start = layout.assignment_offset - HEADER_BYTES;
        for (index, value) in self.assignment.iter().enumerate() {
            let offset = assignment_start + index * 4;
            payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let checksum = checksum(&payload);
        let header = Header {
            dim: self.dim as u32,
            k: self.k as u32,
            n: self.n as u64,
            seed: self.seed,
            budget_bytes: self.budget_bytes,
            file_bytes: layout.file_bytes as u64,
            encoding: crate::format::ENCODING_F32,
            checksum,
        };
        let mut bytes = Vec::with_capacity(layout.file_bytes);
        bytes.extend_from_slice(&header.encode());
        bytes.extend_from_slice(&payload);
        let tmp = path.with_extension("ccv.tmp");
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

struct CapBuild {
    center: Vec<f32>,
    theta: f64,
    members: Vec<u32>,
}

pub fn build(vectors: &Vectors, config: &BuildConfig) -> Result<BuiltIndex> {
    vectors.validate()?;
    let dim = vectors.dim;
    let n = vectors.n;
    let per_cap = (4 * dim + 2) as u64;
    let assignment_bytes = (4 * n) as u64;
    let minimum = (HEADER_BYTES as u64)
        .checked_add(assignment_bytes)
        .and_then(|value| value.checked_add(per_cap))
        .and_then(|value| value.checked_add(8))
        .ok_or_else(|| Error::InvalidData("budget minimum overflows".into()))?;
    if config.budget_bytes < minimum {
        return Err(Error::BudgetTooSmall {
            budget: config.budget_bytes,
            minimum,
        });
    }
    let available = config.budget_bytes - HEADER_BYTES as u64 - assignment_bytes - 8;
    let requested = (available / per_cap).clamp(1, n as u64) as usize;
    let mut leaves: Vec<Vec<u32>> = vec![(0..n as u32).collect()];
    let mut rng = SplitMix64::new(config.seed);
    while leaves.len() < requested {
        let mut target = None;
        let mut target_len = 1;
        for (index, leaf) in leaves.iter().enumerate() {
            if leaf.len() > target_len {
                target_len = leaf.len();
                target = Some(index);
            }
        }
        let Some(index) = target else {
            break;
        };
        let members = std::mem::take(&mut leaves[index]);
        let (left, right) = split(vectors, &members, &mut rng);
        leaves[index] = left;
        leaves.push(right);
    }
    let mut caps = Vec::with_capacity(leaves.len());
    for members in leaves {
        let first = members[0] as usize;
        let mut sum = vec![0f64; dim];
        for &index in &members {
            add(&mut sum, vectors.row(index as usize));
        }
        let center = normalized(&sum, vectors.row(first));
        let center_norm = norm(&center);
        let mut theta = 0f64;
        for &index in &members {
            let angle = cosine(
                &center,
                vectors.row(index as usize),
                center_norm,
                vectors.norm(index as usize),
            )
            .acos();
            if angle > theta {
                theta = angle;
            }
        }
        caps.push(CapBuild {
            center,
            theta,
            members,
        });
    }
    caps.sort_by_key(|cap| cap.members[0]);
    let k = caps.len();
    if k == 0 {
        return Err(Error::InvalidData("build produced no caps".into()));
    }
    let assigned: usize = caps.iter().map(|cap| cap.members.len()).sum();
    if assigned != n {
        return Err(Error::InvalidData(
            "build did not assign every vector".into(),
        ));
    }
    let mut directions = Vec::with_capacity(k * dim);
    let mut angles = Vec::with_capacity(k);
    let mut assignment = vec![0u32; n];
    for (cap_index, cap) in caps.iter().enumerate() {
        directions.extend_from_slice(&cap.center);
        angles.push(angle_raw(cap.theta));
        for &index in &cap.members {
            assignment[index as usize] = cap_index as u32;
        }
    }
    if assignment.iter().any(|&value| value as usize >= k) {
        return Err(Error::InvalidData(
            "assignment references a missing cap".into(),
        ));
    }
    let file_bytes = layout(dim, k, n)?.file_bytes as u64;
    if file_bytes > config.budget_bytes {
        return Err(Error::BudgetTooSmall {
            budget: config.budget_bytes,
            minimum: file_bytes,
        });
    }
    Ok(BuiltIndex {
        dim,
        n,
        k,
        seed: config.seed,
        budget_bytes: config.budget_bytes,
        directions,
        angles,
        assignment,
    })
}

fn split(vectors: &Vectors, members: &[u32], rng: &mut SplitMix64) -> (Vec<u32>, Vec<u32>) {
    let start = members[(rng.next_u64() as usize) % members.len()];
    let far = farthest(vectors, members, start);
    let near = farthest(vectors, members, far);
    let mut left_center = vectors.row(near as usize).to_vec();
    let mut right_center = vectors.row(far as usize).to_vec();
    let mut assignment = vec![0u8; members.len()];
    for _ in 0..4 {
        let mut left_sum = vec![0f64; vectors.dim];
        let mut right_sum = vec![0f64; vectors.dim];
        let mut left_count = 0usize;
        let mut right_count = 0usize;
        let mut min_left = (f64::INFINITY, 0usize);
        let mut min_right = (f64::INFINITY, 0usize);
        for (position, &index) in members.iter().enumerate() {
            let value = vectors.row(index as usize);
            let left_dot = dot(value, &left_center);
            let right_dot = dot(value, &right_center);
            if left_dot < min_left.0 {
                min_left = (left_dot, position);
            }
            if right_dot < min_right.0 {
                min_right = (right_dot, position);
            }
            if left_dot >= right_dot {
                assignment[position] = 0;
                add(&mut left_sum, value);
                left_count += 1;
            } else {
                assignment[position] = 1;
                add(&mut right_sum, value);
                right_count += 1;
            }
        }
        if left_count == 0 {
            let position = min_left.1;
            let value = vectors.row(members[position] as usize);
            subtract(&mut right_sum, value);
            add(&mut left_sum, value);
            assignment[position] = 0;
            left_count = 1;
            right_count -= 1;
        }
        if right_count == 0 {
            let position = min_right.1;
            let value = vectors.row(members[position] as usize);
            subtract(&mut left_sum, value);
            add(&mut right_sum, value);
            assignment[position] = 1;
            right_count = 1;
            left_count -= 1;
        }
        let fallback = vectors.row(members[0] as usize);
        left_center = normalized(&left_sum, fallback);
        right_center = normalized(&right_sum, fallback);
        assert!(left_count > 0 && right_count > 0);
    }
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (position, &index) in members.iter().enumerate() {
        if assignment[position] == 0 {
            left.push(index);
        } else {
            right.push(index);
        }
    }
    if left.is_empty() {
        left.push(right.pop().unwrap());
    }
    if right.is_empty() {
        right.push(left.pop().unwrap());
    }
    (left, right)
}

fn farthest(vectors: &Vectors, members: &[u32], from: u32) -> u32 {
    let origin = vectors.row(from as usize);
    let mut best = members[0];
    let mut best_dot = f64::INFINITY;
    for &index in members {
        let value = dot(origin, vectors.row(index as usize));
        if value < best_dot {
            best_dot = value;
            best = index;
        }
    }
    best
}

fn add(sum: &mut [f64], value: &[f32]) {
    for (slot, component) in sum.iter_mut().zip(value.iter()) {
        *slot += *component as f64;
    }
}

fn subtract(sum: &mut [f64], value: &[f32]) {
    for (slot, component) in sum.iter_mut().zip(value.iter()) {
        *slot -= *component as f64;
    }
}

fn normalized(sum: &[f64], fallback: &[f32]) -> Vec<f32> {
    let length = sum.iter().map(|value| value * value).sum::<f64>().sqrt();
    if !length.is_finite() || length < 1e-12 {
        return fallback.to_vec();
    }
    sum.iter().map(|value| (value / length) as f32).collect()
}
