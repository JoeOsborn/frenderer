//! [`Renderer`] is the main user-facing type of this crate.  You can
//! make one using [`with_default_runtime()`] or provide your own
//! [`super::Runtime`] implementor via [`Renderer::with_runtime()`].
//! If you don't need frenderer to intiialize `wgpu` for you, you
//! don't need to provide any runtime but can instead use
//! [`Renderer::with_gpu`] to construct a renderer with a given
//! instance, adapter, device, and queue (wrapped in a [`crate::gpu::WGPU`]
//! struct), dimensions, and surface.

use crate::{
    colorgeo::{self, ColorGeo},
    sprites::SpriteRenderer,
    WGPU,
};
use std::ops::{Range, RangeBounds};

pub use crate::meshes::{FlatRenderer, MeshRenderer};
/// A wrapper over GPU state, surface, depth texture, and some renderers.
#[allow(dead_code)]
pub struct Renderer {
    pub gpu: WGPU,
    render_width: u32,
    render_height: u32,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    depth_texture_view: wgpu::TextureView,
    color_texture: wgpu::Texture,
    color_texture_view: wgpu::TextureView,
    // These ones are tracked for auto uploading of assets and automatic rendering.
    // You can make your own renderers and use them for more control.
    sprites: SpriteRenderer,
    meshes: MeshRenderer,
    flats: FlatRenderer,
    postprocess: ColorGeo,
    queued_uploads: Vec<Upload>,
}

#[derive(Debug)]
enum Upload {
    Mesh(crate::meshes::MeshGroup, usize, Range<usize>),
    Flat(crate::meshes::MeshGroup, usize, Range<usize>),
    Sprite(usize, Range<usize>),
}

/// Initialize frenderer with default settings for the current target
/// architecture, including logging via `env_logger` on native or `console_log` on web.
/// On web, this also adds a canvas to the given window.  If you don't need all that behavior,
/// consider using your own [`super::Runtime`].
#[cfg(all(not(target_arch = "wasm32"), feature = "winit"))]
pub fn with_default_runtime(
    builder: winit::window::WindowBuilder,
    render_size: Option<(u32, u32)>,
    callback: impl FnOnce(
        winit::event_loop::EventLoop<()>,
        std::sync::Arc<winit::window::Window>,
        Renderer,
    ),
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    let event_loop = winit::event_loop::EventLoop::new()?;
    let window = Arc::new(builder.build(&event_loop)?);

    env_logger::init();
    let wsz = window.inner_size();
    let sz = render_size.unwrap_or((wsz.width, wsz.height));
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone())?;
    let gpu = pollster::block_on(WGPU::new(&instance, Some(&surface)))?;
    callback(
        event_loop,
        window,
        Renderer::with_gpu(sz.0, sz.1, wsz.width, wsz.height, gpu, surface),
    );
    Ok(())
}
#[cfg(all(target_arch = "wasm32", feature = "winit"))]
pub fn with_default_runtime(
    builder: winit::window::WindowBuilder,
    render_size: Option<(u32, u32)>,
    callback: impl FnOnce(winit::event_loop::EventLoop<()>, std::sync::Arc<winit::window::Window>, Renderer)
        + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    use wgpu::web_sys;
    let event_loop = winit::event_loop::EventLoop::new()?;
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Warn).expect("could not initialize logger");
    use wasm_bindgen_futures::wasm_bindgen::JsCast;
    use winit::platform::web::WindowBuilderExtWebSys;
    // On wasm, append the canvas to the document body
    let canvas = web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| {
            let canvas = doc.create_element("canvas").unwrap();
            doc.body()
                .unwrap()
                .append_child(&canvas)
                .ok()
                .map(|_| canvas)
        })
        .expect("couldn't append canvas to document body")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .ok();
    let window = Arc::new(builder.with_canvas(canvas).build(&event_loop)?);
    let wsz = window.inner_size();
    let sz = render_size.unwrap_or_else(|| (wsz.width, wsz.height));
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone())?;
    wasm_bindgen_futures::spawn_local(async move {
        let gpu = WGPU::new(&instance, Some(&surface)).await.unwrap();
        callback(
            event_loop,
            window,
            Renderer::with_gpu(sz.0, sz.1, wsz.width, wsz.height, gpu, surface),
        );
    });
    Ok(())
}

