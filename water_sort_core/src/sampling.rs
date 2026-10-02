//! Platform-independent sampling primitives (D10).
//!
//! `rand`'s `gen_range` and `shuffle` may change between crate versions and depend on `usize`
//! width. These use only `next_u32` (and `next_u64` for [`bounded_u64`]), never `usize`, so
//! results are bit-identical on 64-bit native, wasm32 and through Python.

use rand_core::Rng;

/// Uniform integer in `0..n`, by Lemire's nearly-divisionless method with rejection.
///
/// # Panics
///
/// If `n == 0`.
pub fn bounded_u32<R: Rng + ?Sized>(rng: &mut R, n: u32) -> u32 {
    assert!(n > 0, "bounded_u32 needs a non-empty range");
    let mut m = u64::from(rng.next_u32()) * u64::from(n);
    if low32(m) < n {
        // 2^32 mod n: the number of low values that would bias the result.
        let threshold = n.wrapping_neg() % n;
        while low32(m) < threshold {
            m = u64::from(rng.next_u32()) * u64::from(n);
        }
    }
    high32(m)
}

/// Uniform integer in `0..n`, by Lemire's method on `next_u64` (the 64-bit analogue of
/// [`bounded_u32`]). Used for the DP counts of distributed height vectors (D14), which can
/// exceed `u32`.
///
/// # Panics
///
/// If `n == 0`.
pub fn bounded_u64<R: Rng + ?Sized>(rng: &mut R, n: u64) -> u64 {
    assert!(n > 0, "bounded_u64 needs a non-empty range");
    let mut m = u128::from(rng.next_u64()) * u128::from(n);
    if low64(m) < n {
        // 2^64 mod n.
        let threshold = n.wrapping_neg() % n;
        while low64(m) < threshold {
            m = u128::from(rng.next_u64()) * u128::from(n);
        }
    }
    high64(m)
}

#[allow(clippy::cast_possible_truncation)] // the truncation is the point
const fn low64(x: u128) -> u64 {
    x as u64
}

#[allow(clippy::cast_possible_truncation)] // x >> 64 fits
const fn high64(x: u128) -> u64 {
    (x >> 64) as u64
}

const fn low32(x: u64) -> u32 {
    (x & 0xFFFF_FFFF) as u32
}

const fn high32(x: u64) -> u32 {
    (x >> 32) as u32
}

/// Fisher-Yates shuffle: for `i` from `len - 1` down to 1, swap `i` with
/// `j = bounded_u32(rng, i + 1)`.
///
/// # Panics
///
/// If `xs.len()` exceeds `u32::MAX`.
pub fn fisher_yates<R: Rng + ?Sized, T>(rng: &mut R, xs: &mut [T]) {
    for i in (1..xs.len()).rev() {
        let bound = u32::try_from(i + 1).expect("slice length fits in u32");
        let j = bounded_u32(rng, bound) as usize;
        xs.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use rand_core::TryRng;

    /// Replays a fixed list of `next_u32` outputs.
    struct Script(Vec<u32>);

    impl TryRng for Script {
        type Error = Infallible;
        fn try_next_u32(&mut self) -> Result<u32, Infallible> {
            Ok(self.0.remove(0))
        }
        fn try_next_u64(&mut self) -> Result<u64, Infallible> {
            unimplemented!("sampling must only use next_u32")
        }
        fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), Infallible> {
            unimplemented!("sampling must only use next_u32")
        }
    }

    #[test]
    fn lemire_maps_high_bits() {
        // m = x * n; the result is m >> 32.
        let mut rng = Script(vec![1, u32::MAX, (1 << 31) + 1]);
        assert_eq!(bounded_u32(&mut rng, 10), 0);
        assert_eq!(bounded_u32(&mut rng, 10), 9);
        assert_eq!(bounded_u32(&mut rng, 10), 5);
    }

    #[test]
    fn lemire_rejects_biased_low_values() {
        // n = 3: threshold = 2^32 mod 3 = 1. x = 0 gives low = 0 < 1 -> rejected.
        let mut rng = Script(vec![0, 7]);
        assert_eq!(bounded_u32(&mut rng, 3), 0);
        assert!(rng.0.is_empty(), "the second draw was used");
        // n = 1 never rejects.
        let mut rng = Script(vec![0]);
        assert_eq!(bounded_u32(&mut rng, 1), 0);
    }

    #[test]
    fn fisher_yates_order_of_draws() {
        // len 3: i = 2 draws from 0..3, then i = 1 from 0..2.
        let mut rng = Script(vec![u32::MAX, 0]);
        let mut xs = [10, 20, 30];
        fisher_yates(&mut rng, &mut xs);
        // i = 2, j = 2 (no-op); i = 1, j = 0 (swap).
        assert_eq!(xs, [20, 10, 30]);
        let mut empty: [u8; 0] = [];
        fisher_yates(&mut Script(vec![]), &mut empty);
    }

    /// Replays a fixed list of `next_u64` outputs.
    struct Script64(Vec<u64>);

    impl TryRng for Script64 {
        type Error = Infallible;
        fn try_next_u32(&mut self) -> Result<u32, Infallible> {
            unimplemented!("bounded_u64 must only use next_u64")
        }
        fn try_next_u64(&mut self) -> Result<u64, Infallible> {
            Ok(self.0.remove(0))
        }
        fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), Infallible> {
            unimplemented!("bounded_u64 must only use next_u64")
        }
    }

    #[test]
    fn lemire_u64() {
        // m = x * n; the result is m >> 64.
        let mut rng = Script64(vec![1, u64::MAX, (1 << 63) + 1]);
        assert_eq!(bounded_u64(&mut rng, 10), 0);
        assert_eq!(bounded_u64(&mut rng, 10), 9);
        assert_eq!(bounded_u64(&mut rng, 10), 5);
        // n = 3: threshold = 2^64 mod 3 = 1, so x = 0 is rejected.
        let mut rng = Script64(vec![0, 7]);
        assert_eq!(bounded_u64(&mut rng, 3), 0);
        assert!(rng.0.is_empty(), "the second draw was used");
        // A range above u32.
        let n = 9u64.pow(16);
        let mut rng = Script64(vec![u64::MAX]);
        assert_eq!(bounded_u64(&mut rng, n), n - 1);
    }

    #[test]
    #[should_panic(expected = "non-empty range")]
    fn zero_range_u64_panics() {
        bounded_u64(&mut Script64(vec![1]), 0);
    }

    #[test]
    #[should_panic(expected = "non-empty range")]
    fn zero_range_panics() {
        bounded_u32(&mut Script(vec![1]), 0);
    }
}
