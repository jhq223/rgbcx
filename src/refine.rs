// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
//! Optional endpoint refinement using integer RGB error scoring.
#![forbid(unsafe_code)]
pub(crate) fn palette(pair: u32, alpha: bool) -> [[i32; 3]; 4] {
    let expand = |v: u32| {
        let r = (v >> 11) & 31;
        let g = (v >> 5) & 63;
        let b = v & 31;
        [
            ((r << 3) | (r >> 2)) as i32,
            ((g << 2) | (g >> 4)) as i32,
            ((b << 3) | (b >> 2)) as i32,
        ]
    };
    let a = pair & 65535;
    let b = pair >> 16;
    let mut p = [[0; 3]; 4];
    p[0] = expand(a);
    p[1] = expand(b);
    if alpha || a > b {
        p[2] = std::array::from_fn(|c| (2 * p[0][c] + p[1][c]) / 3);
        p[3] = std::array::from_fn(|c| (p[0][c] + 2 * p[1][c]) / 3);
    } else {
        p[2] = std::array::from_fn(|c| (p[0][c] + p[1][c]) / 2);
    }
    p
}
pub(crate) fn score(
    pixels: &[u32; 16],
    pair: u32,
    selectors: Option<u32>,
    alpha: bool,
) -> (u32, u32) {
    let pal = palette(pair, alpha);
    let n = if alpha || pair & 65535 > pair >> 16 {
        4
    } else {
        3
    };
    let (mut sum, mut bits) = (0, 0);
    for (i, &pixel) in pixels.iter().enumerate() {
        let rgb = [
            (pixel & 255) as i32,
            ((pixel >> 8) & 255) as i32,
            ((pixel >> 16) & 255) as i32,
        ];
        let error = |s: usize| {
            (0..3)
                .map(|c| (rgb[c] - pal[s][c]).pow(2) as u32)
                .sum::<u32>()
        };
        let (s, e) = if let Some(sel) = selectors {
            let s = ((sel >> (2 * i)) & 3) as usize;
            (s, error(s))
        } else {
            (0..n)
                .map(|s| (s, error(s)))
                .min_by_key(|&(s, e)| (e, s))
                .unwrap()
        };
        sum += e;
        bits |= (s as u32) << (2 * i);
    }
    (sum, bits)
}
pub(crate) fn neighbor(value: u32, mut index: u32) -> Option<u32> {
    let mut out = 0;
    for (shift, max) in [(0, 31), (5, 63), (11, 31)] {
        let v = ((value >> shift) & max) as i32 + (index % 3) as i32 - 1;
        index /= 3;
        if v < 0 || v > max as i32 {
            return None;
        }
        out |= (v as u32) << shift;
    }
    Some(out)
}
pub(crate) fn block(pixels: &[u32; 16], original: [u32; 2], alpha: bool) -> [u32; 2] {
    let mut best = original;
    let (mut error, _) = score(pixels, original[0], Some(original[1]), alpha);
    for candidate in 0..729 {
        let Some(a) = neighbor(original[0] & 65535, candidate % 27) else {
            continue;
        };
        let Some(b) = neighbor(original[0] >> 16, candidate / 27) else {
            continue;
        };
        if alpha && a <= b {
            continue;
        }
        let pair = a | (b << 16);
        let (e, selectors) = score(pixels, pair, None, alpha);
        if e < error {
            error = e;
            best = [pair, selectors];
        }
    }
    best
}
pub(crate) fn gather(width: usize, rgba: &[u8], index: usize) -> [u32; 16] {
    crate::image::pixels(width, rgba.len() / 4 / width, rgba, index).map(u32::from_le_bytes)
}

pub(crate) fn validate(
    width: u32,
    height: u32,
    rgba: &[u8],
    format: crate::Format,
    blocks: &[u8],
) -> Result<(), crate::Error> {
    crate::image::validate(width, height, rgba)?;
    if !matches!(format, crate::Format::Bc1 | crate::Format::Bc3) {
        return Err(crate::Error::UnsupportedRefinementFormat(format));
    }
    if blocks.len() != format.encoded_len(width, height)? {
        return Err(crate::Error::OutputLength);
    }
    if format == crate::Format::Bc1
        && blocks.as_chunks::<8>().0.iter().any(|b| {
            let a = u16::from_le_bytes([b[0], b[1]]);
            let c = u16::from_le_bytes([b[2], b[3]]);
            let bits = u32::from_le_bytes(b[4..8].try_into().unwrap());
            a <= c && (0..16).any(|i| (bits >> (i * 2)) & 3 == 3)
        })
    {
        return Err(crate::Error::TransparentBc1);
    }
    Ok(())
}
/// Search the 729 neighboring RGB565 endpoint pairs per BC block. Keeps the
/// original result on equal error, preserves BC3 alpha bytes, never uses the
/// transparent selector for an opaque BC1 block. Supports [`crate::Format::Bc1`]
/// and [`crate::Format::Bc3`].
///
/// # Errors
/// Rejects invalid dimensions, buffer lengths and BC1 blocks with transparency.
/// Validation happens before any output is written.
pub fn refine(
    width: u32,
    height: u32,
    rgba: &[u8],
    format: crate::Format,
    blocks: &mut [u8],
) -> Result<(), crate::Error> {
    validate(width, height, rgba, format, blocks)?;
    let alpha = format == crate::Format::Bc3;
    let stride = if alpha { 16 } else { 8 };
    let offset = if alpha { 8 } else { 0 };
    for (i, out) in blocks.chunks_exact_mut(stride).enumerate() {
        let pixels = gather(width as usize, rgba, i);
        let original = [
            u32::from_le_bytes(out[offset..offset + 4].try_into().unwrap()),
            u32::from_le_bytes(out[offset + 4..offset + 8].try_into().unwrap()),
        ];
        let best = block(&pixels, original, alpha);
        out[offset..offset + 4].copy_from_slice(&best[0].to_le_bytes());
        out[offset + 4..offset + 8].copy_from_slice(&best[1].to_le_bytes());
    }
    Ok(())
}
