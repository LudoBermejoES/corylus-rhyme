//! English rhyme-key helpers shared between the build pipeline and the runtime.
//!
//! ARPAbet phonemes carry a trailing stress digit on vowels: 0 (unstressed),
//! 1 (primary), 2 (secondary). The rhyme tail is everything from the last
//! primary- or secondary-stressed vowel to the end of the word.

/// ARPAbet vowel phoneme prefixes (without the stress digit).
const ARPABET_VOWELS: &[&str] = &[
    "AA", "AE", "AH", "AO", "AW", "AY", "EH", "ER", "EY", "IH", "IY", "OW", "OY",
    "UH", "UW",
];

fn is_vowel_phoneme(p: &str) -> bool {
    // Strip a trailing stress digit then compare against the vowel set.
    let base: String = p.chars().filter(|c| !c.is_ascii_digit()).collect();
    ARPABET_VOWELS.contains(&base.as_str())
}

fn stress_digit(p: &str) -> Option<char> {
    p.chars().rev().find(|c| c.is_ascii_digit())
}

/// Extract the rhyme tail from a sequence of ARPAbet phonemes.
/// Returns the space-joined phonemes from the last 1/2-stressed vowel to the end,
/// or None if there is no stressed vowel or no coda consonant after it.
pub fn rhyme_tail(phonemes: &[&str]) -> Option<String> {
    // Find the last phoneme that is a vowel with stress 1 or 2.
    let mut start: Option<usize> = None;
    for (i, p) in phonemes.iter().enumerate() {
        if is_vowel_phoneme(p) {
            if let Some(d) = stress_digit(p) {
                if d == '1' || d == '2' {
                    start = Some(i);
                }
            }
        }
    }
    let start = start?;
    let tail = &phonemes[start..];

    // Min-coda: require at least one phoneme after the stressed vowel (a coda).
    if tail.len() < 2 {
        return None;
    }
    // And require that at least one of those trailing phonemes is a consonant.
    let has_consonant = tail[1..].iter().any(|p| !is_vowel_phoneme(p));
    if !has_consonant {
        return None;
    }

    Some(tail.join(" "))
}

/// Parse a CMU-dict pronunciation string (space-separated ARPAbet) into a tail.
/// Convenience wrapper used by the build script's intent and by tests.
pub fn rhyme_tail_from_str(pron: &str) -> Option<String> {
    let phonemes: Vec<&str> = pron.split_whitespace().collect();
    rhyme_tail(&phonemes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn election_tail() {
        // election  IH0 L EH1 K SH AH0 N  → tail from EH1
        assert_eq!(
            rhyme_tail_from_str("IH0 L EH1 K SH AH0 N").as_deref(),
            Some("EH1 K SH AH0 N")
        );
    }

    #[test]
    fn climbing_tail() {
        // climbing  K L AY1 M IH0 NG → tail from AY1
        assert_eq!(
            rhyme_tail_from_str("K L AY1 M IH0 NG").as_deref(),
            Some("AY1 M IH0 NG")
        );
    }

    #[test]
    fn open_vowel_no_coda() {
        // go  G OW1  → no coda consonant after OW1 → None
        assert_eq!(rhyme_tail_from_str("G OW1"), None);
    }

    #[test]
    fn predilection_matches_election() {
        // predilection P R EH2 D IH0 L EH1 K SH AH0 N → tail from EH1 (last 1/2 stress)
        assert_eq!(
            rhyme_tail_from_str("P R EH2 D IH0 L EH1 K SH AH0 N").as_deref(),
            Some("EH1 K SH AH0 N")
        );
    }
}
