pub mod error;
pub mod fixture;
pub mod format;
pub mod rng;
pub mod vector;

pub use error::{Error, Result};
pub use fixture::{generate, Fixture, FixtureKind};
pub use vector::Vectors;
