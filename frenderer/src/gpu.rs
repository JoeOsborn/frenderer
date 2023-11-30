//! A wrapper for WGPU state.
//!
//! In a future version of frenderer, this type will be fully public
//! so that it can be provided by client code rather than initialized
//! solely within frenderer.

use std::sync::Arc;

#[derive(Debug)]
enum FrendererError {
    NoUsableAdapter,
}
impl std::fmt::Display for FrendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrendererError::NoUsableAdapter => {
                f.write_str("No valid adapter found for GPU requirements")
            }
        }
    }
}
impl std::error::Error for FrendererError {}

/// A wrapper for a WGPU instance, surface, adapter, device, queue, and surface configuration.
#[allow(dead_code)]
pub struct WGPU {
    adapter: Arc<wgpu::Adapter>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl WGPU {
    pub fn with_resources(
        adapter: Arc<wgpu::Adapter>,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> Self {
        Self {
            adapter,
            device,
            queue,
        }
    }
    pub(crate) async fn new<'inst>(
        instance: &'inst wgpu::Instance,
        surface: Option<&wgpu::Surface<'inst>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("Use storage? {:?}", crate::USE_STORAGE);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: surface,
            })
            .await
            .ok_or(FrendererError::NoUsableAdapter)?;

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: if crate::USE_STORAGE {
                        wgpu::Limits::downlevel_defaults()
                    } else {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    }
                    .using_resolution(adapter.limits()),
                },
                None,
            )
            .await?;
        Ok(Self::with_resources(
            Arc::new(adapter),
            Arc::new(device),
            Arc::new(queue),
        ))
    }
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
