use thiserror::Error;

#[derive(Debug, Error)]
pub enum RhymeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("FST error: {0}")]
    Fst(String),
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("Corrupt map: {0}")]
    CorruptMap(String),
    #[error("Not ready")]
    NotReady,
}

impl From<fst::Error> for RhymeError {
    fn from(e: fst::Error) -> Self {
        RhymeError::Fst(e.to_string())
    }
}
