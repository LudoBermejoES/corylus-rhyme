//! Rhyme-key engine for Corylus' accidental-rhyme detector.
//!
//! Mirrors the rust-lemmatization architecture:
//!   - English: a CMU-Pronouncing-Dictionary-derived FST downloaded on demand
//!     (SHA-256-pinned .tar.gz on the corylus-rhyme GitHub Release), verified,
//!     unpacked under the app-data dir, held in memory.
//!   - Spanish: orthographic rules compiled into the crate — no data file, no
//!     download. The Spanish engine reports Ready immediately.
//!
//! A "rhyme key" is the phoneme sequence from the last stressed vowel to the end
//! of the word: ARPAbet for English, a normalised orthographic suffix for
//! Spanish. Two words with the same key are a perfect rhyme.

mod english;
mod error;
mod map;
mod provision;
mod spanish;
mod state;

#[cfg(test)]
mod tests;

pub use english::{rhyme_tail, rhyme_tail_from_str, assonant_tail, assonant_tail_from_str};
pub use error::RhymeError;
pub use spanish::{spanish_rhyme_key, spanish_assonant_key};

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type Result<T> = std::result::Result<T, RhymeError>;

/// Configuration for one language's rhyme engine.
#[derive(Clone)]
pub struct EngineConfig {
    /// Directory where {lang}.rhyme.fst, {lang}.rhyme.json, {lang}.rhyme.version.json live.
    pub data_dir: PathBuf,
    /// Language code: "en" or "es".
    pub lang: String,
    /// URL of the pinned gzipped tar artifact (English only). Empty for Spanish.
    pub source_url: String,
    /// Pinned SHA-256 hex string of the artifact (English only). Empty for Spanish.
    pub source_sha256: String,
}

impl EngineConfig {
    pub fn default_en(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            lang: "en".into(),
            source_url: "https://github.com/LudoBermejoES/corylus-rhyme/releases/download/v1.0.0/en.rhyme.tar.gz".into(),
            source_sha256: "a7428dea259ded769524717a5402b2d0f3643d4595c82fa34b8730ab3bdd6daf".into(),
        }
    }

    pub fn default_es(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            lang: "es".into(),
            source_url: String::new(),
            source_sha256: String::new(),
        }
    }
}

/// Observable state of the rhyme engine for one language.
#[derive(Clone, Debug, PartialEq)]
pub enum RhymeState {
    NotInstalled,
    Downloading { downloaded: u64, total: Option<u64> },
    Indexing,
    Ready,
    Error { message: String },
}

pub(crate) struct Inner {
    pub config: EngineConfig,
    pub state: RhymeState,
    /// The in-memory English map; None for Spanish (rule-based) or until Ready.
    pub loaded_map: Option<Arc<map::LoadedMap>>,
}

/// Both rhyme keys for a single token.
/// `consonant` is None for open-vowel endings (no coda consonant).
/// `assonant` is None only when no stressed vowel can be found.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RhymeKeys {
    pub consonant: Option<String>,
    pub assonant: Option<String>,
}

/// True for languages whose rhyme keys are computed from rules with no data file.
fn is_rulebased(lang: &str) -> bool {
    lang == "es"
}

/// Per-language rhyme engine. One instance per language. Cheap to clone (Arc).
#[derive(Clone)]
pub struct RhymeEngine {
    pub(crate) inner: Arc<Mutex<Inner>>,
}

impl RhymeEngine {
    pub fn new(config: EngineConfig) -> Self {
        // Rule-based languages (Spanish) are Ready with no install; data-backed
        // languages (English) start NotInstalled and upgrade to Ready after load.
        let initial_state = if is_rulebased(&config.lang) {
            RhymeState::Ready
        } else {
            RhymeState::NotInstalled
        };
        let rulebased = is_rulebased(&config.lang);
        let engine = Self {
            inner: Arc::new(Mutex::new(Inner {
                config,
                state: initial_state,
                loaded_map: None,
            })),
        };
        if !rulebased && state::is_installed_for(&engine.inner.lock().unwrap().config) {
            let _ = provision::try_load_map(engine.inner.clone());
        }
        engine
    }

    pub fn data_dir(&self) -> PathBuf {
        self.inner.lock().unwrap().config.data_dir.clone()
    }

