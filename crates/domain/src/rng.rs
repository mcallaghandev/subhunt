//! A small deterministic random number generator (xorshift64*).
//!
//! Using a seeded generator instead of a global random source means a test can
//! create a `GameWorld` with seed 42 and get the same spawn points every run.

#[derive(Clone, Debug)]
pub(crate) struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    pub(crate) fn new(seed: u64) -> Self {
        // The state must never be zero or the generator gets stuck.
        Self { state: (seed ^ 0x9E37_79B9_7F4A_7C15) | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `[0, 1)`.
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number in `[low, high)`.
    pub(crate) fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next_f32()
    }
}
