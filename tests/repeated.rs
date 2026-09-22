// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use rgbcx::{Format, Quality};

#[test]
fn repeated_blocks_match_independent_encoding() {
    // Cross row and parallel batch boundaries; retain partial right/bottom edges.
    let (width, height) = (1031usize, 9usize);
    let rgba: Vec<u8> = (0..width * height)
        .flat_map(|i| {
            let (x, y) = (i % width, i / width);
            let alpha = match (x / 4) % 5 {
                0 | 1 => 0,
                2 | 3 => 255,
                _ => (x * 17 + y * 3) as u8,
            };
            [120, 148, 176, alpha]
        })
        .collect();
    for format in [Format::Bc1, Format::Bc3, Format::Bc4, Format::Bc5] {
        for quality in [Quality::Fast, Quality::Balanced, Quality::High] {
            let mut expected = Vec::new();
            for y in (0..height).step_by(4) {
                for x in (0..width).step_by(4) {
                    let mut block = [0; 64];
                    for (p, dst) in block.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                        let at =
                            ((y + p / 4).min(height - 1) * width + (x + p % 4).min(width - 1)) * 4;
                        dst.copy_from_slice(&rgba[at..at + 4]);
                    }
                    expected.extend(rgbcx::encode(4, 4, &block, format, quality).unwrap());
                }
            }
            assert_eq!(
                rgbcx::encode(width as u32, height as u32, &rgba, format, quality).unwrap(),
                expected
            );
            #[cfg(feature = "parallel")]
            assert_eq!(
                rgbcx::encode_parallel(width as u32, height as u32, &rgba, format, quality)
                    .unwrap(),
                expected
            );
        }
    }
}
