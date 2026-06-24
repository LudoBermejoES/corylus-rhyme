//! Spanish rhyme-key derivation by orthographic rules (no data file).
//!
//! Spanish is near-phonemic, so the rhyme tail is derivable from the spelling:
//!   1. Locate the stressed vowel:
//!        - explicit accent mark (á é í ó ú) wins; else
//!        - penultimate syllable if the word ends in a vowel, n, or s; else
//!        - final syllable.
//!   2. Extract the suffix from the stressed vowel to the end of the word.
//!   3. Strip accent marks (á→a, é→e, í→i, ó→o, ú→u, ü→u).
//!   4. Normalise silent / equivalent letters:
//!        - b = v (phonemically identical)
//!        - h is silent (strip)
//!        - silent u in gu/qu before e/i (strip the u)
//!        - seseo: z = s, and c before e/i = s
//!   5. The resulting string is the rhyme key.

const VOWELS: &[char] = &['a', 'e', 'i', 'o', 'u', 'á', 'é', 'í', 'ó', 'ú', 'ü'];
const ACCENTED: &[char] = &['á', 'é', 'í', 'ó', 'ú'];

fn is_vowel(c: char) -> bool {
    VOWELS.contains(&c)
}

fn is_accented(c: char) -> bool {
    ACCENTED.contains(&c)
}

/// The 'u' at index `i` is silent when it follows g/q and precedes e/i (gue, gui,
/// que, qui) — it is not a syllable nucleus and does not bear stress.
fn is_silent_u(chars: &[char], i: usize) -> bool {
    if chars[i] != 'u' {
        return false;
    }
    let prev = if i > 0 { chars[i - 1] } else { return false };
    if prev != 'g' && prev != 'q' {
        return false;
    }
    matches!(chars.get(i + 1), Some('e') | Some('i'))
}

fn strip_accent(c: char) -> char {
    match c {
        'á' => 'a',
        'é' => 'e',
        'í' => 'i',
        'ó' => 'o',
        'ú' | 'ü' => 'u',
        other => other,
    }
}

/// Index (in the `chars` vec) of the stressed vowel.
///
/// For syllable counting we treat adjacent vowel pairs that form a diphthong
/// as a single nucleus. A diphthong is: two adjacent vowels where at least one
/// is a weak/closed vowel (unaccented i/u) and the other is strong (a/e/o) or
/// the pair is two weak vowels — and NEITHER carries an accent mark (an accent
/// would mark a hiato, not a diphthong). When we find such a pair, we use the
/// index of the STRONG (or last, for all-weak) vowel as the nucleus index.
fn stressed_vowel_index(chars: &[char]) -> Option<usize> {
    // 1. Explicit accent.
    if let Some(i) = chars.iter().position(|&c| is_accented(c)) {
        return Some(i);
    }

    // Build syllable-nucleus positions, collapsing unaccented diphthong pairs
    // into a single entry (the strong vowel's index).
    // We scan left-to-right; when we see two adjacent unaccented vowels where
    // at least one is weak (i/u), merge them into the strong one's index.
    let raw_vpos: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|&(i, &c)| is_vowel(c) && !is_silent_u(chars, i))
        .map(|(i, _)| i)
        .collect();

    if raw_vpos.is_empty() {
        return None;
    }

    // Merge adjacent diphthong pairs into single nuclei.
    let mut nuclei: Vec<usize> = Vec::new();
    let mut skip_next = false;
    for w_idx in 0..raw_vpos.len() {
        if skip_next {
            skip_next = false;
            continue;
        }
        let cur_pos = raw_vpos[w_idx];
        // Check if this and the next vowel form a diphthong.
        if let Some(&next_pos) = raw_vpos.get(w_idx + 1) {
            if next_pos == cur_pos + 1 {
                let cur_c = chars[cur_pos];
                let next_c = chars[next_pos];
                // Neither is accented (would be hiato) and at least one is weak.
                let cur_weak = matches!(cur_c, 'i' | 'u');
                let next_weak = matches!(next_c, 'i' | 'u');
                if !is_accented(cur_c) && !is_accented(next_c) && (cur_weak || next_weak) {
                    // Diphthong: nucleus is the strong vowel's index.
                    let nucleus = if !cur_weak {
                        cur_pos // cur is strong
                    } else if !next_weak {
                        next_pos // next is strong
                    } else {
                        next_pos // all-weak: last one
                    };
                    nuclei.push(nucleus);
                    skip_next = true;
                    continue;
                }
            }
        }
        nuclei.push(cur_pos);
    }

    let last = *chars.last().unwrap();
    // 2. Penultimate-syllable stress (llana) when ending in vowel, n, or s.
    let llana = is_vowel(last) || last == 'n' || last == 's';
    if llana && nuclei.len() >= 2 {
        return Some(nuclei[nuclei.len() - 2]);
    }
    // 3. Otherwise final syllable (aguda), or single-vowel word.
    Some(*nuclei.last().unwrap())
}