impl Renderer {
    /// The format used for depth textures within frenderer.
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    /// Create a new Renderer with a full set of GPU resources, a size, and a surface.
    pub fn with_gpu(
        width: u32,
        height: u32,
        surf_width: u32,
        surf_height: u32,
        gpu: crate::gpu::WGPU,
        surface: wgpu::Surface<'static>,
    ) -> Self {
        let width = if width == 0 { 320 } else { width };
        let height = if height == 0 { 240 } else { height };
        if crate::USE_STORAGE {
            let supports_storage_resources = gpu
                .adapter()
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::VERTEX_STORAGE)
                && gpu.device().limits().max_storage_buffers_per_shader_stage > 0;
            assert!(supports_storage_resources, "Storage buffers not supported");
        }
        let swapchain_capabilities = surface.get_capabilities(gpu.adapter());
        let swapchain_format = swapchain_capabilities.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: if surf_width == 0 { width } else { surf_width },
            height: if surf_height == 0 {
                height
            } else {
                surf_height
            },
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![swapchain_format],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(gpu.device(), &config);
        let (color_texture, color_texture_view) =
            Self::create_color_texture(gpu.device(), width, height, swapchain_format);
        let lut = colorgeo::lut_identity(&gpu);
        let postprocess = ColorGeo::new(&gpu, &color_texture, &lut, config.format.into());
        let (depth_texture, depth_texture_view) =
            Self::create_depth_texture(gpu.device(), width, height);

