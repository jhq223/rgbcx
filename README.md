# rgbcx

English | [简体中文](README.zh-CN.md)

BC1, BC3, BC4 and BC5 texture encoding and decoding in Rust, with optional
parallel encoding and GPU endpoint refinement. The default build has no dependencies.

## Installation

```toml
[dependencies]
rgbcx = "0.1"
```

| Cargo feature  | Effect                                                 |
| -------------- | ------------------------------------------------------ |
| Default (none) | CPU encoding, decoding and refinement; no dependencies |
| `parallel`     | Adds `encode_parallel` using Rayon                     |
| `gpu`          | Adds a reusable GPU refinement worker using wgpu       |

Both optional features can be enabled independently or together.

## Encoding and decoding

```rust
use rgbcx::{decode, encode, Format, Quality};

fn main() -> Result<(), rgbcx::Error> {
    let rgba = [120, 160, 200, 255].repeat(8 * 8);
    let blocks = encode(8, 8, &rgba, Format::Bc3, Quality::Balanced)?;
    let preview = decode(8, 8, &blocks, Format::Bc3)?;
    Ok(())
}
```

Input pixels are tightly packed RGBA8. Output is a row-major array of 4x4 blocks,
without a DDS, KTX or other container header. Nonzero dimensions are accepted
if buffer sizes fit in addressable memory. Partial blocks replicate edge pixels;
`decode` crops those blocks to the requested dimensions. `Format::encoded_len`
computes the required block buffer size; `encode_into` and `decode_into` write into exactly sized caller-owned buffers.
Reuse these buffers when processing many images. Errors are returned as `rgbcx::Error`;
invalid buffers are rejected before any output is modified.

| Format | Input channels                          | Bytes per block |
| ------ | --------------------------------------- | --------------- |
| BC1    | RGB; alpha is ignored, output is opaque | 8               |
| BC3    | RGBA                                    | 16              |
| BC4    | R                                       | 8               |
| BC5    | R and G                                 | 16              |

BC4 and BC5 use unsigned normalized channels. Decoding fills unused color channels
with zero and alpha with 255. BC1 decoding also accepts externally encoded
punch-through transparency. BC3 always uses four color interpolants.

## Quality and parallelism

`Fast` uses a principal-axis fit and least-squares refinement. `Balanced` adds
cluster fitting and retries difficult blocks with `High`. `High` searches more
partitions and neighboring endpoints. Alpha encoding is the same in every mode.
Color error uses integer RGB values; there is no color-space conversion,
premultiplication, dithering or image resampling. Quality levels are search budgets,
not guarantees of perceptual similarity or bit-identical output across architectures.

Enable `parallel` for `encode_parallel`, which uses Rayon's current pool. It
produces the same blocks as serial encoding on the same build. Applications that
already parallelize images can use `encode` to avoid nested scheduling.

```rust
fn main() -> Result<(), rgbcx::Error> {
    #[cfg(feature = "parallel")]
    {
        use rgbcx::{encode_parallel, Format, Quality};

        let rgba = [120, 160, 200, 255].repeat(8 * 8);
        let blocks = encode_parallel(8, 8, &rgba, Format::Bc3, Quality::Balanced)?;
        assert_eq!(blocks.len(), Format::Bc3.encoded_len(8, 8)?);
    }
    Ok(())
}
```

## Optional GPU refinement

The `gpu` feature provides `gpu::Refiner`. It runs an additional search over 729
neighboring RGB565 endpoint pairs using integer WGSL. Reuse one instance across
images to amortize device and pipeline creation:

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "gpu")]
    {
        use rgbcx::{encode, gpu::Refiner, Format, Quality};

        let rgba = [120, 160, 200, 255].repeat(8 * 8);
        let mut blocks = encode(8, 8, &rgba, Format::Bc3, Quality::Balanced)?;
        let mut worker = Refiner::new()?;
        worker.refine(8, 8, &rgba, Format::Bc3, &mut blocks)?;
    }
    Ok(())
}
```

`refine` performs the same search on the CPU. Both preserve BC3 alpha,
keep the original block on equal error, and never increase RGB squared error.
BC1 refinement requires opaque input blocks. GPU acceleration applies to this
refinement pass; the initial color fit still runs on the CPU.

`gpu::Error` distinguishes invalid input, unavailable hardware and device failures.
Invalid input leaves output unchanged. A device failure may leave partially updated
blocks; retain the original blocks when implementing a fallback.
Refinement minimizes RGB squared error, not a
perceptual metric.

## Development

Requires Rust 1.98.1 or newer.

```sh
cargo test --features parallel
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features --tests -- --include-ignored
```

The last command requires a hardware compute adapter. Tests cover reference alpha
blocks, partial dimensions, concurrent calls and CPU/GPU equivalence.

For local measurements, provide tightly packed raw RGBA8 bytes, not a PNG:

```sh
cargo run --release --all-features --example profile_encoder -- 512 512 image.rgba
cargo run --release --example encode_file -- 512 512 image.rgba bc3 balanced image.bc
```

The profiler warms up and reports the median of five runs, separating encoding
from additional refinement. GPU initialization is timed separately. Refinement
timings include resetting the output; GPU timings include upload and readback.

## License

The Rust implementation is licensed under [MPL-2.0](./LICENSE). Original third-party
notices remain in [licenses/](licenses/README.md).

## Acknowledgements

The color-fitting algorithms, ordering tables and alpha encoder are based on
[Richard Geldreich's rgbcx](https://github.com/richgel999/bc7enc_rdo), version 1.13.
