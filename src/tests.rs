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

// ── Spanish assonant key ──────────────────────────────────────────────────────

#[test]
fn spanish_assonant_casa_manana_match() {
    // Both end in 'a-a' vowel pattern → same assonant key; consonant keys differ.
    let casa = spanish_assonant_key("casa");
    let manana = spanish_assonant_key("mañana");
    assert_eq!(casa, manana);
    assert!(casa.is_some());
    // Consonant keys must differ ("asa" vs "ana").
    assert_ne!(spanish_rhyme_key("casa"), spanish_rhyme_key("mañana"));
}

#[test]
fn spanish_assonant_esdrujula_all_vowels() {
    // pájaro: á is the stressed vowel → tail "ajaro" → vowels "aao"
    // sábado: á stressed → tail "abado" → vowels "aao"
    let pajaro = spanish_assonant_key("pájaro");
    let sabado = spanish_assonant_key("sábado");
    assert_eq!(pajaro, sabado);
    assert_eq!(pajaro.as_deref(), Some("aao"));
}

#[test]
fn spanish_assonant_open_vowel_has_key() {
    // "río" ends in open vowel — consonant key is None, assonant key must be Some.
    assert_eq!(spanish_rhyme_key("río"), None);
    assert!(spanish_assonant_key("río").is_some());
    // "camino" (llana) → stressed penult 'i' → tail "ino" → vowels "io"
    // "sombrío" (aguda stressed í) → tail "ío" → vowels "io" → match
    let camino = spanish_assonant_key("camino");
    let sombrio = spanish_assonant_key("sombrío");
    assert_eq!(camino, sombrio);
}

#[test]
fn spanish_assonant_amor_reloj() {
    // "amor" (aguda, no explicit accent) → stressed 'o' → tail "or" → vowels "o"
    // "reloj" aguda → stressed 'o' → tail "oj" → vowels "o"
    let amor = spanish_assonant_key("amor");
    let reloj = spanish_assonant_key("reloj");
    assert_eq!(amor, reloj);
    assert_eq!(amor.as_deref(), Some("o"));
    // Their consonant keys differ: "or" vs "oj"
    assert_ne!(spanish_rhyme_key("amor"), spanish_rhyme_key("reloj"));
}

#[test]
fn spanish_assonant_jardin_different_from_casa() {
    // "jardín" → "in"; "casa" → "aa" — different assonant keys
    assert_ne!(spanish_assonant_key("jardín"), spanish_assonant_key("casa"));
}

// ── English assonant key ─────────────────────────────────────────────────────

#[test]
fn english_assonant_election_affection() {
    // election:    EH1 K SH AH0 N → tail "EH1 K SH AH0 N" → vowels "EH AH"
    // affection:   AH0 F EH1 K SH AH0 N → tail from EH1 → "EH1 K SH AH0 N" → "EH AH"
    use crate::english::assonant_tail_from_str;
    let el = assonant_tail_from_str("IH0 L EH1 K SH AH0 N");
    let af = assonant_tail_from_str("AH0 F EH1 K SH AH0 N");
    assert_eq!(el, af);
    assert_eq!(el.as_deref(), Some("EH AH"));
}

#[test]
fn english_assonant_open_vowel() {
    // "go" G OW1 → no coda → rhyme_tail returns None, but assonant_tail returns "OW"
    use crate::english::{rhyme_tail_from_str, assonant_tail_from_str};
    assert_eq!(rhyme_tail_from_str("G OW1"), None);
    assert_eq!(assonant_tail_from_str("G OW1").as_deref(), Some("OW"));
}

// ── resolve_batch_both (engine API) ───────────────────────────────────────────

#[test]
fn resolve_batch_both_spanish() {
    let dir = tempfile::tempdir().unwrap();
    let engine = RhymeEngine::new(EngineConfig::default_es(dir.path().to_path_buf()));
    let results = engine.resolve_batch_both(&["casa".into(), "mañana".into(), "jardín".into()]);
    assert_eq!(results.len(), 3);
    // casa and mañana share assonant key but differ in consonant key
    assert_eq!(results[0].assonant, results[1].assonant);
    assert_ne!(results[0].consonant, results[1].consonant);
    // jardín has both consonant and assonant keys
    assert!(results[2].consonant.is_some());
    assert!(results[2].assonant.is_some());
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
