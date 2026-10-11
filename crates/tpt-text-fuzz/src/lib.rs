//! Randomised robustness harness for tpt-text.
//!
//! The tests in `tests/` feed pseudo-random text and widths into the layout
//! engine and assert it neither panics nor loops forever. The generator is a
//! fixed-seed PRNG so any failure reproduces exactly.

/// Minimal deterministic PRNG (64-bit LCG, high bits). Not for cryptography.
pub struct Lcg(u64);

impl Lcg {
    /// Creates a generator from `seed`.
    pub fn new(seed: u64) -> Lcg {
        Lcg(seed)
    }

    /// Returns the next pseudo-random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    /// Returns a value in `0..bound`. `bound` must be non-zero.
    pub fn below(&mut self, bound: u32) -> u32 {
        self.next_u32() % bound
    }
}

/// Characters drawn by the generator: ASCII, Latin-1, Arabic, Devanagari,
/// ZWJ/ZWNJ, emoji, newlines and combining marks.
const POOL: &[char] = &[
    'a',
    'Z',
    ' ',
    '\t',
    '\n',
    '-',
    '.',
    '0',
    '9',
    'é',
    '\u{0301}',
    '\u{200D}',
    '\u{200C}',
    '\u{0628}',
    '\u{062A}',
    '\u{0627}',
    '\u{0646}',
    '\u{0915}',
    '\u{094D}',
    '\u{0937}',
    '\u{1F600}',
    '\u{00AD}',
    '\u{2028}',
];

/// Builds a random string of up to `max_chars` characters from [`POOL`].
pub fn random_text(rng: &mut Lcg, max_chars: u32) -> String {
    let len = rng.below(max_chars + 1);
    (0..len)
        .map(|_| POOL[rng.below(POOL.len() as u32) as usize])
        .collect()
}
