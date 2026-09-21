// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use rgbcx::{Format, Quality, decode, encode};
fn source() -> Vec<u8> {
    (0..64)
        .flat_map(|y| {
            (0..64).flat_map(move |x| {
                let mut p = [
                    (x * 4 + y) as u8,
                    (y * 4 + x) as u8,
                    (x * 3 + y * 2) as u8,
                    if x % 7 == 0 { 0 } else { 255 },
                ];
                if x == y || x == 63 - y {
                    p[..3].fill(0);
                }
                p
            })
        })
        .collect()
}
fn error(original: &[u8], decoded: &[u8]) -> u64 {
    original
        .as_chunks::<4>()
        .0
        .iter()
        .zip(decoded.as_chunks::<4>().0.iter())
        .map(|(a, b)| {
            (0..3)
                .map(|c| (a[c] as i32 - b[c] as i32).pow(2) as u64)
                .sum::<u64>()
        })
        .sum()
}
#[test]
fn gradients_and_line_art_stay_close_to_reference_quality() {
    let rgba = source();
    for (fmt, reference) in [
        (
            Format::Bc1,
            include_bytes!("data/quality-bc1.bin").as_slice(),
        ),
        (
            Format::Bc3,
            include_bytes!("data/quality-bc3.bin").as_slice(),
        ),
    ] {
        let reference_error = error(&rgba, &decode(64, 64, reference, fmt).unwrap());
        for q in [Quality::Balanced, Quality::High] {
            let encoded = encode(64, 64, &rgba, fmt, q).unwrap();
            let decoded = decode(64, 64, &encoded, fmt).unwrap();
            let actual = error(&rgba, &decoded);
            assert!(
                actual * 100 <= reference_error * 102,
                "{fmt:?} {q:?}: {actual} > {reference_error}"
            );
            if fmt == Format::Bc1 {
                assert!(decoded.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
            }
        }
    }
}
#[test]
fn arbitrary_color_blocks_are_valid_and_opaque() {
    let mut seed = 711u32;
    let rgba: Vec<_> = (0..128 * 128 * 4)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as u8
        })
        .collect();
    for q in [Quality::Fast, Quality::Balanced, Quality::High] {
        let blocks = encode(128, 128, &rgba, Format::Bc1, q).unwrap();
        assert!(
            decode(128, 128, &blocks, Format::Bc1)
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[3] == 255)
        );
    }
}
