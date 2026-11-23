use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidHeader(&'static str),
    InvalidData(String),
    BudgetTooSmall { budget: u64, minimum: u64 },
    SimilarityOutOfRange(f32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "io error: {error}"),
            Error::InvalidHeader(reason) => write!(f, "invalid index header: {reason}"),
            Error::InvalidData(reason) => write!(f, "invalid data: {reason}"),
            Error::BudgetTooSmall { budget, minimum } => {
                write!(
                    f,
                    "budget {budget} bytes is below the minimum {minimum} bytes"
                )
            }
            Error::SimilarityOutOfRange(value) => {
                write!(f, "similarity {value} is outside [-1, 1]")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