    pub fn set_data_dir(&self, data_dir: PathBuf) {
        let rulebased = {
            let mut inner = self.inner.lock().unwrap();
            inner.config.data_dir = data_dir;
            inner.loaded_map = None;
            let rb = is_rulebased(&inner.config.lang);
            inner.state = if rb { RhymeState::Ready } else { RhymeState::NotInstalled };
            rb
        };
        if !rulebased && state::is_installed_for(&self.inner.lock().unwrap().config) {
            let _ = provision::try_load_map(self.inner.clone());
        }
    }

    pub fn state(&self) -> RhymeState {
        self.inner.lock().unwrap().state.clone()
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state(), RhymeState::Ready)
    }

    pub fn is_installed(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if is_rulebased(&inner.config.lang) {
            return true;
        }
        state::is_installed_for(&inner.config)
    }

    /// Download → verify → unpack → load. No-op for rule-based languages.
    pub async fn provision(
        &self,
        on_progress: impl Fn(RhymeState) + Send + 'static,
    ) -> Result<()> {
        let rulebased = is_rulebased(&self.inner.lock().unwrap().config.lang);
        if rulebased {
            on_progress(RhymeState::Ready);
            return Ok(());
        }
        provision::run(self.inner.clone(), on_progress).await
    }

    /// Resolve a single surface word to its rhyme key, or None.
    /// English: installed CMU map lookup (+ min-coda filter from the build).
    /// Spanish: orthographic rules. Returns None when not installed or no key.
    pub fn resolve(&self, surface: &str) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        let lang = inner.config.lang.clone();
        if is_rulebased(&lang) {
            drop(inner);
            return spanish::spanish_rhyme_key(surface);
        }
        let map_arc = inner.loaded_map.clone();
        drop(inner);
        map_arc.and_then(|m| m.resolve(surface))
    }

    /// Resolve both rhyme keys (consonant and assonant) for a single surface word.
    pub fn resolve_both(&self, surface: &str) -> RhymeKeys {
        let results = self.resolve_batch_both(&[surface.to_string()]);
        results.into_iter().next().unwrap_or(RhymeKeys { consonant: None, assonant: None })
    }

    /// Resolve both keys for a batch of surface words in order.
    pub fn resolve_batch_both(&self, tokens: &[String]) -> Vec<RhymeKeys> {
        let inner = self.inner.lock().unwrap();
        let lang = inner.config.lang.clone();
        if is_rulebased(&lang) {
            drop(inner);
            return tokens
                .iter()
                .map(|t| RhymeKeys {
                    consonant: spanish::spanish_rhyme_key(t),
                    assonant: spanish::spanish_assonant_key(t),
                })
                .collect();
        }
        let map_arc = inner.loaded_map.clone();
        drop(inner);
        match map_arc {
            Some(m) => tokens
                .iter()
                .map(|t| {
                    let consonant = m.resolve(t);
                    let assonant = consonant.as_deref().map(|cons_key| {
                        let phonemes: Vec<&str> = cons_key.split_whitespace().collect();
                        english::assonant_tail(&phonemes).unwrap_or_default()
                    });
                    RhymeKeys { consonant, assonant }
                })
                .collect(),
            None => tokens.iter().map(|_| RhymeKeys { consonant: None, assonant: None }).collect(),
        }
    }

    /// Resolve a batch of surface words in order.
    pub fn resolve_batch(&self, tokens: &[String]) -> Vec<Option<String>> {
        let inner = self.inner.lock().unwrap();
        let lang = inner.config.lang.clone();
        if is_rulebased(&lang) {
            drop(inner);
            return tokens.iter().map(|t| spanish::spanish_rhyme_key(t)).collect();
        }
        let map_arc = inner.loaded_map.clone();
        drop(inner);
        match map_arc {
            Some(m) => tokens.iter().map(|t| m.resolve(t)).collect(),
            None => tokens.iter().map(|_| None).collect(),
        }
    }

    /// Remove installed data and reset to NotInstalled. No-op for rule-based langs.
    pub fn uninstall(&self) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if is_rulebased(&inner.config.lang) {
            return Ok(());
        }
        let config = &inner.config;
        for path in [
            state::fst_path(config),
            state::tails_path(config),
            state::version_path(config),
        ] {
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
        inner.loaded_map = None;
        inner.state = RhymeState::NotInstalled;
        Ok(())
    }
}