/// Apply phonemic normalisation to the already-lowercased, accent-stripped tail.
fn normalise(tail: &[char]) -> String {
    let mut out = String::with_capacity(tail.len());
    let mut i = 0;
    while i < tail.len() {
        let c = tail[i];
        let next = tail.get(i + 1).copied();
        match c {
            // h is silent.
            'h' => {}
            // b and v are the same phoneme; normalise both to 'b'.
            'v' => out.push('b'),
            // z → s (seseo).
            'z' => out.push('s'),
            // c: before e/i → s (seseo); 'qu'/'gu' u handled below; otherwise k-sound 'c'.
            'c' => {
                if matches!(next, Some('e') | Some('i')) {
                    out.push('s');
                } else {
                    out.push('c');
                }
            }
            // gu / qu before e or i: the u is silent — emit the consonant, skip the u.
            'g' | 'q' => {
                out.push(c);
                if next == Some('u') {
                    if let Some(after) = tail.get(i + 2).copied() {
                        if after == 'e' || after == 'i' {
                            i += 1; // skip the silent u
                        }
                    }
                }
            }
            other => out.push(other),
        }
        i += 1;
    }
    out
}

/// Partition the vowels in `orig_tail` into syllable-nucleus groups.
///
/// A "diphthong run" = a maximal sequence of adjacent vowels in the original
/// word where:
///   - they are positionally adjacent (no consonant between them), AND
///   - NO vowel in the run carries an accent mark (an accent mark signals a
///     hiato — each accented vowel is its own syllable nucleus).
///
/// Runs where any vowel is accented are split at the accent boundary: each
/// accented vowel starts a new singleton group, and the following unaccented
/// vowel also starts fresh. This correctly handles `sombrío` (í,o → two
/// separate groups) while collapsing `reino` (e,i → one group, keeps `e`).
fn vowel_runs(orig_tail: &[char]) -> Vec<Vec<(char, char)>> {
    // Collect (index_in_orig, orig, stripped) for every non-silent-u vowel.
    let tail_stripped: Vec<char> = orig_tail.iter().map(|&c| strip_accent(c)).collect();
    let vpos: Vec<(usize, char, char)> = orig_tail
        .iter()
        .enumerate()
        .filter_map(|(i, &orig)| {
            let stripped = strip_accent(orig);
            if is_vowel(stripped) && !is_silent_u(&tail_stripped, i) {
                Some((i, orig, stripped))
            } else {
                None
            }
        })
        .collect();

    if vpos.is_empty() {
        return vec![];
    }

    let mut runs: Vec<Vec<(char, char)>> = Vec::new();
    let mut run: Vec<(char, char)> = vec![(vpos[0].1, vpos[0].2)];

    for w in vpos.windows(2) {
        let (prev_idx, prev_orig, _) = w[0];
        let (cur_idx, cur_orig, cur_stripped) = w[1];

        // A run boundary occurs when:
        //  (a) the two vowels are not adjacent in the original word, OR
        //  (b) the previous vowel is accented (hiato marker — it is its own nucleus), OR
        //  (c) the current vowel is accented (it starts its own syllable).
        let new_run = cur_idx != prev_idx + 1
            || is_accented(prev_orig)
            || is_accented(cur_orig);

        if new_run {
            runs.push(run.clone());
            run = vec![(cur_orig, cur_stripped)];
        } else {
            run.push((cur_orig, cur_stripped));
        }
    }
    runs.push(run);
    runs
}

