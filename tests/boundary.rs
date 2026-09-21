// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use rgbcx::{Error, Format, Quality, decode, encode, encode_into};
#[test]
fn rejects_bad_buffers_without_writing_output() {
    for (w, h, bytes) in [(0, 8, 0), (8, 0, 0), (8, 8, 255), (u32::MAX, u32::MAX, 0)] {
        assert!(encode(w, h, &vec![0; bytes], Format::Bc1, Quality::Fast).is_err());
    }
    let mut output = [123; 9];
    assert_eq!(
        encode_into(4, 4, &[0; 64], Format::Bc1, Quality::Fast, &mut output),
        Err(Error::OutputLength)
    );
    assert_eq!(output, [123; 9]);
    assert!(decode(4, 4, &[0; 7], Format::Bc1).is_err());
}
#[test]
fn partial_blocks_replicate_edges_and_decode_to_original_dimensions() {
    for (w, h) in [(1, 1), (3, 7), (9, 13), (1025, 1)] {
        let rgba = [64, 128, 192, 137].repeat(w * h);
        for fmt in [Format::Bc1, Format::Bc3, Format::Bc4, Format::Bc5] {
            let bc = encode(w as u32, h as u32, &rgba, fmt, Quality::Balanced).unwrap();
            let decoded = decode(w as u32, h as u32, &bc, fmt).unwrap();
            assert_eq!(decoded.len(), rgba.len());
            assert!(
                decoded
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p == &decoded[..4])
            );
            if fmt == Format::Bc3 {
                assert_eq!(decoded[3], 137);
            }
        }
    }
}
#[test]
fn parallel_calls_are_deterministic() {
    let jobs: Vec<_> = (0..12)
        .map(|_| {
            std::thread::spawn(|| {
                encode(
                    8,
                    8,
                    &[64, 128, 192, 255].repeat(64),
                    Format::Bc1,
                    Quality::Balanced,
                )
                .unwrap()
            })
        })
        .collect();
    let expected = encode(
        8,
        8,
        &[64, 128, 192, 255].repeat(64),
        Format::Bc1,
        Quality::Balanced,
    )
    .unwrap();
    for job in jobs {
        assert_eq!(job.join().unwrap(), expected);
    }
}
#[test]
fn decodes_bc1_transparency_and_bc3_reversed_color_endpoints() {
    let color = [0, 0, 255, 255, 255, 255, 255, 255];
    assert_eq!(decode(4, 4, &color, Format::Bc1).unwrap(), [0; 64]);
    let mut bc3 = vec![255, 255, 0, 0, 0, 0, 0, 0];
    bc3.extend(color);
    assert_eq!(
        decode(4, 4, &bc3, Format::Bc3).unwrap(),
        [170, 170, 170, 255].repeat(16)
    );
    let mut refined = bc3.clone();
    rgbcx::refine(
        4,
        4,
        &[170, 170, 170, 255].repeat(16),
        Format::Bc3,
        &mut refined,
    )
    .unwrap();
    assert_eq!(refined, bc3);
    assert!(rgbcx::refine(4, 4, &[0; 64], Format::Bc1, &mut color.clone()).is_err());
}
