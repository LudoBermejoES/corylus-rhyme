# rust-rhyme

Rhyme-key engine for [Corylus](https://github.com/LudoBermejoES)' accidental-rhyme
detector. Used as a git submodule under `src-tauri/vendor/rust-rhyme`.

A **rhyme key** is the phoneme sequence from the last stressed vowel to the end of
a word; two words sharing a key are a perfect rhyme.

- **English** — keys derived from the [CMU Pronouncing Dictionary][cmu]
  (BSD-2-Clause). The dictionary is compiled into a deterministic FST and
  **downloaded on demand** (SHA-256-pinned `.tar.gz` on this repo's GitHub
  Release), verified, unpacked under the app-data dir, and held in memory —
  mirroring the `rust-lemmatization` engine.
- **Spanish** — keys derived from **orthographic rules** (Spanish is
  near-phonemic). No data file, no download; the Spanish engine is ready
  immediately.

[cmu]: https://github.com/cmusphinx/cmudict

## State machine

`NotInstalled → Downloading → Indexing → Ready → Error` (English).
Spanish is always `Ready`.

## Rebuilding the English artifact

```sh
python3 scripts/build_rhyme.py
# → scripts/artifacts/en.rhyme.tar.gz  (+ prints the SHA-256 to pin)
```

Upload `en.rhyme.tar.gz` to a GitHub Release, then pin its URL + SHA-256 in
`EngineConfig::default_en` (`src/lib.rs`).

## License

This crate's code is dual-licensed under MIT or Apache-2.0. The CMU Pronouncing
Dictionary it derives English data from is BSD-2-Clause, © Carnegie Mellon
University.
