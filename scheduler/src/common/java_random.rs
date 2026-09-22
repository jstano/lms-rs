//! Port of `java.util.Random`'s linear congruential generator — just enough of it
//! (`nextInt()`) for `EmployeeListLoader.createEmployeeList`'s `new Random(seedMillis)` /
//! `randomGenerator.nextInt()` per-employee tiebreak value.
//!
//! Only used to produce a stable-per-run, distinct-per-employee `EmployeeData.random` (read by
//! `DefaultSeniorityComparator` as a last-resort tiebreak, never compared across processes or
//! persisted) — so bit-exact parity with a real JVM's output isn't load-bearing, but porting the
//! actual algorithm (rather than reaching for a different RNG) costs nothing and avoids a
//! needless behavioral divergence.

const MULTIPLIER: i64 = 0x5DEECE66D;
const INCREMENT: i64 = 0xB;
const MASK: i64 = (1i64 << 48) - 1;

/// `java.util.Random`, narrowed to the one constructor and one method this crate calls.
pub struct JavaRandom {
    seed: i64,
}

impl JavaRandom {
    /// `new Random(long seed)`.
    pub fn new(seed: i64) -> Self {
        Self {
            seed: (seed ^ MULTIPLIER) & MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT)) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `nextInt()`.
    pub fn next_i32(&mut self) -> i32 {
        self.next(32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from the well-known reference sequence for `new Random(0).nextInt()` called twice.
    #[test]
    fn matches_the_reference_sequence_for_seed_zero() {
        let mut random = JavaRandom::new(0);
        assert_eq!(random.next_i32(), -1155484576);
        assert_eq!(random.next_i32(), -723955400);
    }
}
