#!/usr/bin/env python3
"""Build deterministic rhyme-key FST artifacts from the CMU Pronouncing Dictionary.

Source data (BSD-2-Clause, Carnegie Mellon University):
  en: https://raw.githubusercontent.com/cmusphinx/cmudict/master/cmudict.dict

The CMU dict lists, per line:  word  PH PH PH ...
Alternate pronunciations use a "word(2)" key; we keep only the primary (no-suffix)
pronunciation so each word maps to one rhyme tail (deterministic, and the primary
pronunciation is the most common reading).

Rhyme tail = ARPAbet phonemes from the last primary- or secondary-stressed vowel
(stress digit 1 or 2) to the end of the word. Words whose tail has no coda
consonant after the stressed vowel (open-vowel endings like "go" G OW1) are
dropped — they generate excessive false positives and the runtime filters them
anyway.

Output (English only — Spanish is rule-based, no artifact):
  en.rhyme.fst   (fst 0.4 Map: word → ordinal, byte-sorted keys)
  en.rhyme.json  (ordinal → rhyme-tail string)
  en.rhyme.tar.gz (the two above, for the GitHub Release)

Determinism:
  - words sorted lexicographically (UTF-8 byte order; CMU dict is ASCII).
  - tails assigned ordinals in sorted order.
  - the .fst is built by the build_rhyme_fst Rust binary (this script emits the
    form_index.json it consumes).
"""

import hashlib
import json
import re
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path

# ── Pinned source ──────────────────────────────────────────────────────────────

CMU_URL = "https://raw.githubusercontent.com/cmusphinx/cmudict/master/cmudict.dict"
# Set to a pinned commit SHA-256 of cmudict.dict after first download (the script
# prints it). Leave as the sentinel to accept any download on the first run.
CMU_SHA256 = "81917843c7f44ce2b094ac63873c2c7a4cf802040792c455ba3ca406891c3d22"

FST_FORMAT_VERSION = 1
SCHEMA_VERSION = 1

ARPABET_VOWELS = {
    "AA", "AE", "AH", "AO", "AW", "AY", "EH", "ER", "EY", "IH", "IY", "OW",
    "OY", "UH", "UW",
}


def sha256_of_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def download_verified(url: str, expected_sha: str, dest: Path) -> None:
    if dest.exists():
        actual = sha256_of_file(dest)
        if expected_sha == "TODO_FILL_AFTER_DOWNLOAD" or actual == expected_sha:
            print(f"  cached: {dest.name}")
            if expected_sha == "TODO_FILL_AFTER_DOWNLOAD":
                print(f"  SHA-256: {actual}  ← pin this in CMU_SHA256")
            return
    print(f"  downloading {dest.name} ...")
    urllib.request.urlretrieve(url, dest)
    actual = sha256_of_file(dest)
    if expected_sha != "TODO_FILL_AFTER_DOWNLOAD" and actual != expected_sha:
        dest.unlink()
        raise RuntimeError(f"SHA-256 mismatch for {dest.name}: expected {expected_sha} got {actual}")
    if expected_sha == "TODO_FILL_AFTER_DOWNLOAD":
        print(f"  SHA-256: {actual}  ← pin this in CMU_SHA256")


def base_phoneme(p: str) -> str:
    return re.sub(r"\d", "", p)


def is_vowel(p: str) -> bool:
    return base_phoneme(p) in ARPABET_VOWELS


def stress_digit(p: str):
    m = re.search(r"(\d)", p)
    return m.group(1) if m else None


def rhyme_tail(phonemes: list[str]):
    """ARPAbet tail from the last 1/2-stressed vowel to end; None if no coda."""
    start = None
    for i, p in enumerate(phonemes):
        if is_vowel(p) and stress_digit(p) in ("1", "2"):
            start = i
    if start is None:
        return None
    tail = phonemes[start:]
    if len(tail) < 2:
        return None
    if not any(not is_vowel(p) for p in tail[1:]):
        return None
    return " ".join(tail)


def parse_cmudict(path: Path) -> dict[str, str]:
    """word → rhyme tail (primary pronunciation only)."""
    pairs: dict[str, str] = {}
    with open(path, "r", encoding="latin-1") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith(";;;"):
                continue
            # Strip trailing "# ..." comments present in cmudict.dict.
            line = line.split("#", 1)[0].strip()
            if not line:
                continue
            parts = line.split()
            word = parts[0]
            # Skip alternate-pronunciation variants: "word(2)".
            if word.endswith(")") and "(" in word:
                continue
            phonemes = parts[1:]
            tail = rhyme_tail(phonemes)
            if tail is None:
                continue
            # Keep only "real" words (drop punctuation-only tokens).
            key = word.lower()
            if not re.search(r"[a-z]", key):
                continue
            pairs[key] = tail
    return pairs


def build_for_en(raw_path: Path, out_dir: Path) -> None:
    print("\n=== EN (CMU) ===")
    pairs = parse_cmudict(raw_path)
    sorted_words = sorted(pairs.keys())
    unique_tails_sorted = sorted(set(pairs.values()))
    tail_to_ord = {t: i for i, t in enumerate(unique_tails_sorted)}
    print(f"  words: {len(sorted_words):,}  tails: {len(unique_tails_sorted):,}")

    tails_path = out_dir / "en.rhyme.json"
    tails_path.write_text(
        json.dumps(unique_tails_sorted, ensure_ascii=False, separators=(",", ":")),
        encoding="utf-8",
    )
    print(f"  wrote {tails_path.name} ({tails_path.stat().st_size:,} bytes)")

    form_to_ord = {w: tail_to_ord[pairs[w]] for w in sorted_words}
    index_path = out_dir / "en.form_index.json"
    index_path.write_text(
        json.dumps(form_to_ord, ensure_ascii=False, separators=(",", ":")),
        encoding="utf-8",
    )
    print(f"  wrote {index_path.name} ({index_path.stat().st_size:,} bytes)")

    # Build the FST via the Rust binary.
    fst_path = out_dir / "en.rhyme.fst"
    crate_dir = Path(__file__).resolve().parent.parent
    print("  building FST via build_rhyme_fst ...")
    subprocess.run(
        [
            "cargo", "run", "--quiet", "--manifest-path", str(crate_dir / "Cargo.toml"),
            "--bin", "build_rhyme_fst", "--",
            "en", str(index_path), str(fst_path),
        ],
        check=True,
    )
    print(f"  wrote {fst_path.name} ({fst_path.stat().st_size:,} bytes)")

    # Package the tarball.
    tar_path = out_dir / "en.rhyme.tar.gz"
    with tarfile.open(tar_path, "w:gz") as tar:
        tar.add(fst_path, arcname="en.rhyme.fst")
        tar.add(tails_path, arcname="en.rhyme.json")
    print(f"  wrote {tar_path.name} ({tar_path.stat().st_size:,} bytes)")
    print(f"  tar.gz SHA-256: {sha256_of_file(tar_path)}  ← pin in EngineConfig::default_en")


def main() -> None:
    out_dir = Path(__file__).parent / "artifacts"
    raw_dir = Path(__file__).parent / "raw"
    out_dir.mkdir(exist_ok=True)
    raw_dir.mkdir(exist_ok=True)

    raw_path = raw_dir / "cmudict.dict"
    download_verified(CMU_URL, CMU_SHA256, raw_path)
    build_for_en(raw_path, out_dir)

    print("\nDone. Upload en.rhyme.tar.gz to the corylus-rhyme Release, then pin")
    print("the SHA-256 above in EngineConfig::default_en.")


if __name__ == "__main__":
    main()
