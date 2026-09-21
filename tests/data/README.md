# Reference fixture

`alpha-reference.bin` contains 9,472 BC4 blocks produced by the C++ reference
encoder. Each block holds a sixteen-sample ramp from `lo` to `max(lo, hi)`;
`lo` runs from 0 through 255 in steps of 7 and `hi` runs from 0 through 255.
Samples use integer division by 15. Each stored block is eight bytes.
The fixture tests endpoints and packed selectors independently of the Rust encoder.

`quality-bc1.bin` and `quality-bc3.bin` contain level-10 reference encodings of
the deterministic 64x64 gradient and diagonal-line image in `tests/quality.rs`.
They bound numerical color regressions; they do not measure perceptual quality.
