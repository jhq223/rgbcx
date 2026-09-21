// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use rgbcx::{Format, Quality, encode, refine};
#[cfg(any(feature = "parallel", feature = "gpu"))]
fn format(alpha: bool) -> Format {
    if alpha { Format::Bc3 } else { Format::Bc1 }
}
fn pixels(w: usize, h: usize) -> Vec<u8> {
    (0..w * h)
        .flat_map(|i| {
            let x = i % w;
            let y = i / w;
            [
                (x * 13 + y * 7) as u8,
                (x * 2 + y * 21) as u8,
                (x * 19 + y * 3) as u8,
                if x.is_multiple_of(3) {
                    0
                } else if y.is_multiple_of(3) {
                    255
                } else {
                    (i * 23) as u8
                },
            ]
        })
        .collect()
}
#[cfg(feature = "parallel")]
#[test]
fn parallel_blocks_match_serial_encoder() {
    for (w, h) in [(1, 1), (7, 13), (64, 32), (128, 64)] {
        let rgba = pixels(w, h);
        for alpha in [false, true] {
            for level in [Quality::Fast, Quality::Balanced, Quality::High] {
                assert_eq!(
                    encode(w as u32, h as u32, &rgba, format(alpha), level).unwrap(),
                    rgbcx::encode_parallel(w as u32, h as u32, &rgba, format(alpha), level)
                        .unwrap()
                );
            }
        }
    }
}
#[test]
fn refinement_preserves_alpha_and_never_increases_error() {
    let rgba = pixels(32, 32);
    let original = encode(32, 32, &rgba, Format::Bc3, Quality::Balanced).unwrap();
    let mut refined = original.clone();
    refine(32, 32, &rgba, Format::Bc3, &mut refined).unwrap();
    for (a, b) in original
        .as_chunks::<16>()
        .0
        .iter()
        .zip(refined.as_chunks::<16>().0.iter())
    {
        assert_eq!(&a[..8], &b[..8]);
    }
    // Independent BC palette decoder and per-block SSE oracle.
    fn errors(rgba: &[u8], blocks: &[u8]) -> Vec<u32> {
        blocks
            .as_chunks::<16>()
            .0
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let e0 = u16::from_le_bytes(b[8..10].try_into().unwrap());
                let e1 = u16::from_le_bytes(b[10..12].try_into().unwrap());
                let expand = |e: u16| {
                    let r = (e >> 11) & 31;
                    let g = (e >> 5) & 63;
                    let b = e & 31;
                    [
                        (r << 3 | r >> 2) as i32,
                        (g << 2 | g >> 4) as i32,
                        (b << 3 | b >> 2) as i32,
                    ]
                };
                let a = expand(e0);
                let c = expand(e1);
                let pal = [
                    a,
                    c,
                    std::array::from_fn(|j| (2 * a[j] + c[j]) / 3),
                    std::array::from_fn(|j| (a[j] + 2 * c[j]) / 3),
                ];
                let bits = u32::from_le_bytes(b[12..16].try_into().unwrap());
                (0..16)
                    .map(|p| {
                        let at = (((i / 8) * 4 + p / 4) * 32 + (i % 8) * 4 + p % 4) * 4;
                        let sel = ((bits >> (2 * p)) & 3) as usize;
                        (0..3)
                            .map(|j| (rgba[at + j] as i32 - pal[sel][j]).pow(2) as u32)
                            .sum::<u32>()
                    })
                    .sum()
            })
            .collect()
    }
    for (a, b) in errors(&rgba, &original)
        .into_iter()
        .zip(errors(&rgba, &refined))
    {
        assert!(b <= a);
    }
}
#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires hardware compute adapter"]
fn gpu_refinement_matches_cpu_with_alpha_edges_and_multiple_batches() {
    let mut gpu = rgbcx::gpu::Refiner::new().unwrap();
    for (w, h) in [(7, 13), (32, 32), (512, 132)] {
        let rgba = pixels(w, h);
        for alpha in [false, true] {
            let mut cpu = encode_blocks(w, h, &rgba, alpha);
            let mut actual = cpu.clone();
            refine(w as u32, h as u32, &rgba, format(alpha), &mut cpu).unwrap();
            gpu.refine(w as u32, h as u32, &rgba, format(alpha), &mut actual)
                .unwrap();
            assert_eq!(cpu, actual);
        }
    }
}
#[cfg(feature = "gpu")]
fn encode_blocks(w: usize, h: usize, rgba: &[u8], alpha: bool) -> Vec<u8> {
    let format = format(alpha);
    let mut blocks = encode(w as u32, h as u32, rgba, format, Quality::Fast).unwrap();
    // External BC3 blocks may use equal or reversed color endpoints.
    if alpha {
        for (i, b) in blocks.as_chunks_mut::<16>().0.iter_mut().enumerate() {
            if i % 3 == 0 {
                b[8..12].copy_from_slice(&[0, 0, 255, 255]);
            }
            if i % 3 == 1 {
                b[8..12].copy_from_slice(&[31, 0, 31, 0]);
            }
        }
    }
    blocks
}

#[test]
fn alpha_matches_reference_endpoints_and_selectors() {
    let expected = include_bytes!("data/alpha-reference.bin");
    let mut index = 0;
    for lo in (0u8..=255).step_by(7) {
        for hi in 0u8..=255 {
            let hi = hi.max(lo);
            let values = std::array::from_fn(|p| {
                (lo as u32 + (hi as u32 - lo as u32) * p as u32 / 15) as u8
            });
            assert_eq!(
                rgbcx::alpha::encode(&values),
                expected[index..index + 8],
                "{lo}..{hi}"
            );
            index += 8;
        }
    }
    assert_eq!(index, expected.len());
}
