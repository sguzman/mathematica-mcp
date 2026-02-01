//! src/session_id.rs
//!
//! A small, **readable** replacement
//! for Python's
//! `animalid.AnimalIdGenerator`.
//!
//! Goal:
//! - Generate human-friendly session IDs like `quick_fox-kind_sloth-bright_auk-calm_mole`
//! - Allow `verify(session_id)` to
//!   detect tampering (no extra
//!   signature suffix in the ID)
//!
//! How it works (simple + robust):
//! - We encode **44 bits** into **4
//!   words** (each word encodes 11
//!   bits).
//! - Those 44 bits are: 32-bit random
//!   payload + 12-bit HMAC checksum.
//! - The checksum is derived from
//!   `HMAC-SHA256(secret_key,
//!   payload32)` (top 12 bits).
//! - On verify: decode words -> recover
//!   payload32 + checksum12 ->
//!   recompute checksum12 -> compare.
//!
//! This mirrors the intent of Python
//! `AnimalIdGenerator.generate()` and
//! `.verify()` without needing to
//! append a signature string.

use hmac::{
  Hmac,
  Mac
};
use rand::TryRngCore;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const ENV_SECRET_KEY: &str =
  "ANIMALID_SECRET_KEY";
pub const DEFAULT_SECRET_KEY: &str =
  "default-secret-key-for-dev";

/// This is intentionally named to match
/// the Python library semantics.
///
/// The rest of the Rust code can keep
/// calling:
/// - `SessionIdSigner::from_env()`
/// - `SessionIdSigner::generate()`
/// - `SessionIdSigner::verify(id)`
pub type SessionIdSigner =
  AnimalIdGenerator;

#[derive(Clone, Debug)]
pub struct AnimalIdGenerator {
  key: Vec<u8>
}

impl AnimalIdGenerator {
  /// Load the secret key from
  /// `ANIMALID_SECRET_KEY` or fall back
  /// to a dev default.
  pub fn from_env() -> Self {
    let secret =
      std::env::var(ENV_SECRET_KEY)
        .unwrap_or_else(|_| {
          DEFAULT_SECRET_KEY.to_string()
        });

    if secret == DEFAULT_SECRET_KEY {
      tracing::warn!(
        "Using default \
         {ENV_SECRET_KEY}. Set \
         {ENV_SECRET_KEY} for \
         production."
      );
    }

    Self {
      key: secret.into_bytes()
    }
  }

  /// Generate a new tamper-evident
  /// session id.
  pub fn generate(&self) -> String {
    // 32-bit random payload
    let payload = random_u32();

    // 12-bit checksum (HMAC over
    // payload)
    let checksum12 =
      self.checksum12(payload);

    // Pack into 44 bits: [payload32 |
    // checksum12]
    let packed: u64 =
      ((payload as u64) << 12)
        | (checksum12 as u64);

    // Split into 4 chunks of 11 bits
    // (most-significant chunk first)
    let mut words = [
      String::new(),
      String::new(),
      String::new(),
      String::new()
    ];
    for i in 0..4 {
      let shift = 11 * (3 - i);
      let idx = ((packed >> shift)
        & 0x7ff) as u16; // 0..2047
      words[i] = index_to_word(idx);
    }

    words.join("-")
  }

  /// Verify that a session id is
  /// well-formed and untampered.
  pub fn verify(
    &self,
    session_id: &str
  ) -> bool {
    let parts: Vec<&str> =
      session_id.split('-').collect();
    if parts.len() != 4 {
      return false;
    }

    // Decode 4 x 11-bit indices back
    // into packed 44-bit value
    let mut packed: u64 = 0;
    for part in parts {
      let Some(idx) =
        word_to_index(part)
      else {
        return false;
      };
      packed =
        (packed << 11) | (idx as u64);
    }

    // Unpack: [payload32 | checksum12]
    let payload = (packed >> 12) as u32;
    let checksum =
      (packed & 0x0fff) as u16;

    // Recompute and compare
    self.checksum12(payload) == checksum
  }

