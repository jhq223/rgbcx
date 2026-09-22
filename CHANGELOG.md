# Changelog

## 0.1.2

- Reuse encoded bytes for consecutive identical RGBA blocks without changing output.
- Copy complete 4x4 pixel rows directly while preserving partial-edge replication.
- Cover repeated blocks across image rows and parallel batch boundaries.

## 0.1.1

- Cache block projections before sorting and find projection extrema in one pass.
- Simplify partition fitting with exact prefix sums.

## 0.1.0

- BC1, BC3, BC4 and BC5 encoding and decoding.
- Three color-fitting presets and optional Rayon parallelism.
- CPU and GPU endpoint refinement with matching integer scoring.
- Caller-owned RGBA decoding buffers and typed refinement errors.

