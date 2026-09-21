// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
//! Integer BC4 channel encoding, also used for BC3 alpha blocks.
#![forbid(unsafe_code)]
/// Encode sixteen scalar values. Preserves the input minimum and maximum.
pub fn encode(values: &[u8; 16]) -> [u8; 8] {
    let lo = *values.iter().min().unwrap();
    let hi = *values.iter().max().unwrap();
    let mut out = [0; 8];
    out[0] = hi;
    out[1] = lo;
    if lo == hi {
        return out;
    }
    let delta = i32::from(hi) - i32::from(lo);
    let bias = 4 - i32::from(lo) * 14;
    let map = [1, 7, 6, 5, 4, 3, 2, 0];
    let mut bits = 0u64;
    for (i, &v) in values.iter().enumerate() {
        let v = i32::from(v) * 14 + bias;
        let index = [13, 11, 9, 7, 5, 3, 1]
            .iter()
            .filter(|&&t| v >= delta * t)
            .count();
        bits |= (map[index] as u64) << (i * 3);
    }
    out[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
    out
}
