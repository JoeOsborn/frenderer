use crate::USE_STORAGE;

#[allow(dead_code)]
pub struct WGPU {
    instance: wgpu::Instance,
    pub(crate) surface: wgpu::Surface,
    adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
}

impl WGPU {
    pub async fn load_texture(
        &self,
        path: &std::path::Path,
        label: Option<&str>,
    ) -> Result<(wgpu::Texture, image::RgbaImage), Box<dyn std::error::Error>> {
        #[cfg(target_arch = "wasm32")]
        let img = {
            let fetch = web_sys::window()
                .map(|win| win.fetch_with_str(path.as_ref().to_str().unwrap()))
                .unwrap();
            let resp: web_sys::Response = wasm_bindgen_futures::JsFuture::from(fetch)
                .await
                .unwrap()
                .into();
            log::debug!("{:?} {:?}", &resp, resp.status());
            let buf: js_sys::ArrayBuffer =
                wasm_bindgen_futures::JsFuture::from(resp.array_buffer().unwrap())
                    .await
                    .unwrap()
                    .into();
            log::debug!("{:?} {:?}", &buf, buf.byte_length());
            let u8arr = js_sys::Uint8Array::new(&buf);
            log::debug!("{:?}, {:?}", &u8arr, u8arr.length());
            let mut bytes = vec![0; u8arr.length() as usize];
            log::debug!("{:?}", &bytes);
            u8arr.copy_to(&mut bytes);
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?
                .to_rgba8()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let img = image::open(path)?.to_rgba8();
        let (width, height) = img.dimensions();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            &img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        Ok((texture, img))
    }

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
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        Self {
            instance,
            surface,
            adapter,
            device,
            queue,
            config,
        }
    }

    pub(crate) fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }
}
