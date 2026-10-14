//! Readable, HMAC-signed session IDs.
//!
//! Four adjective_animal tokens give 44 bits of entropy. A 128-bit HMAC
//! tag prevents an arbitrary, well-formed string from passing verification.
//! The secret changes when the server restarts; sessions are not persistent.

use hmac::{Hmac, Mac};
use rand::TryRngCore;
use sha2::Sha256;

pub type SessionIdSigner = AnimalIdGenerator;

#[derive(Clone)]
pub struct AnimalIdGenerator {
  secret: [u8; 32]
}

impl Default for AnimalIdGenerator {
  fn default() -> Self {
    let mut secret = [0u8; 32];
    let mut rng = rand::rngs::OsRng;
    rng.try_fill_bytes(&mut secret).expect("OS randomness unavailable");
    Self { secret }
  }
}

impl AnimalIdGenerator {
  pub fn from_env() -> Self {
    Self::default()
  }

  pub fn generate(&self) -> String {
    let mut rng = rand::rngs::OsRng;
    let mut words = [String::new(), String::new(), String::new(), String::new()];
    for word in &mut words {
      let mut buf = [0u8; 2];
      rng.try_fill_bytes(&mut buf).expect("OS randomness unavailable");
      let idx = u16::from_be_bytes(buf) & 0x07ff;
      *word = index_to_word(idx);
    }
    let readable = words.join("-");
    format!("{}-{}", readable, self.sign(&readable))
  }

  pub fn verify(&self, session_id: &str) -> bool {
    let Some((readable, tag)) = session_id.rsplit_once('-') else {
      return false;
    };
    let words: Vec<&str> = readable.split('-').collect();
    if words.len() != 4 || words.iter().any(|word| word_to_index(word).is_none()) {
      return false;
    }
    if tag.len() != 32 || !tag.bytes().all(|b| b.is_ascii_hexdigit()) {
      return false;
    }

    let expected = self.sign(readable);
    // Constant-time comparison over the fixed-size tag.
    let mut difference = 0u8;
    for (a, b) in expected.bytes().zip(tag.bytes()) {
      difference |= a ^ b.to_ascii_lowercase();
    }
    difference == 0
  }

  fn sign(&self, readable: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret).expect("valid HMAC key");
    mac.update(readable.as_bytes());
    let digest = mac.finalize().into_bytes();
    digest[..16].iter().map(|byte| format!("{byte:02x}")).collect()
  }
}

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
  format!("{}_{}", ADJECTIVES[(idx / 32) as usize], ANIMALS[(idx % 32) as usize])
}

fn word_to_index(word: &str) -> Option<u16> {
  let (adj, animal) = word.split_once('_')?;
  let adj_i = ADJECTIVES.iter().position(|&w| w == adj)?;
  let animal_i = ANIMALS.iter().position(|&w| w == animal)?;
  Some((adj_i * 32 + animal_i) as u16)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn generated_id_is_signed_by_this_server_only() {
    let issuer = AnimalIdGenerator::default();
    let different_issuer = AnimalIdGenerator::default();
    let id = issuer.generate();
    assert!(issuer.verify(&id));
    assert!(!different_issuer.verify(&id));
    assert_eq!(id.split('-').count(), 5);
  }

  #[test]
  fn modified_or_malformed_id_rejected() {
    let issuer = AnimalIdGenerator::default();
    let original = issuer.generate();
    assert!(!issuer.verify("not-even-close"));
    assert!(!issuer.verify("alert_fox-ancient_wolf-brave_bear-calm_cat"));
    let (words, tag) = original.rsplit_once('-').unwrap();
    let mut changed = tag.as_bytes().to_vec();
    changed[0] = if changed[0] == b'0' { b'1' } else { b'0' };
    assert!(!issuer.verify(&format!("{words}-{}", String::from_utf8(changed).unwrap())));
    assert!(!issuer.verify(&format!("unknown_animal-{words}-{tag}")));
  }
}
