// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use crate::{Quality, tables};

#[derive(Clone, Copy)]
struct Candidate {
    a: u16,
    b: u16,
    selectors: u32,
    error: u32,
    three: bool,
}
fn unpack(v: u16) -> [i32; 3] {
    [(v >> 11) as i32, ((v >> 5) & 63) as i32, (v & 31) as i32]
}
fn pack(v: [i32; 3]) -> u16 {
    ((v[0] << 11) | (v[1] << 5) | v[2]) as u16
}
fn expand(v: u16) -> [i32; 3] {
    let [r, g, b] = unpack(v);
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}
fn quantize(v: [f32; 3]) -> u16 {
    pack(std::array::from_fn(|c| {
        let max = if c == 1 { 63 } else { 31 };
        let value = v[c].clamp(0., 255.);
        let q = (value * max as f32 / 255.) as i32;
        let expanded = |x: i32| {
            if c == 1 {
                (x << 2) | (x >> 4)
            } else {
                (x << 3) | (x >> 2)
            }
        };
        if q < max && value > ((expanded(q) + expanded(q + 1)) as f32 * 0.5) {
            q + 1
        } else {
            q
        }
    }))
}
fn evaluate(p: &[[u8; 4]; 16], mut a: u16, mut b: u16, three: bool, limit: u32) -> Candidate {
    if three {
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
    } else {
        if a < b {
            std::mem::swap(&mut a, &mut b);
        }
        if a == b {
            if b > 0 {
                b -= 1;
            } else {
                a = 1;
            }
        }
    }
    let a_rgb = expand(a);
    let b_rgb = expand(b);
    let pal = [
        a_rgb,
        b_rgb,
        std::array::from_fn(|c| {
            if three {
                (a_rgb[c] + b_rgb[c]) / 2
            } else {
                (2 * a_rgb[c] + b_rgb[c]) / 3
            }
        }),
        std::array::from_fn(|c| (a_rgb[c] + 2 * b_rgb[c]) / 3),
    ];
    let mut result = Candidate {
        a,
        b,
        selectors: 0,
        error: 0,
        three,
    };
    for (i, pixel) in p.iter().enumerate() {
        let mut best = u32::MAX;
        let mut selected = 0;
        for (s, color) in pal.iter().enumerate().take(if three { 3 } else { 4 }) {
            let e = (0..3)
                .map(|c| (i32::from(pixel[c]) - color[c]).pow(2) as u32)
                .sum();
            if e < best {
                best = e;
                selected = s;
            }
        }
        result.error += best;
        result.selectors |= (selected as u32) << (2 * i);
        if result.error >= limit {
            break;
        }
    }
    result
}
fn accept(best: &mut Candidate, trial: Candidate) {
    if trial.error < best.error {
        *best = trial;
    }
}
fn solid(p: &[[u8; 4]; 16], average: [usize; 3], three: bool) -> Candidate {
    let (red, green, blue) = if three {
        (
            tables::MATCH5_3[average[0]],
            tables::MATCH6_3[average[1]],
            tables::MATCH5_3[average[2]],
        )
    } else {
        (
            tables::MATCH5_4[average[0]],
            tables::MATCH6_4[average[1]],
            tables::MATCH5_4[average[2]],
        )
    };
    evaluate(
        p,
        pack([red[0] as i32, green[0] as i32, blue[0] as i32]),
        pack([red[1] as i32, green[1] as i32, blue[1] as i32]),
        three,
        u32::MAX,
    )
}
fn least_squares(p: &[[u8; 4]; 16], best: Candidate) -> Option<(u16, u16)> {
    let scale = if best.three { 2 } else { 3 };
    let weights = if best.three {
        [2, 0, 1, 0]
    } else {
        [3, 0, 2, 1]
    };
    let (mut aa, mut ab, mut bb) = (0, 0, 0);
    let (mut rhs_a, mut rhs_b) = ([0; 3], [0; 3]);
    for (i, pixel) in p.iter().enumerate() {
        let a = weights[((best.selectors >> (i * 2)) & 3) as usize];
        let b = scale - a;
        aa += a * a;
        ab += a * b;
        bb += b * b;
        for c in 0..3 {
            rhs_a[c] += a * i32::from(pixel[c]);
            rhs_b[c] += b * i32::from(pixel[c]);
        }
    }
    let det = aa * bb - ab * ab;
    if det == 0 {
        return None;
    }
    let factor = scale as f32 / det as f32;
    Some((
        quantize(std::array::from_fn(|c| {
            (bb * rhs_a[c] - ab * rhs_b[c]) as f32 * factor
        })),
        quantize(std::array::from_fn(|c| {
            (aa * rhs_b[c] - ab * rhs_a[c]) as f32 * factor
        })),
    ))
}
fn polish(p: &[[u8; 4]; 16], mut best: Candidate, average: [usize; 3]) -> Candidate {
    for _ in 0..2 {
        let trial = match least_squares(p, best) {
            Some((a, b)) => evaluate(p, a, b, best.three, best.error),
            None => solid(p, average, best.three),
        };
        if trial.error >= best.error {
            break;
        }
        best = trial;
    }
    best
}
fn initial(
    p: &[[u8; 4]; 16],
    average: [usize; 3],
    min: [i32; 3],
    max: [i32; 3],
    high: bool,
) -> (u16, u16) {
    let mut cov = [0i32; 6];
    for pixel in p {
        let [r, g, b] = std::array::from_fn(|c| pixel[c] as i32 - average[c] as i32);
        cov[0] += r * r;
        cov[1] += r * g;
        cov[2] += r * b;
        cov[3] += g * g;
        cov[4] += g * b;
        cov[5] += b * b;
    }
    let c = cov.map(|v| v as f32 / 255.);
    let mut axis = std::array::from_fn(|i| (max[i] - min[i]) as f32);
    if cov[2] < 0 {
        axis[0] = -axis[0];
    }
    if cov[4] < 0 {
        axis[1] = -axis[1];
    }
    for _ in 0..if high { 6 } else { 4 } {
        let [r, g, b] = axis;
        axis = [
            r * c[0] + g * c[1] + b * c[2],
            r * c[1] + g * c[3] + b * c[4],
            r * c[2] + g * c[4] + b * c[5],
        ];
    }
    let magnitude = axis.iter().map(|v| v.abs()).fold(0f32, f32::max);
    let axis = if magnitude >= 2. {
        axis.map(|v| (v * 2048. / magnitude) as i32)
    } else {
        [306, 601, 117]
    };
    let projection = |i: usize| ((0..3).map(|c| i32::from(p[i][c]) * axis[c]).sum::<i32>(), i);
    let mut low = projection(0);
    let mut high = low;
    for i in 1..16 {
        let value = projection(i);
        low = low.min(value);
        high = high.max(value);
    }
    let (low, high) = (low.1, high.1);
    (
        quantize(std::array::from_fn(|c| p[low][c] as f32)),
        quantize(std::array::from_fn(|c| p[high][c] as f32)),
    )
}
fn bounding_box(
    p: &[[u8; 4]; 16],
    average: [usize; 3],
    min: [i32; 3],
    max: [i32; 3],
) -> (u16, u16) {
    let mut low = std::array::from_fn(|c| {
        (min[c] as f32 + (max[c] - min[c] - 8) as f32 / 16.).clamp(0., 255.)
    });
    let mut high = std::array::from_fn(|c| {
        (max[c] as f32 - (max[c] - min[c] - 8) as f32 / 16.).clamp(0., 255.)
    });
    for c in 0..2 {
        let cov = p
            .iter()
            .map(|v| (v[c] as i32 - average[c] as i32) * (v[2] as i32 - average[2] as i32))
            .sum::<i32>();
        if cov < 0 {
            std::mem::swap(&mut low[c], &mut high[c]);
        }
    }
    (quantize(low), quantize(high))
}
fn cluster(
    p: &[[u8; 4]; 16],
    mut best: Candidate,
    average: [usize; 3],
    trials: usize,
    iterations: usize,
) -> Candidate {
    // Quantization often maps several partitions to the same endpoint pair.
    // A collision only causes recomputation; it never skips an unseen pair.
    let mut seen = [0u64; 64];
    for _ in 0..iterations {
        let before = best.error;
        let a = expand(best.a);
        let b = expand(best.b);
        let direction = std::array::from_fn::<_, 3, _>(|c| b[c] - a[c]);
        // Compute projections once, rather than in every sort comparison.
        // The source index remains the tie-break, preserving selector order.
        let mut ordered: [(i32, u8); 16] =
            std::array::from_fn(|i| ((0..3).map(|c| p[i][c] as i32 * direction[c]).sum(), i as u8));
        ordered.sort_unstable();
        let mut prefix = [[0i32; 3]; 17];
        for (i, &(_, index)) in ordered.iter().enumerate() {
            prefix[i + 1] = std::array::from_fn(|c| prefix[i][c] + p[index as usize][c] as i32);
        }
        let mut hist = [0usize; 4];
        for i in 0..16 {
            hist[((best.selectors >> (2 * i)) & 3) as usize] += 1;
        }
        let row = if best.three {
            tables::HASH3[hist[0] + 17 * hist[1]] as usize
        } else {
            tables::HASH4[hist[0] + 17 * hist[2] + 289 * hist[3]] as usize
        };
        let scale = if best.three { 2 } else { 3 };
        for q in 0..trials.min(if best.three { 32 } else { 128 }) {
            let h = if best.three {
                let h = tables::ORDERS3[tables::BEST3[row][q] as usize];
                [h[0], h[2], h[1], 0]
            } else {
                tables::ORDERS4[tables::BEST4[row][q] as usize]
            };
            let [n0, n1, n2, n3] = h.map(i32::from);
            let (aa, ab, bb) = if best.three {
                (4 * n0 + n1, n1, n1 + 4 * n2)
            } else {
                (9 * n0 + 4 * n1 + n2, 2 * (n1 + n2), n1 + 4 * n2 + 9 * n3)
            };
            let first = h[0] as usize;
            let second = first + h[1] as usize;
            let third = second + h[2] as usize;
            // Weighted interval sums telescope into the interior prefix sums.
            // All arithmetic stays integral until the original endpoint solve.
            let rhs_a: [i32; 3] = std::array::from_fn(|c| {
                prefix[first][c] + prefix[second][c] + if best.three { 0 } else { prefix[third][c] }
            });
            let rhs_b: [i32; 3] = std::array::from_fn(|c| scale * prefix[16][c] - rhs_a[c]);
            let det = aa * bb - ab * ab;
            let trial = if det == 0 {
                solid(p, average, best.three)
            } else {
                let factor = scale as f32 / det as f32;
                let a = quantize(std::array::from_fn(|c| {
                    (bb * rhs_a[c] - ab * rhs_b[c]) as f32 * factor
                }));
                let b = quantize(std::array::from_fn(|c| {
                    (aa * rhs_b[c] - ab * rhs_a[c]) as f32 * factor
                }));
                let key = u64::from(a.min(b)) | (u64::from(a.max(b)) << 16);
                let slot = ((key ^ (key >> 11) ^ (key >> 22)) & 63) as usize;
                if seen[slot] == key + 1 {
                    continue;
                }
                seen[slot] = key + 1;
                evaluate(p, a, b, best.three, best.error)
            };
            accept(&mut best, trial);
        }
        if best.error == 0 || best.error == before {
            break;
        }
    }
    best
}
fn search(p: &[[u8; 4]; 16], mut best: Candidate) -> Candidate {
    const DIRECTIONS: [[i32; 4]; 16] = [
        [1, 0, 0, 3],
        [0, 1, 0, 4],
        [0, 0, 1, 5],
        [-1, 0, 0, 0],
        [0, -1, 0, 1],
        [0, 0, -1, 2],
        [1, 1, 0, 9],
        [1, 0, 1, 10],
        [0, 1, 1, 11],
        [-1, -1, 0, 6],
        [-1, 0, -1, 7],
        [0, -1, -1, 8],
        [-1, 1, 0, 13],
        [1, -1, 0, 12],
        [0, -1, 1, 15],
        [0, 1, -1, 14],
    ];
    let mut last = 0;
    let mut forbidden = usize::MAX;
    for i in 0..256 {
        if forbidden == (i & 31) {
            continue;
        }
        let mut a = unpack(best.a);
        let mut b = unpack(best.b);
        let target = if i & 16 != 0 { &mut a } else { &mut b };
        let direction = DIRECTIONS[i & 15];
        for c in 0..3 {
            target[c] = (target[c] + direction[c]).clamp(0, if c == 1 { 63 } else { 31 });
        }
        let trial = evaluate(p, pack(a), pack(b), best.three, best.error);
        if trial.error < best.error {
            best = trial;
            last = i;
            forbidden = direction[3] as usize | (i & 16);
        }
        if i - last > 32 {
            break;
        }
    }
    best
}
/// Encode RGB channels; BC1 punch-through transparency is deliberately excluded.
pub fn encode(p: &[[u8; 4]; 16], quality: Quality, allow_three: bool) -> [u8; 8] {
    let average =
        std::array::from_fn(|c| (p.iter().map(|v| v[c] as usize).sum::<usize>() + 8) / 16);
    let three = allow_three && quality != Quality::Fast;
    let mut best = solid(p, average, false);
    if p.iter().all(|v| v[..3] == p[0][..3]) {
        if three {
            accept(&mut best, solid(p, average, true));
        }
    } else {
        let min = std::array::from_fn(|c| p.iter().map(|v| v[c] as i32).min().unwrap());
        let max = std::array::from_fn(|c| p.iter().map(|v| v[c] as i32).max().unwrap());
        let (a, b) = initial(p, average, min, max, quality == Quality::High);
        let seed = evaluate(p, a, b, false, u32::MAX);
        accept(&mut best, polish(p, seed, average));
        if quality == Quality::High {
            let (a, b) = bounding_box(p, average, min, max);
            accept(
                &mut best,
                polish(p, evaluate(p, a, b, false, u32::MAX), average),
            );
        }
        if quality != Quality::Fast {
            best = cluster(
                p,
                best,
                average,
                if quality == Quality::High { 128 } else { 20 },
                if quality == Quality::High { 2 } else { 1 },
            );
        }
        if three {
            let seed = polish(p, evaluate(p, a, b, true, u32::MAX), average);
            let trial = cluster(
                p,
                seed,
                average,
                if quality == Quality::High { 32 } else { 8 },
                1,
            );
            accept(&mut best, trial);
        }
        if quality == Quality::High {
            best = search(p, best);
        }
    }
    let mut out = [0; 8];
    out[..2].copy_from_slice(&best.a.to_le_bytes());
    out[2..4].copy_from_slice(&best.b.to_le_bytes());
    out[4..].copy_from_slice(&best.selectors.to_le_bytes());
    out
}