/// Reduce a vowel sequence for the assonant key per *Métrica Castellana* §2.1:
///
/// Step 1 — diphthong reduction: for each adjacent-vowel run (a diphthong or
/// triphthong within one syllable), keep only the strong/open vowel (a/e/o) or
/// the accented vowel. If the run is all-weak (ui/iu), keep the last one.
/// Runs split by accent marks (hiatos) are kept as separate vowels.
///
/// Step 2 — esdrújula reduction: if the resulting vowel sequence has more than
/// two elements, keep only the first (stressed) and the last (final).
fn reduce_assonant_vowels(orig_tail: &[char]) -> Vec<char> {
    let runs = vowel_runs(orig_tail);
    if runs.is_empty() {
        return vec![];
    }

    // Step 1: collapse each run to its nucleus.
    let mut reduced: Vec<char> = Vec::new();
    for run in &runs {
        if run.len() == 1 {
            reduced.push(run[0].1);
        } else {
            // Diphthong run: find the dominant (strong) vowel.
            // Strong = a/e/o (open); accented weak (í/ú) is handled by the
            // hiato split above, so runs here never contain accented vowels.
            let strong: Vec<char> = run.iter().filter_map(|&(_orig, stripped)| {
                if matches!(stripped, 'a' | 'e' | 'o') { Some(stripped) } else { None }
            }).collect();
            if !strong.is_empty() {
                // Keep the first strong vowel — it is the syllable nucleus.
                reduced.push(strong[0]);
            } else {
                // All-weak run (ui/iu): keep the last (Spanish convention).
                reduced.push(run.last().unwrap().1);
            }
        }
    }

    // Step 2: esdrújula — more than two vowels → first + last.
    if reduced.len() > 2 {
        let first = reduced[0];
        let last = *reduced.last().unwrap();
        vec![first, last]
    } else {
        reduced
    }
}

/// Compute the Spanish assonant rhyme key: the vowels only from the stressed
/// vowel to the end of the word, with diphthong and esdrújula reductions per
/// *Métrica Castellana* §2.1. Returns None only when there is no stressed vowel.
pub fn spanish_assonant_key(word: &str) -> Option<String> {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let idx = stressed_vowel_index(&chars)?;

    // Keep the original tail (with accents) for nucleus detection.
    let orig_tail: Vec<char> = chars[idx..].to_vec();

    let reduced = reduce_assonant_vowels(&orig_tail);
    if reduced.is_empty() {
        return None;
    }
    Some(reduced.into_iter().collect())
}

/// Compute the Spanish rhyme key (consonante). Returns None only for a single
/// bare stressed vowel (min-tail filter applied by the caller). Vowel-final
/// words like "casa" (→ "asa") are valid consonant rhyme candidates per
/// *Métrica Castellana* §2.1.
pub fn spanish_rhyme_key(word: &str) -> Option<String> {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let idx = stressed_vowel_index(&chars)?;

    // Suffix from the stressed vowel to the end, accents stripped.
    let tail: Vec<char> = chars[idx..].iter().map(|&c| strip_accent(c)).collect();

    let key = normalise(&tail);

    // A bare single-vowel key (e.g. "a", "e") is excluded — the min-tail
    // filter prevents every open-monosyllable from rhyming with everything.
    if key.len() <= 1 {
        return None;
    }
    Some(key)
}
