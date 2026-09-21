// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
use rgbcx::{Format, Quality};
use std::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn measure<T>(label: &str, mut run: impl FnMut() -> Result<T>) -> Result<T> {
    let mut result = run()?; // Warm up before collecting samples.
    let mut samples = [0.0; 5];
    for sample in &mut samples {
        let start = Instant::now();
        let next = run()?;
        *sample = start.elapsed().as_secs_f64() * 1000.0;
        result = next;
    }
    samples.sort_by(f64::total_cmp);
    println!("{label}_median_ms={:.3}", samples[2]);
    Ok(result)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: profile_encoder WIDTH HEIGHT INPUT.rgba".into());
    }
    let width = args[1].parse()?;
    let height = args[2].parse()?;
    let rgba = std::fs::read(&args[3])?;
    let format = if rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255) {
        Format::Bc3
    } else {
        Format::Bc1
    };
    println!("format={format:?} quality=Balanced samples=5");
    let encoded = measure("encode_serial", || {
        Ok(rgbcx::encode(
            width,
            height,
            &rgba,
            format,
            Quality::Balanced,
        )?)
    })?;
    #[cfg(feature = "parallel")]
    {
        let parallel = measure("encode_parallel", || {
            Ok(rgbcx::encode_parallel(
                width,
                height,
                &rgba,
                format,
                Quality::Balanced,
            )?)
        })?;
        if parallel != encoded {
            return Err("parallel encoding differs from serial encoding".into());
        }
    }
    let mut cpu = encoded.clone();
    measure("refine_cpu", || {
        cpu.copy_from_slice(&encoded);
        Ok(rgbcx::refine(width, height, &rgba, format, &mut cpu)?)
    })?;
    #[cfg(feature = "gpu")]
    {
        let start = Instant::now();
        let mut gpu = rgbcx::gpu::Refiner::new()?;
        println!(
            "gpu_init_ms={:.3} adapter={}",
            start.elapsed().as_secs_f64() * 1000.0,
            gpu.adapter_name()
        );
        let mut actual = encoded.clone();
        measure("refine_gpu", || {
            actual.copy_from_slice(&encoded);
            Ok(gpu.refine(width, height, &rgba, format, &mut actual)?)
        })?;
        if actual != cpu {
            return Err("GPU refinement differs from CPU refinement".into());
        }
    }
    Ok(())
}
