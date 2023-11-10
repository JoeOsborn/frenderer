//! A wrapper for WGPU state.
//!
//! In a future version of frenderer, this type will be fully public
//! so that it can be provided by client code rather than initialized
//! solely within frenderer.

use crate::USE_STORAGE;

/// A wrapper for a WGPU instance, surface, adapter, device, queue, and surface configuration.
#[allow(dead_code)]
pub struct WGPU {
    instance: wgpu::Instance,
    pub surface: wgpu::Surface,
    adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::Texture,
    pub depth_texture_view: wgpu::TextureView,
}

impl WGPU {
    pub fn create_array_texture(
        &self,
        images: &[&[u8]],
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        label: Option<&str>,
    ) -> wgpu::Texture {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: images.len() as u32,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if images.len() == 1 {
            self.queue.write_texture(
                texture.as_image_copy(),
                images[0],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                size,
            );
        } else {
            let image_combined_len: usize = images.iter().map(|img| img.len()).sum();
            let mut staging = Vec::with_capacity(image_combined_len);
            for img in images {
                staging.extend_from_slice(img);
            }
            // TODO Fixme this will also make a copy, it might be better to do multiple write_texture calls or else take an images[] slice which is already dense in memory
            self.queue.write_texture(
                texture.as_image_copy(),
                &staging,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                size,
            );
        }
        texture
    }
    pub fn create_texture(
        &self,
        image: &[u8],
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        label: Option<&str>,
    ) -> wgpu::Texture {
        self.create_array_texture(&[image], format, (width, height), label)
    }
    /// Initialize [`wgpu`] with the given [`winit::window::Window`].
    pub(crate) async fn new(window: &winit::window::Window) -> Self {
        let size = window.inner_size();

        log::info!("Use storage? {:?}", USE_STORAGE);

        let instance = wgpu::Instance::default();

        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: if USE_STORAGE {
                        wgpu::Limits::downlevel_defaults()
                    } else {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    }
                    .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        if USE_STORAGE {
            let supports_storage_resources = adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::VERTEX_STORAGE)
                && device.limits().max_storage_buffers_per_shader_stage > 0;
            assert!(supports_storage_resources, "Storage buffers not supported");
        }
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);
        let (depth_texture, depth_texture_view) = Self::create_depth_texture(&device, &config);
        Self {
            instance,
            surface,
            adapter,
            device,
            queue,
            config,
            depth_texture,
            depth_texture_view,
        }
    }
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
    /// Resize the WGPU surface
    pub(crate) fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        let (depth_tex, depth_view) = Self::create_depth_texture(&self.device, &self.config);
        self.depth_texture = depth_tex;
        self.depth_texture_view = depth_view;
    }
}