  fn checksum12(
    &self,
    payload: u32
  ) -> u16 {
    let mut mac =
      HmacSha256::new_from_slice(
        &self.key
      )
      .expect(
        "HMAC accepts any key length"
      );
    mac.update(&payload.to_be_bytes());
    let digest =
      mac.finalize().into_bytes();

    // Take the top 12 bits of the first
    // 2 bytes as checksum
    // Example: bytes = ab cd ->
    // checksum = (ab cd) >> 4
    let hi16 = ((digest[0] as u16)
      << 8)
      | (digest[1] as u16);
    hi16 >> 4
  }
}

fn random_u32() -> u32 {
  let mut rng = rand::rngs::OsRng;
  let mut buf = [0u8; 4];
  rng.try_fill_bytes(&mut buf).expect(
    "os rng should be available"
  );
  u32::from_be_bytes(buf)
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
  "alert", "ancient", "brave",
  "bright", "brisk", "calm", "clever",
  "cool", "curious", "daring", "eager",
  "earthy", "fast", "fierce", "gentle",
  "glad", "golden", "grand", "happy",
  "hardy", "hazy", "honest", "icy",
  "jolly", "kind", "lively", "lucky",
  "mellow", "mighty", "mirthful",
  "nimble", "noble", "odd", "patient",
  "peppy", "playful", "polite",
  "proud", "quick", "quiet", "rapid",
  "rare", "regal", "rustic", "sharp",
  "shiny", "silent", "smart", "smooth",
  "snug", "solid", "spry", "steady",
  "sturdy", "sunny", "swift", "tidy",
  "tiny", "tranquil", "vivid", "warm",
  "wild", "witty", "zesty"
];

const ANIMALS: [&str; 32] = [
  "auk", "bear", "beaver", "bison",
  "camel", "cat", "cougar", "coyote",
  "crab", "crow", "deer", "dog",
  "dolphin", "eagle", "elk", "fox",
  "frog", "goose", "hawk", "horse",
  "lion", "mole", "otter", "owl",
  "panda", "rabbit", "raven", "seal",
  "shark", "sloth", "tiger", "wolf"
];

fn index_to_word(idx: u16) -> String {
  debug_assert!(idx < 2048);
  let adj_i = (idx / 32) as usize; // 0..63
  let animal_i = (idx % 32) as usize; // 0..31
  format!(
    "{}_{}",
    ADJECTIVES[adj_i],
    ANIMALS[animal_i]
  )
}

fn word_to_index(
  word: &str
) -> Option<u16> {
  let (adj, animal) =
    word.split_once('_')?;

  let adj_i = ADJECTIVES
    .iter()
    .position(|&w| w == adj)?;
  let animal_i = ANIMALS
    .iter()
    .position(|&w| w == animal)?;

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

  fn gen_with_key(
    key: &str
  ) -> AnimalIdGenerator {
    AnimalIdGenerator {
      key: key.as_bytes().to_vec()
    }
  }

  #[test]
  fn generate_then_verify_ok() {
    let g = gen_with_key("test-secret");
    let id = g.generate();
    assert!(
      g.verify(&id),
      "generated id should verify: \
       {id}"
    );
  }

  #[test]
  fn tamper_one_char_fails() {
    let g = gen_with_key("test-secret");
    let mut id = g.generate();
    assert!(g.verify(&id));

    // mutate the last char in the
    // string (guaranteed to change
    // something)
    let last = id.pop().unwrap();
    id.push(
      if last == 'a' {
        'b'
      } else {
        'a'
      }
    );

    assert!(
      !g.verify(&id),
      "tampered id must fail: {id}"
    );
  }

  #[test]
  fn wrong_format_fails() {
    let g = gen_with_key("test-secret");
    assert!(
      !g.verify("not-even-close")
    );
    assert!(!g.verify("a_b-c_d")); // only 2 words, should be 4
    assert!(!g.verify("alert_fox-ancient_wolf-brave_bear")); // 3 words
  }

  #[test]
  fn unknown_word_fails() {
    let g = gen_with_key("test-secret");
    assert!(!g.verify("alert_fox-ancient_wolf-brave_bear-unknown_animal"));
  }
}
