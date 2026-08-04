//! The one source of randomness in the crate.
//!
//! Upstream reaches for Python's global `random` module; ADR-0006 forbids that
//! shape outright. Every draw here comes from a caller-supplied `u64` seed
//! through splitmix64 — the same generator the compiler's layout solver uses
//! (`crates/compiler/src/solver.rs`), duplicated rather than shared because this
//! crate sits below the compiler and must not depend on it.
//!
//! Nothing else in the crate may consume entropy: no wall clock, no hash order,
//! no address-dependent iteration.

/// A deterministic, non-cryptographic PRNG (splitmix64).
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Seed a stream.
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// Next 64-bit value (advances the state).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Choose an index in proportion to `weights`.
    ///
    /// A single candidate is returned **without** drawing, so adding an
    /// unreachable alternative to a rule cannot shift the stream of an
    /// unrelated part of the model. Returns `None` only for an empty or
    /// all-zero weight list, which [`crate::ir::Program::validate`] already
    /// rejects.
    pub fn weighted(&mut self, weights: &[u32]) -> Option<usize> {
        match weights.len() {
            0 => return None,
            1 => return if weights[0] == 0 { None } else { Some(0) },
            _ => {}
        }
        let total: u64 = weights.iter().map(|&w| w as u64).sum();
        if total == 0 {
            return None;
        }
        let mut pick = self.next_u64() % total;
        for (i, &w) in weights.iter().enumerate() {
            let w = w as u64;
            if pick < w {
                return Some(i);
            }
            pick -= w;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let mut c = Rng::new(43);
        let sa: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let sb: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        let sc: Vec<u64> = (0..8).map(|_| c.next_u64()).collect();
        assert_eq!(sa, sb);
        assert_ne!(sa, sc);
    }

    #[test]
    fn single_candidate_does_not_consume_the_stream() {
        let mut rng = Rng::new(7);
        assert_eq!(rng.weighted(&[3]), Some(0));
        let after = rng.clone().next_u64();
        assert_eq!(after, Rng::new(7).next_u64());
    }

    #[test]
    fn weights_are_respected() {
        let mut rng = Rng::new(1);
        let mut counts = [0usize; 3];
        for _ in 0..1000 {
            counts[rng.weighted(&[8, 1, 1]).unwrap()] += 1;
        }
        assert!(counts[0] > counts[1] + counts[2], "got {counts:?}");
        assert!(counts[1] > 0 && counts[2] > 0, "got {counts:?}");
        assert_eq!(rng.weighted(&[0, 0]), None);
        assert_eq!(rng.weighted(&[]), None);
    }
}
