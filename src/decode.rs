// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use crate::{Error, Format};
pub(crate) fn color(block: &[u8; 8], four: bool) -> [[u8; 4]; 16] {
    let a = u16::from_le_bytes(block[..2].try_into().unwrap());
    let b = u16::from_le_bytes(block[2..4].try_into().unwrap());
    let expand = |v: u16| {
        let r = (v >> 11) & 31;
        let g = (v >> 5) & 63;
        let b = v & 31;
        [
            (r << 3 | r >> 2) as u8,
            (g << 2 | g >> 4) as u8,
            (b << 3 | b >> 2) as u8,
            255,
        ]
    };
    let mut pal = [expand(a), expand(b), [0; 4], [0; 4]];
    let first = pal[0];
    let second = pal[1];
    pal[2] = std::array::from_fn(|c| {
        if four || a > b {
            ((2 * first[c] as u16 + second[c] as u16) / 3) as u8
        } else {
            ((first[c] as u16 + second[c] as u16) / 2) as u8
        }
    });
    if four || a > b {
        pal[3] = std::array::from_fn(|c| ((first[c] as u16 + 2 * second[c] as u16) / 3) as u8);
    }
    pal[2][3] = 255;
    if four || a > b {
        pal[3][3] = 255;
    }
    let indices = u32::from_le_bytes(block[4..].try_into().unwrap());
    std::array::from_fn(|i| pal[((indices >> (2 * i)) & 3) as usize])
}
fn channel(block: &[u8]) -> [u8; 16] {
    let mut pal = [0u8; 8];
    pal[0] = block[0];
    pal[1] = block[1];
    if pal[0] > pal[1] {
        for i in 0..6 {
            pal[i + 2] = (((6 - i) * pal[0] as usize + (i + 1) * pal[1] as usize) / 7) as u8;
        }
    } else {
        for i in 0..4 {
            pal[i + 2] = (((4 - i) * pal[0] as usize + (i + 1) * pal[1] as usize) / 5) as u8;
        }
        pal[7] = 255;
    }
    let mut bytes = [0u8; 8];
    bytes[..6].copy_from_slice(&block[2..8]);
    let indices = u64::from_le_bytes(bytes);
    std::array::from_fn(|i| pal[((indices >> (i * 3)) & 7) as usize])
}
/// Decode row-major blocks to tightly packed RGBA8. BC4 writes (R,0,0,255),
/// BC5 writes (R,G,0,255). BC3 always uses four color interpolants regardless
/// of endpoint ordering. Encoded sRGB values are not converted to linear RGB.
pub fn decode(width: u32, height: u32, blocks: &[u8], format: Format) -> Result<Vec<u8>, Error> {
    let len = crate::image::rgba_len(width, height)?;
    if blocks.len() != format.encoded_len(width, height)? {
        return Err(Error::InputLength);
    }
    let mut output = vec![0; len];
    decode_into(width, height, blocks, format, &mut output)?;
    Ok(output)
}

/// Decode BC blocks into an exactly sized, tightly packed RGBA8 buffer.
/// Partial edge blocks are cropped to the requested image dimensions.
///
/// # Errors
/// Rejects invalid dimensions and buffer lengths before writing any output.
pub fn decode_into(
    width: u32,
    height: u32,
    blocks: &[u8],
    format: Format,
    output: &mut [u8],
) -> Result<(), Error> {
    let len = crate::image::rgba_len(width, height)?;
    if blocks.len() != format.encoded_len(width, height)? {
        return Err(Error::InputLength);
    }
    if output.len() != len {
        return Err(Error::OutputLength);
    }
    let columns = width.div_ceil(4) as usize;
    for (index, block) in blocks.chunks_exact(format.block_bytes()).enumerate() {
        let mut pixels = match format {
            Format::Bc1 => color(block.try_into().unwrap(), false),
            Format::Bc3 => color(block[8..].try_into().unwrap(), true),
            _ => [[0, 0, 0, 255]; 16],
        };
        if format == Format::Bc3 {
            let alpha = channel(&block[..8]);
            for i in 0..16 {
                pixels[i][3] = alpha[i];
            }
        }
        if matches!(format, Format::Bc4 | Format::Bc5) {
            let red = channel(&block[..8]);
            for i in 0..16 {
                pixels[i][0] = red[i];
            }
            if format == Format::Bc5 {
                let green = channel(&block[8..]);
                for i in 0..16 {
                    pixels[i][1] = green[i];
                }
            }
        }
        let x = (index % columns) * 4;
        let y = (index / columns) * 4;
        for (p, pixel) in pixels.iter().enumerate() {
            let px = x + p % 4;
            let py = y + p / 4;
            if px < width as usize && py < height as usize {
                let at = (py * width as usize + px) * 4;
                output[at..at + 4].copy_from_slice(pixel);
            }
        }
    }
    Ok(())
}
