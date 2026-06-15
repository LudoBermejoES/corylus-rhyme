//! In-memory fst::Map backed by the compiled CMU-derived rhyme data.
//! Loaded once at engine startup, held behind Arc, never re-opened per lookup.
//!
//! Encoding mirrors rust-lemmatization:
//!   - {lang}.rhyme.fst : form (lowercased word) → ordinal (fst::Map value)
//!   - {lang}.rhyme.json : ordinal → rhyme-tail string (ARPAbet phonemes,
//!     space-separated, from the last primary/secondary-stressed vowel to end)

use fst::Map;
use std::path::Path;
use crate::{RhymeError, Result};

pub struct LoadedMap {
    map: Map<Vec<u8>>,
    /// Ordinal → rhyme-tail string, aligned with the fst values.
    tails: Vec<String>,
}

impl LoadedMap {
    pub fn load(fst_path: &Path, tails_path: &Path) -> Result<Self> {
        let fst_bytes = std::fs::read(fst_path)
            .map_err(|e| RhymeError::CorruptMap(format!("read fst: {e}")))?;
        let map = Map::new(fst_bytes)
            .map_err(|e| RhymeError::CorruptMap(format!("parse fst: {e}")))?;

        let tails_raw = std::fs::read_to_string(tails_path)
            .map_err(|e| RhymeError::CorruptMap(format!("read tails: {e}")))?;
        let tails: Vec<String> = serde_json::from_str(&tails_raw)
            .map_err(|e| RhymeError::CorruptMap(format!("parse tails: {e}")))?;

        Ok(Self { map, tails })
    }

    /// Resolve an English surface word to its rhyme key (ARPAbet tail).
    /// Lowercases → fst lookup → tail via ordinal. Returns None when the word is
    /// absent from the map. The min-coda filter is applied by the caller.
    pub fn resolve(&self, surface: &str) -> Option<String> {
        let lower = surface.to_lowercase();
        let ordinal = self.map.get(lower.as_bytes())?;
        self.tails.get(ordinal as usize).cloned()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
