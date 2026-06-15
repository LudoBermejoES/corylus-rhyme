//! Unit tests for the rhyme engine.

use super::*;
use tempfile::TempDir;

// ── Spanish orthographic rules ────────────────────────────────────────────────

#[test]
fn spanish_accented_aguda() {
    assert_eq!(spanish_rhyme_key("jardín").as_deref(), Some("in"));
    assert_eq!(spanish_rhyme_key("bergantín").as_deref(), Some("in"));
}

#[test]
fn spanish_penultimate_llana() {
    assert_eq!(spanish_rhyme_key("ventana").as_deref(), Some("ana"));
    assert_eq!(spanish_rhyme_key("mañana").as_deref(), Some("ana"));
}

#[test]
fn spanish_b_equals_v() {
    // tubo / tuvo → ubo (penultimate stress, v→b)
    assert_eq!(spanish_rhyme_key("tubo"), spanish_rhyme_key("tuvo"));
    assert_eq!(spanish_rhyme_key("tubo").as_deref(), Some("ubo"));
}

#[test]
fn spanish_silent_h() {
    // "ahora" → stressed penultimate 'o', tail "ora" (h stripped already absent here)
    // "almohada" → penultimate 'a', tail "ada" with the h before it irrelevant.
    // Direct check: "búho" → aguda? ends in vowel → llana, penult vowel is ú → "uo"?
    // Use a clearer case: "zanahoria" penult 'o' → tail "oria".
    assert_eq!(spanish_rhyme_key("hola").as_deref(), Some("ola"));
}

#[test]
fn spanish_silent_u_in_gue() {
    // "merengue" → penult 'e', tail "engue" → u after g before e is silent → "enge"
    assert_eq!(spanish_rhyme_key("merengue").as_deref(), Some("enge"));
}

#[test]
fn spanish_esdrujula() {
    // rápido → explicit accent on á → tail "apido"
    assert_eq!(spanish_rhyme_key("rápido").as_deref(), Some("apido"));
}

#[test]
fn spanish_seseo_z_and_c() {
    // "caza" (llana, penult 'a') → "asa" ; "casa" → "asa" → rhyme under seseo
    assert_eq!(spanish_rhyme_key("caza"), spanish_rhyme_key("casa"));
}

#[test]
fn spanish_open_vowel_rejected() {
    // "café" → aguda, tail "e" → no coda → None
    assert_eq!(spanish_rhyme_key("café"), None);
    // single vowel
    assert_eq!(spanish_rhyme_key("a"), None);
}

// ── English: fixture map resolution ───────────────────────────────────────────

/// Build a minimal English engine with a temp FST + tails + version file.
fn make_en_engine(pairs: &[(&str, &str)]) -> (TempDir, RhymeEngine) {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();

    // Assign ordinals to unique tails (sorted, deterministic).
    let mut unique_tails: Vec<&str> = pairs.iter().map(|(_, t)| *t).collect();
    unique_tails.sort_unstable();
    unique_tails.dedup();
    let tail_to_ord: std::collections::BTreeMap<&str, u64> =
        unique_tails.iter().enumerate().map(|(i, &t)| (t, i as u64)).collect();

    let mut form_index: std::collections::BTreeMap<String, u64> = Default::default();
    for (word, tail) in pairs {
        form_index.insert(word.to_lowercase(), tail_to_ord[*tail]);
    }

    let fst_path = data_dir.join("en.rhyme.fst");
    {
        use fst::MapBuilder;
        use std::io::BufWriter;
        let f = std::fs::File::create(&fst_path).unwrap();
        let mut builder = MapBuilder::new(BufWriter::new(f)).unwrap();
        for (form, ord) in &form_index {
            builder.insert(form.as_bytes(), *ord).unwrap();
        }
        builder.finish().unwrap();
    }

    let tails_path = data_dir.join("en.rhyme.json");
    std::fs::write(&tails_path, serde_json::to_string(&unique_tails).unwrap()).unwrap();

    let ver_path = data_dir.join("en.rhyme.version.json");
    let ver = state::VersionFile {
        lang: "en".to_string(),
        source_sha256: String::new(),
        schema_version: state::SCHEMA_VERSION,
        fst_format_version: state::FST_FORMAT_VERSION,
    };
    std::fs::write(&ver_path, serde_json::to_string(&ver).unwrap()).unwrap();

    let mut config = EngineConfig::default_en(data_dir);
    config.source_sha256 = String::new(); // match the version file so is_installed_for passes
    let engine = RhymeEngine::new(config);
    (dir, engine)
}

#[test]
fn english_resolves_installed_word() {
    let (_d, engine) = make_en_engine(&[
        ("election", "EH1 K SH AH0 N"),
        ("predilection", "EH1 K SH AH0 N"),
    ]);
    assert!(engine.is_ready());
    assert_eq!(engine.resolve("election").as_deref(), Some("EH1 K SH AH0 N"));
    // Same key → these two rhyme.
    assert_eq!(engine.resolve("election"), engine.resolve("predilection"));
}

#[test]
fn english_unknown_word_none() {
    let (_d, engine) = make_en_engine(&[("election", "EH1 K SH AH0 N")]);
    assert_eq!(engine.resolve("zzzznotaword"), None);
}

#[test]
fn english_not_installed_resolves_none() {
    let dir = tempfile::tempdir().unwrap();
    let engine = RhymeEngine::new(EngineConfig::default_en(dir.path().to_path_buf()));
    assert_eq!(engine.state(), RhymeState::NotInstalled);
    assert_eq!(engine.resolve("election"), None);
}

#[test]
fn english_batch() {
    let (_d, engine) = make_en_engine(&[
        ("election", "EH1 K SH AH0 N"),
        ("cat", "AE1 T"),
    ]);
    let out = engine.resolve_batch(&[
        "election".to_string(),
        "unknown".to_string(),
        "cat".to_string(),
    ]);
    assert_eq!(out, vec![
        Some("EH1 K SH AH0 N".to_string()),
        None,
        Some("AE1 T".to_string()),
    ]);
}

// ── Spanish engine via the public API (rule-based, no install) ─────────────────

#[test]
fn spanish_engine_ready_without_install() {
    let dir = tempfile::tempdir().unwrap();
    let engine = RhymeEngine::new(EngineConfig::default_es(dir.path().to_path_buf()));
    assert!(engine.is_ready());
    assert!(engine.is_installed());
    assert_eq!(engine.resolve("jardín").as_deref(), Some("in"));
    let batch = engine.resolve_batch(&["jardín".into(), "bergantín".into()]);
    assert_eq!(batch[0], batch[1]); // they rhyme
}

#[test]
fn spanish_uninstall_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let engine = RhymeEngine::new(EngineConfig::default_es(dir.path().to_path_buf()));
    assert!(engine.uninstall().is_ok());
    assert!(engine.is_ready());
}
