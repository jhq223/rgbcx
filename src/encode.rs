// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use crate::image::{pixels, validate};
use crate::{Error, Format, Quality, alpha, color, decode};

fn block(pixels: &[[u8; 4]; 16], format: Format, quality: Quality, out: &mut [u8]) {
    match format {
        Format::Bc1 | Format::Bc3 => {
            let offset = if format == Format::Bc3 {
                out[..8].copy_from_slice(&alpha::encode(&std::array::from_fn(|i| pixels[i][3])));
                8
            } else {
                0
            };
            let mut color = color::encode(pixels, quality, format == Format::Bc1);
            if quality == Quality::Balanced {
                let error = |data: &[u8; 8]| {
                    let decoded = decode::color(data, format == Format::Bc3);
                    let mut sum = 0u32;
                    let mut count = 0u32;
                    for i in 0..16 {
                        if format == Format::Bc3 && pixels[i][3] < 128 {
                            continue;
                        }
                        for c in 0..3 {
                            sum += (pixels[i][c] as i32 - decoded[i][c] as i32).pow(2) as u32;
                            count += 1;
                        }
                    }
                    (sum, count)
                };
                let (before, count) = error(&color);
                if before > count * 25 {
                    let trial = color::encode(pixels, Quality::High, format == Format::Bc1);
                    if error(&trial).0 < before {
                        color = trial;
                    }
                }
            }
            out[offset..offset + 8].copy_from_slice(&color);
        }
        Format::Bc4 => out.copy_from_slice(&alpha::encode(&std::array::from_fn(|i| pixels[i][0]))),
        Format::Bc5 => {
            out[..8].copy_from_slice(&alpha::encode(&std::array::from_fn(|i| pixels[i][0])));
            out[8..].copy_from_slice(&alpha::encode(&std::array::from_fn(|i| pixels[i][1])));
        }
    }
}
/// Encode tightly packed RGBA8 pixels. Partial edge blocks replicate the last
/// row/column. BC1 ignores alpha; BC3 uses alpha, BC4 uses R, BC5 uses R and G.
/// No color-space conversion, premultiplication or dithering is performed.
///
/// # Errors
/// Returns an error for invalid dimensions or an incorrectly sized input buffer.
pub fn encode(
    width: u32,
    height: u32,
    rgba: &[u8],
    format: Format,
    quality: Quality,
) -> Result<Vec<u8>, Error> {
    validate(width, height, rgba)?;
    let mut output = vec![0; format.encoded_len(width, height)?];
    encode_into(width, height, rgba, format, quality, &mut output)?;
    Ok(output)
}
/// Encode into an exactly sized caller-owned buffer, in row-major block order.
///
/// # Errors
/// Returns an error for invalid dimensions or buffer lengths. Validation happens
/// before any output is written.
pub fn encode_into(
    width: u32,
    height: u32,
    rgba: &[u8],
    format: Format,
    quality: Quality,
    output: &mut [u8],
) -> Result<(), Error> {
    validate(width, height, rgba)?;
    if output.len() != format.encoded_len(width, height)? {
        return Err(Error::OutputLength);
    }
    for (i, out) in output.chunks_exact_mut(format.block_bytes()).enumerate() {
        block(
            &pixels(width as usize, height as usize, rgba, i),
            format,
            quality,
            out,
        );
    }
    Ok(())
}
/// Encode blocks using the caller's Rayon pool. No private thread pool is created.
///
/// # Errors
/// Returns an error for invalid dimensions or an incorrectly sized input buffer.
#[cfg(feature = "parallel")]
pub fn encode_parallel(
    width: u32,
    height: u32,
    rgba: &[u8],
    format: Format,
    quality: Quality,
) -> Result<Vec<u8>, Error> {
    use rayon::prelude::*;
    validate(width, height, rgba)?;
    let mut output = vec![0; format.encoded_len(width, height)?];
    output
        .par_chunks_mut(format.block_bytes() * 64)
        .enumerate()
        .for_each(|(batch, chunk)| {
            for (i, out) in chunk.chunks_exact_mut(format.block_bytes()).enumerate() {
                block(
                    &pixels(width as usize, height as usize, rgba, batch * 64 + i),
                    format,
                    quality,
                    out,
                );
            }
        });
    Ok(output)
}
