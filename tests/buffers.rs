// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use rgbcx::{Error, Format, Quality, decode, decode_into, encode, refine};

#[test]
fn caller_owned_decode_matches_allocating_decode_for_all_formats() {
    let rgba: Vec<u8> = (0..7 * 13 * 4).map(|i| (i * 17 + i / 7) as u8).collect();
    for format in [Format::Bc1, Format::Bc3, Format::Bc4, Format::Bc5] {
        let encoded = encode(7, 13, &rgba, format, Quality::Fast).unwrap();
        let mut buffer = vec![91; rgba.len()];
        decode_into(7, 13, &encoded, format, &mut buffer).unwrap();
        assert_eq!(buffer, decode(7, 13, &encoded, format).unwrap());
    }
}

#[test]
fn decoding_rejects_invalid_buffers_before_mutation() {
    let mut output = [123; 64];
    assert_eq!(
        decode_into(4, 4, &[0; 7], Format::Bc1, &mut output),
        Err(Error::InputLength)
    );
    assert_eq!(output, [123; 64]);
    assert_eq!(
        decode_into(4, 4, &[0; 8], Format::Bc1, &mut output[..63]),
        Err(Error::OutputLength)
    );
    assert_eq!(output, [123; 64]);
}

#[test]
fn refinement_errors_are_typed_and_leave_blocks_untouched() {
    for format in [Format::Bc4, Format::Bc5] {
        let mut blocks = vec![91; format.block_bytes()];
        let original = blocks.clone();
        assert_eq!(
            refine(4, 4, &[0; 64], format, &mut blocks),
            Err(Error::UnsupportedRefinementFormat(format))
        );
        assert_eq!(blocks, original);
    }
    let mut transparent = [0, 0, 255, 255, 255, 255, 255, 255];
    let original = transparent;
    assert_eq!(
        refine(4, 4, &[0; 64], Format::Bc1, &mut transparent),
        Err(Error::TransparentBc1)
    );
    assert_eq!(transparent, original);
}
