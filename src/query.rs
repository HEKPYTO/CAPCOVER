use crate::error::{Error, Result};

pub const EPS: f64 = 1e-12;

pub fn alpha_from_similarity(similarity: f32) -> Result<f64> {
    if !similarity.is_finite() || !(-1.0..=1.0).contains(&similarity) {
        return Err(Error::SimilarityOutOfRange(similarity));
    }
    Ok((similarity as f64).acos())
}
