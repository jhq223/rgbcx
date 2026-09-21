// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
/// Unsigned, normalized block-compressed pixel format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// RGB blocks with optional one-bit transparency when decoding.
    Bc1,
    /// RGB blocks with a separately encoded alpha channel.
    Bc3,
    /// One unsigned normalized channel.
    Bc4,
    /// Two unsigned normalized channels.
    Bc5,
}
impl Format {
    /// Bytes in one 4x4 block.
    pub const fn block_bytes(self) -> usize {
        match self {
            Self::Bc1 | Self::Bc4 => 8,
            _ => 16,
        }
    }
    /// Storage required, including partial blocks along the image edges.
    pub fn encoded_len(self, width: u32, height: u32) -> Result<usize, Error> {
        if width == 0 || height == 0 {
            return Err(Error::Dimensions);
        }
        (width.div_ceil(4) as usize)
            .checked_mul(height.div_ceil(4) as usize)
            .and_then(|n| n.checked_mul(self.block_bytes()))
            .filter(|&n| n <= isize::MAX as usize)
            .ok_or(Error::Dimensions)
    }
}
/// Color fitting effort. Alpha uses the same integer encoder in every mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    /// Principal-axis fitting and least-squares refinement.
    Fast,
    #[default]
    /// Cluster fitting with a high-quality retry for difficult blocks.
    Balanced,
    /// Extended partition and endpoint search.
    High,
}
/// Invalid input or output buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Zero dimensions or a size that exceeds addressable storage.
    Dimensions,
    /// Input length does not match the image dimensions and format.
    InputLength,
    /// Output length does not match the image dimensions and format.
    OutputLength,
    /// Endpoint refinement supports BC1 and BC3 only.
    UnsupportedRefinementFormat(Format),
    /// Endpoint refinement requires opaque BC1 blocks.
    TransparentBc1,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnsupportedRefinementFormat(format) => {
                return write!(f, "endpoint refinement does not support {format:?}");
            }
            Self::TransparentBc1 => "BC1 refinement requires opaque blocks",
            Self::Dimensions => "image dimensions are zero or overflow addressable storage",
            Self::InputLength => "input length does not match image dimensions and format",
            Self::OutputLength => "output length does not match image dimensions and format",
        })
    }
}
impl std::error::Error for Error {}
