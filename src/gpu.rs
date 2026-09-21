// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
//! Optional hardware endpoint search; CPU and GPU share integer scoring and tie rules.
#![forbid(unsafe_code)]
use std::{sync::Mutex, time::Duration};
/// Failure to initialize or execute GPU refinement.
#[derive(Debug)]
pub enum Error {
    /// Invalid pixels, format or destination blocks.
    Input(crate::Error),
    /// No usable hardware adapter or pipeline could be created.
    Unavailable(String),
    /// Device execution, synchronization or readback failed.
    Device(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(error) => error.fmt(f),
            Self::Unavailable(message) => write!(f, "GPU refinement unavailable: {message}"),
            Self::Device(message) => write!(f, "GPU refinement failed: {message}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            _ => None,
        }
    }
}

/// Reusable GPU device, pipeline and buffers for BC1/BC3 endpoint refinement.
pub struct Refiner {
    name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind: wgpu::BindGroup,
    pixels: wgpu::Buffer,
    originals: wgpu::Buffer,
    result: wgpu::Buffer,
    readback: wgpu::Buffer,
    params: wgpu::Buffer,

    device_error: std::sync::Arc<Mutex<Option<String>>>,
}
impl Refiner {
    async fn create() -> Result<Self, String> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            return Err("software GPU adapter rejected".into());
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rgbcx exact selector search"),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let device_error = std::sync::Arc::new(Mutex::new(None));
        let captured = device_error.clone();
        device.on_uncaptured_error(std::sync::Arc::new(move |e| {
            if let Ok(mut error) = captured.lock() {
                *error = Some(e.to_string());
            }
        }));
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rgbcx integer matcher"),
            source: wgpu::ShaderSource::Wgsl(include_str!("refine.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("rgbcx integer matcher"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        use wgpu::BufferUsages as U;
        let pixels = buffer("pixels", 4096 * 16 * 4, U::STORAGE | U::COPY_DST);
        let originals = buffer("originals", 4096 * 2 * 4, U::STORAGE | U::COPY_DST);
        let result = buffer("indices", 4096 * 2 * 4, U::STORAGE | U::COPY_SRC);
        let readback = buffer("readback", 4096 * 2 * 4, U::MAP_READ | U::COPY_DST);
        let params = buffer("params", 16, U::UNIFORM | U::COPY_DST);
        let buffers = [&pixels, &originals, &result, &params];
        let entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(i, b)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgbcx search"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        if let Some(error) = scope.pop().await {
            return Err(error.to_string());
        }
        Ok(Self {
            name: format!("{} ({:?})", info.name, info.backend),
            device,
            queue,
            pipeline,
            bind,
            pixels,
            originals,
            result,
            readback,
            params,

            device_error,
        })
    }
    /// Create one reusable hardware worker. Initialization errors are returned;
    /// callers can explicitly choose CPU refinement instead.
    pub fn new() -> Result<Self, Error> {
        pollster::block_on(Self::create()).map_err(Error::Unavailable)
    }
    /// Hardware adapter name and graphics backend.
    pub fn adapter_name(&self) -> &str {
        &self.name
    }
    /// GPU equivalent of [`crate::refine`]. BC3 alpha bytes are not changed.
    ///
    /// # Errors
    /// Invalid input is rejected before any output is written. Device failures
    /// can leave partial output; discard those blocks before a CPU fallback.
    pub fn refine(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        format: crate::Format,
        blocks: &mut [u8],
    ) -> Result<(), Error> {
        crate::refine::validate(width, height, rgba, format, blocks).map_err(Error::Input)?;
        let result = self.refine_validated(width, rgba, format == crate::Format::Bc3, blocks);
        if result.is_err() {
            self.readback.unmap();
        }
        result.map_err(Error::Device)
    }

    fn refine_validated(
        &mut self,
        width: u32,
        rgba: &[u8],
        alpha: bool,
        blocks: &mut [u8],
    ) -> Result<(), String> {
        if let Some(e) = self
            .device_error
            .lock()
            .map_err(|_| "GPU error lock poisoned")?
            .clone()
        {
            return Err(e);
        }
        let stride = if alpha { 16 } else { 8 };
        let offset = if alpha { 8 } else { 0 };
        let total = blocks.len() / stride;
        let mut pixels = Vec::<u32>::with_capacity(4096 * 16);
        let mut originals = Vec::<u32>::with_capacity(4096 * 2);
        let mut output = vec![0u32; 4096 * 2];
        for first in (0..total).step_by(4096) {
            let count = (total - first).min(4096);
            pixels.clear();
            originals.clear();
            for index in first..first + count {
                pixels.extend(crate::refine::gather(width as usize, rgba, index));
                let at = index * stride + offset;
                originals.push(u32::from_le_bytes(blocks[at..at + 4].try_into().unwrap()));
                originals.push(u32::from_le_bytes(
                    blocks[at + 4..at + 8].try_into().unwrap(),
                ));
            }
            self.queue
                .write_buffer(&self.pixels, 0, bytemuck::cast_slice(&pixels));
            self.queue
                .write_buffer(&self.originals, 0, bytemuck::cast_slice(&originals));
            self.queue.write_buffer(
                &self.params,
                0,
                bytemuck::cast_slice(&[count as u32, alpha as u32, 0, 0]),
            );
            self.dispatch(count as u32, &mut output[..count * 2])?;
            for index in 0..count {
                let at = (first + index) * stride + offset;
                blocks[at..at + 4].copy_from_slice(&output[index * 2].to_le_bytes());
                blocks[at + 4..at + 8].copy_from_slice(&output[index * 2 + 1].to_le_bytes());
            }
        }
        Ok(())
    }
    fn dispatch(&self, groups: u32, output: &mut [u32]) -> Result<(), String> {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("rgbcx endpoint search"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.dispatch_workgroups(groups, 1, 1);
        }
        let bytes = output.len() as u64 * 4;
        encoder.copy_buffer_to_buffer(&self.result, 0, &self.readback, 0, bytes);
        let index = self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..bytes);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: Some(Duration::from_secs(30)),
            })
            .map_err(|e| e.to_string())?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        {
            let mapped = slice.get_mapped_range().map_err(|e| e.to_string())?;
            for (out, word) in output.iter_mut().zip(mapped.as_chunks::<4>().0.iter()) {
                *out = u32::from_le_bytes(*word);
            }
        }
        self.readback.unmap();
        if let Some(error) = self
            .device_error
            .lock()
            .map_err(|_| "GPU error lock poisoned")?
            .clone()
        {
            return Err(error);
        }
        Ok(())
    }
}
