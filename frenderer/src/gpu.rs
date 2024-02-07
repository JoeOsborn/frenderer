//! A wrapper for WGPU state.

use std::sync::Arc;

#[derive(Debug)]
pub enum FrendererError {
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
    instance: Arc<wgpu::Instance>,
    adapter: Arc<wgpu::Adapter>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl WGPU {
    /// Create a WGPU structure with already-created GPU resources.
    pub fn with_resources(
        instance: Arc<wgpu::Instance>,
        adapter: Arc<wgpu::Adapter>,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> Self {
        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }
    /// Create a WGPU structure by initializing WGPU for display onto the given surface.
    pub async fn new(
        instance: Arc<wgpu::Instance>,
        surface: Option<&wgpu::Surface<'static>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: surface,
            })
            .await
            .ok_or(FrendererError::NoUsableAdapter)?;
        let is_gl = adapter.get_info().backend == wgpu::Backend::Gl;
        #[cfg(not(target_arch = "wasm32"))]
        let is_web = false;
        #[cfg(target_arch = "wasm32")]
        let is_web = true;
        let use_storage = !(is_web && is_gl);

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: if use_storage {
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
            instance,
            Arc::new(adapter),
            Arc::new(device),
            Arc::new(queue),
        ))
    }
    /// Returns true if this GPU interface is using a GL backend, important to work around some bugs
    pub fn is_gl(&self) -> bool {
        self.adapter.get_info().backend == wgpu::Backend::Gl
    }
    /// Returns true if this GPU interface is in web mode
    #[cfg(target_arch = "wasm32")]
    pub fn is_web(&self) -> bool {
        true
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_web(&self) -> bool {
        false
    }
    /// Whether this GPU supports storage buffers
    pub fn supports_storage(&self) -> bool {
        !(self.is_gl() && self.is_web())
            && self
                .adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::VERTEX_STORAGE)
            && self.device.limits().max_storage_buffers_per_shader_stage > 0
    }
    /// Returns this GPU wrapper's [`wgpu::Instance`].
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    /// Returns this GPU wrapper's [`wgpu::Adapter`].
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }
    /// Returns this GPU wrapper's [`wgpu::Device`].
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
    /// Returns this GPU wrapper's [`wgpu::Queue`].
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
