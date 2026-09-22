// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use crate::Error;

pub(crate) fn rgba_len(width: u32, height: u32) -> Result<usize, Error> {
    if width == 0 || height == 0 {
        return Err(Error::Dimensions);
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|&n| n <= isize::MAX as usize)
        .ok_or(Error::Dimensions)
}
pub(crate) fn validate(width: u32, height: u32, rgba: &[u8]) -> Result<(), Error> {
    if rgba.len() != rgba_len(width, height)? {
        return Err(Error::InputLength);
    }
    Ok(())
}
pub(crate) fn pixels(width: usize, height: usize, rgba: &[u8], index: usize) -> [[u8; 4]; 16] {
    let x = (index % width.div_ceil(4)) * 4;
    let y = (index / width.div_ceil(4)) * 4;
    if x + 4 <= width && y + 4 <= height {
        let mut block = [[0; 4]; 16];
        for (row, out) in block.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let at = ((y + row) * width + x) * 4;
            out.copy_from_slice(rgba[at..at + 16].as_chunks::<4>().0);
        }
        return block;
    }
    std::array::from_fn(|p| {
        let at = (((y + p / 4).min(height - 1)) * width + (x + p % 4).min(width - 1)) * 4;
        rgba[at..at + 4].try_into().unwrap()
    })
}
