//! Metal backend abstraction.

use crate::error::{GpuError, Result};
use metal::*;

/// Metal device wrapper.
#[derive(Clone)]
pub struct MetalDevice {
    device: Device,
    name: String,
}

impl MetalDevice {
    /// Create a new Metal device with default system device.
    pub fn new() -> Result<Self> {
        let device = Device::system_default()
            .ok_or_else(|| GpuError::Metal("No Metal-compatible GPU found".to_string()))?;
        let name = device.name().to_string();
        Ok(Self { device, name })
    }

    /// Create a Metal device with specific preference.
    pub fn with_preference(preference: crate::config::MetalDevicePreference) -> Result<Self> {
        let devices = Device::all();
        match preference {
            crate::config::MetalDevicePreference::Auto => Device::system_default()
                .ok_or_else(|| GpuError::Metal("No default Metal device".to_string()))
                .map(|device| Self {
                    name: device.name().to_string(),
                    device,
                }),
            crate::config::MetalDevicePreference::Discrete => {
                // Try to find a discrete GPU
                let discrete = devices.iter().find(|d| !d.is_low_power());
                discrete
                    .or_else(|| devices.first())
                    .map(|device| Self {
                        name: device.name().to_string(),
                        device: device.clone(),
                    })
                    .ok_or_else(|| GpuError::Metal("No Metal devices found".to_string()))
            }
            crate::config::MetalDevicePreference::Integrated => {
                // Try to find an integrated GPU
                let integrated = devices.iter().find(|d| d.is_low_power());
                integrated
                    .or_else(|| devices.first())
                    .map(|device| Self {
                        name: device.name().to_string(),
                        device: device.clone(),
                    })
                    .ok_or_else(|| GpuError::Metal("No Metal devices found".to_string()))
            }
        }
    }

    /// Get the underlying Metal device.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get the device name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if device is low power (integrated).
    pub fn is_low_power(&self) -> bool {
        self.device.is_low_power()
    }

    /// Check if device is removable (external GPU).
    pub fn is_removable(&self) -> bool {
        self.device.is_removable()
    }

    /// Get recommended maximum working set size in bytes.
    pub fn recommended_max_working_set_size(&self) -> u64 {
        self.device.recommended_max_working_set_size()
    }

    /// Create a new Metal command queue.
    pub fn new_command_queue(&self) -> CommandQueue {
        self.device.new_command_queue()
    }
}

/// Metal buffer wrapper for GPU memory.
pub struct MetalBuffer {
    buffer: Buffer,
    length: usize,
}

impl MetalBuffer {
    /// Create a new Metal buffer with data.
    pub fn new(device: &MetalDevice, data: &[u8]) -> Self {
        let buffer = device.device().new_buffer_with_data(
            data.as_ptr() as *const _,
            data.len() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache | MTLResourceOptions::StorageModeShared,
        );
        Self {
            buffer,
            length: data.len(),
        }
    }

    /// Get the underlying Metal buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Get buffer length in bytes.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}
