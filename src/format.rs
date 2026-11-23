use crate::error::{Error, Result};
use xxhash_rust::xxh3::xxh3_64;

pub const MAGIC: [u8; 4] = *b"CCV1";
pub const VERSION: u16 = 1;
pub const HEADER_BYTES: usize = 256;
pub const ENCODING_F32: u32 = 0;
pub const MAX_DIM: usize = 1024;
pub const MAX_N: u64 = 250_000;
pub const ANGLE_STEP: f64 = std::f64::consts::PI / 65535.0;

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub dim: u32,
    pub k: u32,
    pub n: u64,
    pub seed: u64,
    pub budget_bytes: u64,
    pub file_bytes: u64,
    pub encoding: u32,
    pub checksum: u64,
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0u8; HEADER_BYTES];
        bytes[0..4].copy_from_slice(&MAGIC);
        bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
        bytes[6..8].copy_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
        bytes[8..12].copy_from_slice(&self.dim.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.k.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.n.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.seed.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.budget_bytes.to_le_bytes());
        bytes[40..48].copy_from_slice(&self.file_bytes.to_le_bytes());
        bytes[48..52].copy_from_slice(&self.encoding.to_le_bytes());
        bytes[56..64].copy_from_slice(&self.checksum.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(Error::InvalidHeader("file is shorter than the header"));
        }
        if bytes[0..4] != MAGIC {
            return Err(Error::InvalidHeader("bad magic"));
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != VERSION {
            return Err(Error::InvalidHeader("unsupported version"));
        }
        let header_len = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
        if header_len as usize != HEADER_BYTES {
            return Err(Error::InvalidHeader("bad header length"));
        }
        let dim = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let k = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let n = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let seed = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        let budget_bytes = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
        let file_bytes = u64::from_le_bytes(bytes[40..48].try_into().unwrap());
        let encoding = u32::from_le_bytes(bytes[48..52].try_into().unwrap());
        let checksum = u64::from_le_bytes(bytes[56..64].try_into().unwrap());
        if dim == 0 || dim as usize > MAX_DIM {
            return Err(Error::InvalidHeader("dimension must be in 1..=1024"));
        }
        if k == 0 {
            return Err(Error::InvalidHeader("cap count must be positive"));
        }
        if n == 0 {
            return Err(Error::InvalidHeader("vector count must be positive"));
        }
        if n > MAX_N {
            return Err(Error::InvalidHeader("vector count exceeds 250000"));
        }
        if k as u64 > n {
            return Err(Error::InvalidHeader("cap count exceeds vector count"));
        }
        if encoding != ENCODING_F32 {
            return Err(Error::InvalidHeader("unsupported encoding"));
        }
        Ok(Self {
            dim,
            k,
            n,
            seed,
            budget_bytes,
            file_bytes,
            encoding,
            checksum,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub direction_offset: usize,
    pub direction_bytes: usize,
    pub angle_offset: usize,
    pub angle_bytes: usize,
    pub angle_padding: usize,
    pub assignment_offset: usize,
    pub assignment_bytes: usize,
    pub file_bytes: usize,
}

pub fn layout(dim: usize, k: usize, n: usize) -> Result<Layout> {
    let direction_bytes = dim
        .checked_mul(k)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| Error::InvalidData("direction section overflows".into()))?;
    let angle_bytes = k
        .checked_mul(2)
        .ok_or_else(|| Error::InvalidData("angle section overflows".into()))?;
    let angle_padding = (4 - (angle_bytes % 4)) % 4;
    let assignment_bytes = n
        .checked_mul(4)
        .ok_or_else(|| Error::InvalidData("assignment section overflows".into()))?;
    let direction_offset = HEADER_BYTES;
    let angle_offset = direction_offset
        .checked_add(direction_bytes)
        .ok_or_else(|| Error::InvalidData("file layout overflows".into()))?;
    let assignment_offset = angle_offset
        .checked_add(angle_bytes)
        .and_then(|value| value.checked_add(angle_padding))
        .ok_or_else(|| Error::InvalidData("file layout overflows".into()))?;
    let file_bytes = assignment_offset
        .checked_add(assignment_bytes)
        .ok_or_else(|| Error::InvalidData("file layout overflows".into()))?;
    Ok(Layout {
        direction_offset,
        direction_bytes,
        angle_offset,
        angle_bytes,
        angle_padding,
        assignment_offset,
        assignment_bytes,
        file_bytes,
    })
}

pub fn angle_raw(theta: f64) -> u16 {
    let raw = (theta / ANGLE_STEP).ceil();
    if !raw.is_finite() || raw <= 0.0 {
        0
    } else if raw >= 65535.0 {
        65535
    } else {
        raw as u16
    }
}

pub fn angle_decode(raw: u16) -> f64 {
    if raw == 65535 {
        std::f64::consts::PI
    } else {
        raw as f64 * ANGLE_STEP
    }
}

pub fn checksum(bytes: &[u8]) -> u64 {
    xxh3_64(bytes)
}
