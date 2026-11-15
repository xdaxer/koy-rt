use crate::media::CoverFingerprint;

/// `new Random(seed).NextBytes(data)` yerine: aynı tohum aynı baytları, farklı tohum farklı baytları verir
/// (testler yalnız bunu ister; .NET'in Random algoritması taşınmadı).
fn bytes(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(0x1234_5678_9ABC_DEF1);
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

#[test]
fn same_bytes_give_the_same_fingerprint() {
    assert_eq!(CoverFingerprint::of(&bytes(5000, 1)), CoverFingerprint::of(&bytes(5000, 1)));
}

#[test]
fn different_covers_differ() {
    assert_ne!(CoverFingerprint::of(&bytes(5000, 1)), CoverFingerprint::of(&bytes(5000, 2)));
    assert_ne!(CoverFingerprint::of(&bytes(5000, 1)), CoverFingerprint::of(&bytes(5001, 1)));
}

#[test]
fn last_bytes_are_part_of_the_fingerprint() {
    let a = bytes(5000, 3);
    let mut b = a.clone();
    *b.last_mut().unwrap() ^= 0xFF;
    assert_ne!(CoverFingerprint::of(&a), CoverFingerprint::of(&b));
}

#[test]
fn short_inputs_do_not_throw() {
    for length in [0usize, 1, 64, 65, 100] {
        assert_eq!(length as i32, CoverFingerprint::of(&bytes(length, 4)).length, "{length}");
    }
}
