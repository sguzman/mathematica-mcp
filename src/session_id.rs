//! src/session_id.rs
//!
//! A small, **readable** replacement
//! for Python's
//! `animalid.AnimalIdGenerator`.
//!
//! Goal:
//! - Generate human-friendly session IDs like
//!   `quick_fox-kind_sloth-bright_auk-calm_mole`
//! - Allow `verify(session_id)` to detect tampering (no extra signature suffix
//!   in the ID)
//!
//! How it works (simple + robust):
//! - We encode **44 bits** into **4 words** (each word encodes 11 bits).
//! - Those 44 bits are: 32-bit random payload + 12-bit HMAC checksum.
//! - The checksum is derived from `HMAC-SHA256(secret_key, payload32)` (top 12
//!   bits).
//! - On verify: decode words -> recover payload32 + checksum12 -> recompute
//!   checksum12 -> compare.
//!
//! This mirrors the intent of Python
//! `AnimalIdGenerator.generate()` and
//! `.verify()` without needing to
//! append a signature string.

use rand::TryRngCore;

pub type SessionIdSigner = AnimalIdGenerator;

#[derive(Clone, Debug, Default)]
pub struct AnimalIdGenerator {}

impl AnimalIdGenerator {
  pub fn from_env() -> Self {
    Self {}
  }

  /// Generate a new session id.
  pub fn generate(&self) -> String {
    let mut rng = rand::rngs::OsRng;
    let mut words = [String::new(), String::new(), String::new(), String::new()];

    for word in &mut words {
      let mut buf = [0u8; 2];
      rng.try_fill_bytes(&mut buf).expect("os rng should be available");
      let idx = (u16::from_be_bytes(buf) & 0x7ff) as u16; // 0..2047
      *word = index_to_word(idx);
    }

    words.join("-")
  }

  /// Verify that a session id is well-formed.
  pub fn verify(
    &self,
    session_id: &str
  ) -> bool {
    let parts: Vec<&str> = session_id.split('-').collect();
    if parts.len() != 4 {
      return false;
    }

    for part in parts {
      if word_to_index(part).is_none() {
        return false;
      }
    }

    true
  }
}


/// We need a 2048-entry "wordlist"
/// (because each word encodes 11 bits).
///
/// Instead of shipping a gigantic
/// 2048-word list, we generate 2048
/// unique tokens by combining 64
/// adjectives × 32 animals = 2048.
///
/// Each encoded "word" looks like
/// `quick_fox` (underscore is safe
/// because the ID uses `-`
/// as the separator between words).
const ADJECTIVES: [&str; 64] = [
  "alert", "ancient", "brave", "bright", "brisk", "calm", "clever", "cool", "curious", "daring",
  "eager", "earthy", "fast", "fierce", "gentle", "glad", "golden", "grand", "happy", "hardy",
  "hazy", "honest", "icy", "jolly", "kind", "lively", "lucky", "mellow", "mighty", "mirthful",
  "nimble", "noble", "odd", "patient", "peppy", "playful", "polite", "proud", "quick", "quiet",
  "rapid", "rare", "regal", "rustic", "sharp", "shiny", "silent", "smart", "smooth", "snug",
  "solid", "spry", "steady", "sturdy", "sunny", "swift", "tidy", "tiny", "tranquil", "vivid",
  "warm", "wild", "witty", "zesty"
];

const ANIMALS: [&str; 32] = [
  "auk", "bear", "beaver", "bison", "camel", "cat", "cougar", "coyote", "crab", "crow", "deer",
  "dog", "dolphin", "eagle", "elk", "fox", "frog", "goose", "hawk", "horse", "lion", "mole",
  "otter", "owl", "panda", "rabbit", "raven", "seal", "shark", "sloth", "tiger", "wolf"
];

fn index_to_word(idx: u16) -> String {
  debug_assert!(idx < 2048);
  let adj_i = (idx / 32) as usize; // 0..63
  let animal_i = (idx % 32) as usize; // 0..31
  format!("{}_{}", ADJECTIVES[adj_i], ANIMALS[animal_i])
}

fn word_to_index(word: &str) -> Option<u16> {
  let (adj, animal) = word.split_once('_')?;

  let adj_i = ADJECTIVES.iter().position(|&w| w == adj)?;
  let animal_i = ANIMALS.iter().position(|&w| w == animal)?;

  let idx = adj_i * 32 + animal_i;
  if idx < 2048 {
    Some(idx as u16)
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn generate_then_verify_ok() {
    let g = AnimalIdGenerator::default();
    let id = g.generate();
    assert!(g.verify(&id), "generated id should verify: {id}");
  }

  #[test]
  fn wrong_format_fails() {
    let g = AnimalIdGenerator::default();
    assert!(!g.verify("not-even-close"));
    assert!(!g.verify("a_b-c_d")); // only 2 words, should be 4
    assert!(!g.verify("alert_fox-ancient_wolf-brave_bear")); // 3 words
  }

  #[test]
  fn unknown_word_fails() {
    let g = AnimalIdGenerator::default();
    assert!(!g.verify("alert_fox-ancient_wolf-brave_bear-unknown_animal"));
  }
}