        let intermediate_color_state = wgpu::ColorTargetState {
            format: color_texture.format(),
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent::OVER,
                alpha: wgpu::BlendComponent::OVER,
            }),
            write_mask: wgpu::ColorWrites::ALL,
        };
        let sprites = SpriteRenderer::new(
            &gpu,
            intermediate_color_state.clone(),
            depth_texture.format(),
        );
        let meshes = MeshRenderer::new(
            &gpu,
            intermediate_color_state.clone(),
            depth_texture.format(),
        );
        let flats = FlatRenderer::new(&gpu, intermediate_color_state, depth_texture.format());
        Self {
            gpu,
            render_width: width,
            render_height: height,
            surface,
            config,
            depth_texture,
            depth_texture_view,
            postprocess,
            sprites,
            meshes,
            flats,
            queued_uploads: Vec::with_capacity(16),
            color_texture,
            color_texture_view,
        }
    }
    /// Resize the internal surface and depth textures (typically called when the window or canvas size changes).
    pub fn resize_surface(&mut self, w: u32, h: u32) {
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(self.gpu.device(), &self.config);
    }
    pub fn resize_render(&mut self, w: u32, h: u32) {
        self.render_width = w;
        self.render_height = h;
        let (color_texture, color_texture_view) =
            Self::create_color_texture(self.gpu.device(), w, h, self.config.format);
        self.color_texture = color_texture;
        self.color_texture_view = color_texture_view;
        self.postprocess
            .replace_color_texture(&self.gpu, &self.color_texture);
        let (depth_tex, depth_view) = Self::create_depth_texture(self.gpu.device(), w, h);
        self.depth_texture = depth_tex;
        self.depth_texture_view = depth_view;
    }
    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width,
            height,
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
            view_formats: &[Self::DEPTH_FORMAT],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
    fn create_color_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[format],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Uploads sprite, mesh, and flat data accessed since the last
    /// time [`do_uploads`] was called.  Call this manually if you
    /// want, or let [`render`] call it automatically.
    pub fn do_uploads(&mut self) {
        for upload in self.queued_uploads.drain(..) {
            log::info!("upload: {upload:?}");
            match upload {
                Upload::Mesh(mg, m, r) => self.meshes.upload_meshes(&self.gpu, mg, m, r),
                Upload::Flat(mg, m, r) => self.flats.upload_meshes(&self.gpu, mg, m, r),
                Upload::Sprite(s, r) => self.sprites.upload_sprites(&self.gpu, s, r),
            }
        }
    }

    /// Acquire the next frame, create a [`wgpu::RenderPass`], draw
    /// into it, and submit the encoder.
    /// This also queues uploads of mesh, sprite, or other instance data, so if you don't use render
    /// in your code be sure to call [`do_uploads`] if you're using the built-in mesh, flat, or sprite renderers.
    pub fn render(&mut self) {
        self.do_uploads();
        let (frame, view, mut encoder) = self.render_setup();
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            self.render_into(&mut rpass);
        }
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            self.postprocess.render(&mut rpass);
        }
        self.render_finish(frame, encoder);
    }
    /// Renders all the frenderer stuff into a given
    /// [`wgpu::RenderPass`].  Just does rendering of the built-in
    /// renderers, with no encoder submission or frame
    /// acquire/present.
    pub fn render_into<'s, 'pass>(&'s self, rpass: &mut wgpu::RenderPass<'pass>)
    where
        's: 'pass,
    {
        self.meshes.render(rpass, ..);
        self.flats.render(rpass, ..);
        self.sprites.render(rpass, ..);
    }
    /// Convenience method for acquiring a surface texture, view, and
    /// command encoder
    pub fn render_setup(
        &self,
    ) -> (
        wgpu::SurfaceTexture,
        wgpu::TextureView,
        wgpu::CommandEncoder,
    ) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.view_formats[0]),
            ..Default::default()
        });
        let encoder = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        (frame, view, encoder)
    }
    /// Convenience method for submitting a command encoder and
    /// presenting the swapchain image.
    pub fn render_finish(&self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.gpu.queue().submit(Some(encoder.finish()));
        frame.present();
    }
    /// Creates an array texture on the renderer's GPU.
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
            depth_or_array_layers: if self.gpu.is_gl() {
                // Workaround for opengl: If len is 1, this array texture is just initialized and treated as a regular single texture.  So we lie and say we have at least two (and if we have 6, we lie and say we have 7 so it isn't treated as a cubemap)
                match images.len() {
                    1 => 2,
                    6 => 7,
                    l => l,
                }
            } else {
                images.len()
            } as u32,
        };
        let texture = self.gpu.device().create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, img) in images.iter().enumerate() {
            assert_eq!(
                img.len(),
                images[0].len(),
                "Can't create an array texture with images of different dimensions"
            );
            self.gpu.queue().write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                img,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        // again, if it's opengl we may need to copy our first texture again to the last (bonus) layer index.
        if size.depth_or_array_layers > images.len() as u32 {
            self.gpu.queue().write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: images.len() as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                images[0],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        texture
    }
    /// Creates a single texture on the renderer's GPU.
    pub fn create_texture(
        &self,
        image: &[u8],
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        label: Option<&str>,
    ) -> wgpu::Texture {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.gpu.device().create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.gpu.queue().write_texture(
            texture.as_image_copy(),
            image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        texture
    }

    /// Create a new sprite group sized to fit `world_transforms` and
    /// `sheet_regions`, which should be the same size.  Returns the
    /// sprite group index corresponding to this group.
    pub fn sprite_group_add(
        &mut self,
        tex: &wgpu::Texture,
        world_transforms: Vec<crate::sprites::Transform>,
        sheet_regions: Vec<crate::sprites::SheetRegion>,
        camera: crate::sprites::Camera2D,
    ) -> usize {
        self.sprites
            .add_sprite_group(&self.gpu, tex, world_transforms, sheet_regions, camera)
    }
    /// Returns the number of sprite groups (including placeholders for removed groups).
    pub fn sprite_group_count(&self) -> usize {
        self.sprites.sprite_group_count()
    }
    /// Deletes a sprite group, leaving an empty group slot behind (this might get recycled later).
    pub fn sprite_group_remove(&mut self, which: usize) {
        self.sprites.remove_sprite_group(which)
    }
    /// Reports the size of the given sprite group.  Panics if the given sprite group is not populated.
    pub fn sprite_group_size(&self, which: usize) -> usize {
        self.sprites.sprite_group_size(which)
    }
    /// Resizes a sprite group.  If the new size is smaller, this is
    /// very cheap; if it's larger than it's ever been before, it
    /// might involve reallocating the [`Vec<Transform>`],
    /// [`Vec<SheetRegion>`], or the GPU buffer used to draw sprites,
    /// so it could be expensive.
    ///
    /// Panics if the given sprite group is not populated.
    pub fn sprite_group_resize(&mut self, which: usize, len: usize) -> usize {
        self.sprites.resize_sprite_group(&self.gpu, which, len)
    }
    /// Set the given camera transform on a specific sprite group.  Uploads to the GPU.
    /// Panics if the given sprite group is not populated.
    pub fn sprite_group_set_camera(&mut self, which: usize, camera: crate::sprites::Camera2D) {
        self.sprites.set_camera(&self.gpu, which, camera)
    }
    /// Get a mutable slice of a specified sprite group's world transforms and texture regions.
    /// Marks these sprites for later upload.
    /// Since this causes an upload later on, call it as few times as possible per frame.
    /// Most importantly, don't call it with lots of tiny regions or overlapped regions.
    ///
    /// Panics if the given sprite group is not populated or the range is out of bounds.
    pub fn sprites_mut(
        &mut self,
        which: usize,
        range: impl RangeBounds<usize>,
    ) -> (
        &mut [crate::sprites::Transform],
        &mut [crate::sprites::SheetRegion],
    ) {
        let count = self.sprite_group_size(which);
        let range = crate::range(range, count);
        self.queued_uploads
            .push(Upload::Sprite(which, range.clone()));
        let (trfs, uvs) = self.sprites.get_sprites_mut(which);
        (&mut trfs[range.clone()], &mut uvs[range])
    }

    /// Sets the given camera for all textured mesh groups.
    pub fn mesh_set_camera(&mut self, camera: crate::meshes::Camera3D) {
        self.meshes.set_camera(&self.gpu, camera)
    }
    /// Add a mesh group with the given array texture.
    /// All meshes in the group pull from the same vertex buffer, and each submesh is defined in terms of a range of indices within that buffer.
    /// When loading your mesh resources from whatever format they're stored in, fill out vertex and index vecs while tracking the beginning and end of each mesh and submesh (see [`crate::meshes::MeshEntry`] for details).
    pub fn mesh_group_add(
        &mut self,
        texture: &wgpu::Texture,
        vertices: Vec<crate::meshes::Vertex>,
        indices: Vec<u32>,
        mesh_info: Vec<crate::meshes::MeshEntry>,
    ) -> crate::meshes::MeshGroup {
        self.meshes
            .add_mesh_group(&self.gpu, texture, vertices, indices, mesh_info)
    }
    /// Deletes a mesh group, leaving an empty placeholder.
    pub fn mesh_group_remove(&mut self, which: crate::meshes::MeshGroup) {
        self.meshes.remove_mesh_group(which)
    }
    /// Returns how many mesh groups there are.
    pub fn mesh_group_count(&self) -> usize {
        self.meshes.mesh_group_count()
    }
    /// Returns how many meshes there are in the given mesh group.
    pub fn mesh_group_size(&self, which: crate::meshes::MeshGroup) -> usize {
        self.meshes.mesh_count(which)
    }
    /// Returns how many mesh instances there are in the given mesh of the given mesh group.
    pub fn mesh_instance_count(
        &self,
        which: crate::meshes::MeshGroup,
        mesh_number: usize,
    ) -> usize {
        self.meshes.mesh_instance_count(which, mesh_number)
    }
    /// Change the number of instances of the given mesh of the given mesh group.
    pub fn mesh_instance_resize(
        &mut self,
        which: crate::meshes::MeshGroup,
        idx: usize,
        len: usize,
    ) -> usize {
        self.meshes.resize_group_mesh(&self.gpu, which, idx, len)
    }
    /// Gets the (mutable) transforms of every instance of the given mesh of a mesh group.
    /// Since this causes an upload later on, call it as few times as possible per frame.
    /// Most importantly, don't call it with lots of tiny regions or overlapped regions.
    pub fn meshes_mut(
        &mut self,
        which: crate::meshes::MeshGroup,
        idx: usize,
        range: impl RangeBounds<usize>,
    ) -> &mut [crate::meshes::Transform3D] {
        let count = self.meshes.mesh_instance_count(which, idx);
        let range = crate::range(range, count);
        self.queued_uploads
            .push(Upload::Mesh(which, idx, range.clone()));
        let trfs = self.meshes.get_meshes_mut(which, idx);
        &mut trfs[range]
    }

    /// Sets the given camera for all flat mesh groups.
    pub fn flat_set_camera(&mut self, camera: crate::meshes::Camera3D) {
        self.flats.set_camera(&self.gpu, camera)
    }
    /// Add a flat mesh group with the given color materials.
    /// All meshes in the group pull from the same vertex buffer, and each submesh is defined in terms of a range of indices within that buffer.
    /// When loading your mesh resources from whatever format they're stored in, fill out vertex and index vecs while tracking the beginning and end of each mesh and submesh (see [`crate::meshes::MeshEntry`] for details).
    pub fn flat_group_add(
        &mut self,
        material_colors: &[[f32; 4]],
        vertices: Vec<crate::meshes::FlatVertex>,
        indices: Vec<u32>,
        mesh_info: Vec<crate::meshes::MeshEntry>,
    ) -> crate::meshes::MeshGroup {
        self.flats
            .add_mesh_group(&self.gpu, material_colors, vertices, indices, mesh_info)
    }
    /// Deletes a mesh group, leaving an empty placeholder.
    pub fn flat_group_remove(&mut self, which: crate::meshes::MeshGroup) {
        self.flats.remove_mesh_group(which)
    }
    /// Returns how many mesh groups there are.
    pub fn flat_group_count(&self) -> usize {
        self.flats.mesh_group_count()
    }
    /// Returns how many meshes there are in the given mesh group.
    pub fn flat_group_size(&self, which: crate::meshes::MeshGroup) -> usize {
        self.flats.mesh_count(which)
    }
    /// Returns how many mesh instances there are in the given mesh of the given mesh group.
    pub fn flat_instance_count(
        &self,
        which: crate::meshes::MeshGroup,
        mesh_number: usize,
    ) -> usize {
        self.flats.mesh_instance_count(which, mesh_number)
    }
    /// Change the number of instances of the given mesh of the given mesh group.
    pub fn flat_instance_resize(
        &mut self,
        which: crate::meshes::MeshGroup,
        idx: usize,
        len: usize,
    ) -> usize {
        self.flats.resize_group_mesh(&self.gpu, which, idx, len)
    }
    /// Gets the (mutable) transforms of every instance of the given mesh of a mesh group.
    /// Since this causes an upload later on, call it as few times as possible per frame.
    /// Most importantly, don't call it with lots of tiny regions or overlapped regions.
    pub fn flats_mut(
        &mut self,
        which: crate::meshes::MeshGroup,
        idx: usize,
        range: impl RangeBounds<usize>,
    ) -> &mut [crate::meshes::Transform3D] {
        let count = self.flats.mesh_instance_count(which, idx);
        let range = crate::range(range, count);
        self.queued_uploads
            .push(Upload::Flat(which, idx, range.clone()));
        let trfs = self.flats.get_meshes_mut(which, idx);
        &mut trfs[range]
    }
    pub fn post_transform(&self) -> [f32; 16] {
        self.postprocess.transform()
    }
    pub fn post_color_transform(&self) -> [f32; 16] {
        self.postprocess.color_transform()
    }
    pub fn post_saturation(&self) -> f32 {
        self.postprocess.saturation()
    }
    pub fn post_set(&mut self, trf: [f32; 16], color_trf: [f32; 16], sat: f32) {
        self.postprocess.set_post(&self.gpu, trf, color_trf, sat);
    }
    pub fn post_set_transform(&mut self, trf: [f32; 16]) {
        self.postprocess.set_transform(&self.gpu, trf);
    }
    pub fn post_set_color_transform(&mut self, trf: [f32; 16]) {
        self.postprocess.set_color_transform(&self.gpu, trf);
    }
    pub fn post_set_saturation(&mut self, sat: f32) {
        self.postprocess.set_saturation(&self.gpu, sat);
    }
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }
    pub fn depth_texture(&self) -> &wgpu::Texture {
        &self.depth_texture
    }
    pub fn depth_texture_view(&self) -> &wgpu::TextureView {
        &self.depth_texture_view
    }
}
