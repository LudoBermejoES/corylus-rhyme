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
//!
//! Returns None when the resulting tail has no consonant after the stressed
//! vowel (open-vowel ending) so the caller's min-coda filter is satisfied.

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
fn stressed_vowel_index(chars: &[char]) -> Option<usize> {
    // 1. Explicit accent.
    if let Some(i) = chars.iter().position(|&c| is_accented(c)) {
        return Some(i);
    }

    // Collect vowel indices (treating any vowel as a syllable nucleus — a simplification
    // that ignores diphthongs but is adequate for rhyme-tail extraction). The silent
    // u in gu/qu before e/i is NOT a nucleus and must be excluded from stress counting.
    let vowel_positions: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|&(i, &c)| is_vowel(c) && !is_silent_u(chars, i))
        .map(|(i, _)| i)
        .collect();
    if vowel_positions.is_empty() {
        return None;
    }

    let last = *chars.last().unwrap();
    // 2. Penultimate-syllable stress (llana) when ending in vowel, n, or s.
    let llana = is_vowel(last) || last == 'n' || last == 's';
    if llana && vowel_positions.len() >= 2 {
        return Some(vowel_positions[vowel_positions.len() - 2]);
    }
    // 3. Otherwise final syllable (aguda), or single-vowel word.
    Some(*vowel_positions.last().unwrap())
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

/// Compute the Spanish rhyme key, or None if the tail has no coda consonant.
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

    // Min-coda: require at least one consonant somewhere after the first char
    // (the stressed vowel). A pure open-vowel ending (e.g. "a", "ío"→"io") is
    // rejected to limit false positives.
    let has_coda_consonant = key.chars().skip(1).any(|c| !is_vowel(c));
    if !has_coda_consonant {
        return None;
    }
    Some(key)
}
