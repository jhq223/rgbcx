// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod alpha;
mod color;
mod decode;
mod encode;
#[cfg(feature = "gpu")]
pub mod gpu;
mod image;
mod refine;
mod tables;
mod types;

pub use decode::{decode, decode_into};
#[cfg(feature = "parallel")]
pub use encode::encode_parallel;
pub use encode::{encode, encode_into};
pub use refine::refine;
pub use types::{Error, Format, Quality};
