// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use rgbcx::{Format, Quality};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 7 {
        return Err("usage: encode_file WIDTH HEIGHT INPUT.rgba bc1|bc3|bc4|bc5 fast|balanced|high OUTPUT.bc".into());
    }
    let w = args[1].parse()?;
    let h = args[2].parse()?;
    let rgba = std::fs::read(&args[3])?;
    let fmt = match args[4].as_str() {
        "bc1" => Format::Bc1,
        "bc3" => Format::Bc3,
        "bc4" => Format::Bc4,
        "bc5" => Format::Bc5,
        _ => return Err("unknown format".into()),
    };
    let quality = match args[5].as_str() {
        "fast" => Quality::Fast,
        "balanced" => Quality::Balanced,
        "high" => Quality::High,
        _ => return Err("unknown quality".into()),
    };
    let start = std::time::Instant::now();
    let bc = rgbcx::encode(w, h, &rgba, fmt, quality)?;
    println!("encode_ms={:.3}", start.elapsed().as_secs_f64() * 1000.);
    let dec = rgbcx::decode(w, h, &bc, fmt)?;
    let channels = match fmt {
        Format::Bc1 => 3,
        Format::Bc3 => 4,
        Format::Bc4 => 1,
        Format::Bc5 => 2,
    };
    let sse: u64 = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dec.as_chunks::<4>().0.iter())
        .map(|(a, b)| {
            (0..channels)
                .map(|i| (a[i] as i32 - b[i] as i32).pow(2) as u64)
                .sum::<u64>()
        })
        .sum();
    println!("encoded_channel_sse={sse}");
    std::fs::write(&args[6], bc)?;
    Ok(())
}
