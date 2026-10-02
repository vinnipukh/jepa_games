//! Seed derivation. Pure functions: core never reads the clock (D1), so it stays WASM-safe.

/// The `SplitMix64` step: the first output of a `SplitMix64` generator whose state is `x`.
///
/// Used to derive seeds (time seeds here, per-puzzle seeds in Phase 4).
pub const fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Seed from a clock reading and a per-process counter:
/// `splitmix64(now_nanos ^ splitmix64(counter))`.
///
/// The caller reads the clock (`SystemTime` natively, `Date.now()` on the web) and owns the
/// counter, which keeps seeds distinct within one clock tick. A time seed is a convenient seed
/// choice, not true randomness; it is recorded like any other seed.
pub const fn time_seed(now_nanos: u64, counter: u64) -> u64 {
    splitmix64(now_nanos ^ splitmix64(counter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_reference_values() {
        // Reference outputs of SplitMix64 seeded with 0 (Vigna's splitmix64.c).
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix64(0x9E37_79B9_7F4A_7C15), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(
            splitmix64(0x9E37_79B9_7F4A_7C15_u64.wrapping_mul(2)),
            0x06C4_5D18_8009_454F
        );
    }

    #[test]
    fn counter_separates_equal_clock_readings() {
        let now = 1_759_363_200_000_000_000;
        let seeds: Vec<u64> = (0..1000).map(|c| time_seed(now, c)).collect();
        let mut unique = seeds.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), seeds.len());
        assert_ne!(time_seed(now, 0), time_seed(now + 1, 0));
    }
}
