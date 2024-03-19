//! [`Renderer`] is the main user-facing type of this crate.  If you
//! want a renderer as quickly as possible, you can use
//! [`crate::Driver`] if you have kept the `winit` feature flag on.
//! If you don't need frenderer to initialize `wgpu` or windowing for
//! you, you can instead use [`Renderer::with_gpu`] to construct a
//! renderer with a given instance, adapter, device, and queue
//! (wrapped in a [`crate::gpu::WGPU`] struct), dimensions, and
//! surface.  [`Renderer`]'s built-in rendering scheme uses off-screen
//! rendering at a given resolution, then a color postprocessing step
//! to produce output on the [`wgpu::Surface`].
//!
//! Besides managing the swapchain, [`Renderer`] also offers
//! facilities for accessing the internal data of a sprite renderer, a
//! textured unlit mesh renderer, and a flat-colored unlit mesh
//! renderer, as well as a color postprocessing step.  Accesses to
//! subsets of their data through [`Renderer`] are recorded for upload
//! before rendering starts; so, any sprite transform data or mesh
//! data accessed through [`Renderer`] will be marked for upload
//! automatically.  This won't always be the most efficient strategy,
//! but you can always create your own
//! [`crate::sprites::SpriteRenderer`] for example and use your own
//! scheme.
//!
//! It is also important to note that you don't actually need to
//! create a [`Renderer`] to use the rendering strategies in this
//! crate.  It's just a convenience.

use crate::{
    colorgeo::{self, ColorGeo},
    sprites::SpriteRenderer,
    WGPU,
};
use std::{
    ops::{Range, RangeBounds},
    sync::Arc,
};

pub use crate::meshes::{FlatRenderer, MeshRenderer};
/// A wrapper over GPU state, surface, depth texture, and some renderers.
#[allow(dead_code)]
pub struct Renderer {
    pub gpu: WGPU,
    render_width: u32,
    render_height: u32,
    surface: Option<wgpu::Surface<'static>>,
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

impl Renderer {
    /// The format used for depth textures within frenderer.
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    /// Creates a [Renderer] and its internal [crate::gpu::WGPU] using a wgpu [wgpu::Instance] and [wgpu::Surface], along with the rendering resolution (`w`, `h`) and surface dimensions.
    pub async fn with_surface(
        width: u32,
        height: u32,
        surf_width: u32,
        surf_height: u32,
        instance: std::sync::Arc<wgpu::Instance>,
        surface: Option<wgpu::Surface<'static>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let gpu = WGPU::new(instance, surface.as_ref()).await?;
        Ok(Self::with_gpu(
            width,
            height,
            surf_width,
            surf_height,
            gpu,
            surface,
        ))
    }
    /// Create a new Renderer with a full set of GPU resources, a
    /// render size (`width`,`height), a surface size, and a surface.
    pub fn with_gpu(
        width: u32,
        height: u32,
        surf_width: u32,
        surf_height: u32,
        gpu: crate::gpu::WGPU,
        surface: Option<wgpu::Surface<'static>>,
    ) -> Self {
        let width = if width == 0 { 320 } else { width };
        let height = if height == 0 { 240 } else { height };
        let swapchain_capabilities = surface
            .as_ref()
            .map(|s| s.get_capabilities(gpu.adapter()))
            .unwrap_or_default();
        let swapchain_format = swapchain_capabilities
            .formats
            .first()
            .unwrap_or(&wgpu::TextureFormat::Rgba8Unorm);
        let swapchain_format_srgb = swapchain_format.add_srgb_suffix();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *swapchain_format,
            width: if surf_width == 0 { width } else { surf_width },
            height: if surf_height == 0 {
                height
            } else {
                surf_height
            },
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![*swapchain_format, swapchain_format_srgb],
            desired_maximum_frame_latency: 2,
        };

        if let Some(surface) = surface.as_ref() {
            surface.configure(gpu.device(), &config)
        };
        let (color_texture, color_texture_view) = Self::create_color_texture(
            gpu.device(),
            width,
            height,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let lut = colorgeo::lut_identity(&gpu);
        let postprocess = ColorGeo::new(&gpu, &color_texture, &lut, swapchain_format_srgb.into());
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
    /// Change the presentation mode used by the swapchain
    pub fn set_present_mode(&mut self, mode: wgpu::PresentMode) {
        self.config.present_mode = mode;
        self.configure_surface();
    }
    /// Returns the current surface
    pub fn surface(&self) -> Option<&wgpu::Surface<'static>> {
        self.surface.as_ref()
    }
    /// Creates a new surface for this renderer
    pub fn create_surface(&mut self, window: Arc<winit::window::Window>) {
        let surface = self.gpu.instance().create_surface(window).unwrap();
        let swapchain_capabilities = surface.get_capabilities(self.gpu.adapter());
        let swapchain_format = swapchain_capabilities.formats[0];
        let swapchain_format_srgb = swapchain_format.add_srgb_suffix();

        self.config = wgpu::SurfaceConfiguration {
            format: swapchain_format,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![swapchain_format, swapchain_format_srgb],
            ..self.config
        };
        self.postprocess.set_color_target(
            &self.gpu,
            (*self.config.view_formats.last().unwrap()).into(),
        );
        self.surface = Some(surface);
        self.configure_surface();
    }
    fn configure_surface(&mut self) {
        if let Some(surface) = self.surface.as_ref() {
            surface.configure(self.gpu.device(), &self.config);
        }
    }
    /// Resize the internal surface texture (typically called when the window or canvas size changes).
    pub fn resize_surface(&mut self, w: u32, h: u32) {
        self.config.width = w;
        self.config.height = h;
        self.configure_surface();
    }
    /// Resize the internal color and depth targets (the actual rendering resolution).
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
    /// time [`Renderer::do_uploads`] was called.  Call this manually if you
    /// want, or let [`Renderer::render`] call it automatically.
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
    /// into it, and submit the encoder.  This also queues uploads of
    /// mesh, sprite, or other instance data, so if you don't use
    /// [`Renderer::render`] in your code be sure to call [`Renderer::do_uploads`] if you're
    /// using the built-in mesh, flat, or sprite renderers.
    pub fn render(&mut self) {
        self.do_uploads();
        let Some((frame, view, mut encoder)) = self.render_setup() else {
            return;
        };
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
    /// renderers, with no data uploads, encoder submission, or frame
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
    /// command encoder.  If this returns `None` it means the surface isn't ready yet.
    pub fn render_setup(
        &self,
    ) -> Option<(
        wgpu::SurfaceTexture,
        wgpu::TextureView,
        wgpu::CommandEncoder,
    )> {
        let Some(surface) = self.surface.as_ref() else {
            println!("render_setup called before surface was ready");
            return None;
        };
        let frame = surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.view_formats[1]),
            ..Default::default()
        });
        let encoder = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        Some((frame, view, encoder))
    }
    /// Convenience method for submitting a command encoder and
    /// presenting the swapchain image.
    pub fn render_finish(&self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.gpu.queue().submit(Some(encoder.finish()));
        frame.present();
    }
    /// Returns the size of the surface onto which the rendered image is stretched
    pub fn surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
    /// Returns the size of the internal rendering texture (i.e., the rendering resolution)
    pub fn render_size(&self) -> (u32, u32) {
        (self.render_width, self.render_height)
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
    /// `sheet_regions`, which should be the same length.  Returns the
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
    /// Most importantly, don't call it with lots of tiny or overlapped regions.
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
        // TODO: should this resize the group to fit?
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
    /// Add a mesh group with the given array texture.  All meshes in
    /// the group pull from the same vertex buffer, and each submesh
    /// is defined in terms of a range of indices within that buffer.
    /// When loading your mesh resources from whatever format they're
    /// stored in, fill out vertex and index vecs while tracking the
    /// beginning and end of each mesh and submesh (see
    /// [`crate::meshes::MeshEntry`] for details).
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
    /// Add a flat mesh group with the given color materials.  All
    /// meshes in the group pull from the same vertex buffer, and each
    /// submesh is defined in terms of a range of indices within that
    /// buffer.  When loading your mesh resources from whatever format
    /// they're stored in, fill out vertex and index vecs while
    /// tracking the beginning and end of each mesh and submesh (see
    /// [`crate::meshes::MeshEntry`] for details).
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
    /// Returns the current geometric transform used in postprocessing (a 4x4 column-major homogeneous matrix)
    pub fn post_transform(&self) -> [f32; 16] {
        self.postprocess.transform()
    }
    /// Returns the current color transform used in postprocessing (a 4x4 column-major homogeneous matrix)
    pub fn post_color_transform(&self) -> [f32; 16] {
        self.postprocess.color_transform()
    }
    /// Returns the current saturation value in postprocessing (a value between -1 and 1, with 0.0 meaning an identity transformation)
    pub fn post_saturation(&self) -> f32 {
        self.postprocess.saturation()
    }
    /// Sets all postprocessing parameters
    pub fn post_set(&mut self, trf: [f32; 16], color_trf: [f32; 16], sat: f32) {
        self.postprocess.set_post(&self.gpu, trf, color_trf, sat);
    }
    /// Sets the postprocessing geometric transform (a 4x4 column-major homogeneous matrix)
    pub fn post_set_transform(&mut self, trf: [f32; 16]) {
        self.postprocess.set_transform(&self.gpu, trf);
    }
    /// Sets the postprocessing color transform (a 4x4 column-major homogeneous matrix)
    pub fn post_set_color_transform(&mut self, trf: [f32; 16]) {
        self.postprocess.set_color_transform(&self.gpu, trf);
    }
    /// Sets the postprocessing saturation value (a number between -1 and 1, with 0.0 meaning an identity transformation)
    pub fn post_set_saturation(&mut self, sat: f32) {
        self.postprocess.set_saturation(&self.gpu, sat);
    }
    /// Sets the postprocessing color lookup table texture
    pub fn post_set_lut(&mut self, lut: &wgpu::Texture) {
        self.postprocess.replace_lut(&self.gpu, lut);
    }
    /// Gets the surface configuration
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }
    /// Gets a reference to the active depth texture
    pub fn depth_texture(&self) -> &wgpu::Texture {
        &self.depth_texture
    }
    /// Gets a view on the active depth texture
    pub fn depth_texture_view(&self) -> &wgpu::TextureView {
        &self.depth_texture_view
    }
}

/// [`Immediate`] wraps a [`Renderer`] with an immediate-mode API with
/// functions like [`Immediate::draw_sprite`].  This API is less
/// modular and may be less efficient, but is simpler for some use
/// cases.
pub struct Immediate {
    pub(crate) renderer: Renderer,
    flats_used: Vec<Vec<usize>>,
    meshes_used: Vec<Vec<usize>>,
    sprites_used: Vec<usize>,
    auto_clear: bool,
}
impl Immediate {
    /// Permanently converts a [Renderer] into an [Immediate].
    pub fn new(renderer: Renderer) -> Self {
        Self {
            auto_clear: true,
            flats_used: (0..(renderer.flat_group_count()))
                .map(|mg| vec![0; renderer.flat_group_size(mg.into())])
                .collect(),
            meshes_used: (0..(renderer.mesh_group_count()))
                .map(|mg| vec![0; renderer.mesh_group_size(mg.into())])
                .collect(),
            sprites_used: vec![0; renderer.sprite_group_count()],
            renderer,
        }
    }
    /// Whether this renderer should clear its counters/state during rendering.  If set to false, it will accumulate drawing commands from multiple frames until [Immediate::clear] is called.
    pub fn auto_clear(&mut self, c: bool) {
        self.auto_clear = c;
    }
    /// Clear the render state.  If done in the middle of a frame this
    /// cancels out earlier draw commands, and if done between frames
    /// (when `auto_clear` is false) will set up the renderer for the
    /// next frame.
    pub fn clear(&mut self) {
        self.sprites_used.fill(0);
        for used_sets in self.meshes_used.iter_mut() {
            used_sets.fill(0);
        }
        for used_sets in self.flats_used.iter_mut() {
            used_sets.fill(0);
        }
    }
    /// Changes the present mode for this renderer
    pub fn set_present_mode(&mut self, mode: wgpu::PresentMode) {
        self.renderer.set_present_mode(mode)
    }
    /// Returns the current surface
    pub fn surface(&self) -> Option<&wgpu::Surface<'static>> {
        self.renderer.surface()
    }
    /// Creates a new surface for this renderer
    pub fn create_surface(&mut self, window: Arc<winit::window::Window>) {
        self.renderer.create_surface(window)
    }
    /// Resize the internal surface texture (typically called when the window or canvas size changes).
    pub fn resize_surface(&mut self, w: u32, h: u32) {
        self.renderer.resize_surface(w, h)
    }
    /// Resize the internal color and depth targets (the actual rendering resolution).
    pub fn resize_render(&mut self, w: u32, h: u32) {
        self.renderer.resize_render(w, h)
    }
    /// Acquire the next frame, create a [`wgpu::RenderPass`], draw
    /// into it, and submit the encoder.  This also queues uploads of
    /// mesh, sprite, or other instance data, so if you don't use
    /// [`Renderer::render`] in your code be sure to call [`Renderer::do_uploads`] if you're
    /// using the built-in mesh, flat, or sprite renderers.
    pub fn render(&mut self) {
        // upload affected ranges
        for (sg, used) in self.sprites_used.iter_mut().enumerate() {
            self.renderer
                .sprites
                .resize_sprite_group(&self.renderer.gpu, sg, *used);
            self.renderer
                .sprites
                .upload_sprites(&self.renderer.gpu, sg, 0..*used);
        }
        for (mg_idx, used_sets) in self.meshes_used.iter_mut().enumerate() {
            for (mesh_idx, used) in used_sets.iter_mut().enumerate() {
                self.renderer.meshes.resize_group_mesh(
                    &self.renderer.gpu,
                    mg_idx.into(),
                    mesh_idx,
                    *used,
                );
                self.renderer.meshes.upload_meshes(
                    &self.renderer.gpu,
                    mg_idx.into(),
                    mesh_idx,
                    0..*used,
                );
            }
        }
        for (mg_idx, used_sets) in self.flats_used.iter_mut().enumerate() {
            for (mesh_idx, used) in used_sets.iter_mut().enumerate() {
                self.renderer.flats.resize_group_mesh(
                    &self.renderer.gpu,
                    mg_idx.into(),
                    mesh_idx,
                    *used,
                );
                self.renderer.flats.upload_meshes(
                    &self.renderer.gpu,
                    mg_idx.into(),
                    mesh_idx,
                    0..*used,
                );
            }
        }
        self.renderer.render();
        if self.auto_clear {
            self.clear();
        }
    }
    /// Returns the size of the surface onto which the rendered image is stretched
    pub fn surface_size(&self) -> (u32, u32) {
        self.renderer.surface_size()
    }
    /// Returns the size of the internal rendering texture (i.e., the rendering resolution)
    pub fn render_size(&self) -> (u32, u32) {
        self.renderer.render_size()
    }
    /// Creates an array texture on the renderer's GPU.
    pub fn create_array_texture(
        &self,
        images: &[&[u8]],
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        label: Option<&str>,
    ) -> wgpu::Texture {
        self.renderer
            .create_array_texture(images, format, (width, height), label)
    }
    /// Creates a single texture on the renderer's GPU.
    pub fn create_texture(
        &self,
        image: &[u8],
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        label: Option<&str>,
    ) -> wgpu::Texture {
        self.renderer
            .create_texture(image, format, (width, height), label)
    }
    /// Create a new sprite group sized to fit `world_transforms` and
    /// `sheet_regions`, which should be the same length.  Returns the
    /// sprite group index corresponding to this group.
    pub fn sprite_group_add(
        &mut self,
        tex: &wgpu::Texture,
        world_transforms: Vec<crate::sprites::Transform>,
        sheet_regions: Vec<crate::sprites::SheetRegion>,
        camera: crate::sprites::Camera2D,
    ) -> usize {
        let group_count =
            self.renderer
                .sprite_group_add(tex, world_transforms, sheet_regions, camera);
        self.sprites_used.resize(group_count + 1, 0);
        group_count
    }
    /// Returns the number of sprite groups (including placeholders for removed groups).
    pub fn sprite_group_count(&self) -> usize {
        self.renderer.sprite_group_count()
    }
    /// Deletes a sprite group, leaving an empty group slot behind (this might get recycled later).
    pub fn sprite_group_remove(&mut self, which: usize) {
        self.renderer.sprite_group_remove(which)
    }
    /// Reports the size of the given sprite group.  Panics if the given sprite group is not populated.
    pub fn sprite_group_size(&self, which: usize) -> usize {
        self.renderer.sprite_group_size(which)
    }
    /// Makes sure that the size of the given sprite group is at least as large as num.
    pub fn ensure_sprites_size(&mut self, which: usize, num: usize) {
        if self.renderer.sprites.sprite_group_size(which) <= num {
            self.renderer.sprites.resize_sprite_group(
                &self.renderer.gpu,
                which,
                (num + 1).next_power_of_two(),
            );
        }
    }
    /// Set the given camera transform on a specific sprite group.  Uploads to the GPU.
    /// Panics if the given sprite group is not populated.
    pub fn sprite_group_set_camera(&mut self, which: usize, camera: crate::sprites::Camera2D) {
        self.renderer.sprite_group_set_camera(which, camera)
    }
    /// Draws a sprite with the given transform and sheet region
    pub fn draw_sprite(
        &mut self,
        group: usize,
        transform: crate::sprites::Transform,
        sheet_region: crate::sprites::SheetRegion,
    ) {
        let old_count = self.sprites_used[group];
        self.ensure_sprites_size(group, old_count + 1);
        let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(group);
        trfs[old_count] = transform;
        uvs[old_count] = sheet_region;
        self.sprites_used[group] += 1;
    }
    /// Gets a block of `howmany` sprites to draw into, as per [Renderer::get_sprites_mut]
    pub fn draw_sprites(
        &mut self,
        group: usize,
        howmany: usize,
    ) -> (
        &mut [crate::sprites::Transform],
        &mut [crate::sprites::SheetRegion],
    ) {
        let old_count = self.sprites_used[group];
        self.ensure_sprites_size(group, old_count + howmany);
        let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(group);
        let trfs = &mut trfs[old_count..(old_count + howmany)];
        let uvs = &mut uvs[old_count..(old_count + howmany)];
        trfs.fill(crate::sprites::Transform::ZERO);
        uvs.fill(crate::sprites::SheetRegion::ZERO);
        self.sprites_used[group] += howmany;
        (trfs, uvs)
    }

    /// Draws a line of text with the given [`crate::bitfont::BitFont`].
    pub fn draw_text<B: RangeBounds<char>>(
        &mut self,
        group: usize,
        bitfont: &crate::bitfont::BitFont<B>,
        text: &str,
        screen_pos: [f32; 2],
        depth: u16,
        char_height: f32,
    ) -> ([f32; 2], usize) {
        let (trfs, uvs) = self.draw_sprites(group, text.len());
        let (corner, used) = bitfont.draw_text(trfs, uvs, text, screen_pos, depth, char_height);
        (corner, used)
    }
    /// Draws the sprites of a [`crate::nineslice::NineSlice`].
    #[allow(clippy::too_many_arguments)]
    pub fn draw_nineslice(
        &mut self,
        group: usize,
        ninesl: &crate::nineslice::NineSlice,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        z_offset: u16,
    ) -> usize {
        let (trfs, uvs) = self.draw_sprites(group, ninesl.sprite_count(w, h));
        ninesl.draw(trfs, uvs, x, y, w, h, z_offset)
    }

    /// Sets the given camera for all textured mesh groups.
    pub fn mesh_set_camera(&mut self, camera: crate::meshes::Camera3D) {
        self.renderer.mesh_set_camera(camera)
    }
    /// Add a mesh group with the given array texture.  All meshes in
    /// the group pull from the same vertex buffer, and each submesh
    /// is defined in terms of a range of indices within that buffer.
    /// When loading your mesh resources from whatever format they're
    /// stored in, fill out vertex and index vecs while tracking the
    /// beginning and end of each mesh and submesh (see
    /// [`crate::meshes::MeshEntry`] for details).
    /// Sets the given camera for all flat mesh groups.
    pub fn mesh_group_add(
        &mut self,
        texture: &wgpu::Texture,
        vertices: Vec<crate::meshes::Vertex>,
        indices: Vec<u32>,
        mesh_info: Vec<crate::meshes::MeshEntry>,
    ) -> crate::meshes::MeshGroup {
        let mesh_count = mesh_info.len();
        let group = self
            .renderer
            .mesh_group_add(texture, vertices, indices, mesh_info);
        self.meshes_used.resize(group.index() + 1, vec![]);
        self.meshes_used[group.index()].resize(mesh_count, 0);
        group
    }
    /// Deletes a mesh group, leaving an empty placeholder.
    pub fn mesh_group_remove(&mut self, which: crate::meshes::MeshGroup) {
        self.renderer.mesh_group_remove(which)
    }
    /// Returns how many mesh groups there are.
    pub fn mesh_group_count(&self) -> usize {
        self.renderer.mesh_group_count()
    }
    /// Returns how many meshes there are in the given mesh group.
    pub fn mesh_group_size(&self, which: crate::meshes::MeshGroup) -> usize {
        self.renderer.mesh_group_size(which)
    }
    /// Makes sure that the mesh instance slice for the given mesh group and index is at least big enough to hold `num`.
    pub fn ensure_meshes_size(&mut self, which: crate::meshes::MeshGroup, idx: usize, num: usize) {
        if self.renderer.meshes.mesh_instance_count(which, idx) <= num {
            self.renderer.meshes.resize_group_mesh(
                &self.renderer.gpu,
                which,
                idx,
                (num + 1).next_power_of_two(),
            );
        }
    }
    /// Draws a textured, unlit mesh with the given [`crate::meshes::Transform3D`].
    pub fn draw_mesh(
        &mut self,
        which: crate::meshes::MeshGroup,
        idx: usize,
        trf: crate::meshes::Transform3D,
    ) {
        let old_count = self.meshes_used[which.index()][idx];
        self.ensure_meshes_size(which, idx, old_count + 1);
        let trfs = self.renderer.meshes.get_meshes_mut(which, idx);
        trfs[old_count] = trf;
        self.meshes_used[which.index()][idx] += 1;
    }
    /// Gets a block of `howmany` mesh instances to draw into, as per [Renderer::get_meshes_mut]
    pub fn draw_meshes(
        &mut self,
        group: crate::meshes::MeshGroup,
        idx: usize,
        howmany: usize,
    ) -> &mut [crate::meshes::Transform3D] {
        let old_count = self.meshes_used[group.index()][idx];
        self.ensure_meshes_size(group, idx, old_count + howmany);
        let trfs = self.renderer.meshes.get_meshes_mut(group, idx);
        let trfs = &mut trfs[old_count..(old_count + howmany)];
        trfs.fill(crate::meshes::Transform3D::ZERO);
        self.meshes_used[group.index()][idx] += howmany;
        trfs
    }
    /// Sets the given camera for all flat mesh groups.
    pub fn flat_set_camera(&mut self, camera: crate::meshes::Camera3D) {
        self.renderer.flat_set_camera(camera)
    }
    /// Add a flat mesh group with the given color materials.  All
    /// meshes in the group pull from the same vertex buffer, and each
    /// submesh is defined in terms of a range of indices within that
    /// buffer.  When loading your mesh resources from whatever format
    /// they're stored in, fill out vertex and index vecs while
    /// tracking the beginning and end of each mesh and submesh (see
    /// [`crate::meshes::MeshEntry`] for details).
    pub fn flat_group_add(
        &mut self,
        material_colors: &[[f32; 4]],
        vertices: Vec<crate::meshes::FlatVertex>,
        indices: Vec<u32>,
        mesh_info: Vec<crate::meshes::MeshEntry>,
    ) -> crate::meshes::MeshGroup {
        let mesh_count = mesh_info.len();
        let group = self
            .renderer
            .flat_group_add(material_colors, vertices, indices, mesh_info);
        self.flats_used.resize(group.index() + 1, vec![]);
        self.flats_used[group.index()].resize(mesh_count, 0);
        group
    }
    /// Deletes a mesh group, leaving an empty placeholder.
    pub fn flat_group_remove(&mut self, which: crate::meshes::MeshGroup) {
        self.renderer.flat_group_remove(which)
    }
    /// Returns how many mesh groups there are.
    pub fn flat_group_count(&self) -> usize {
        self.renderer.flat_group_count()
    }
    /// Returns how many meshes there are in the given mesh group.
    pub fn flat_group_size(&self, which: crate::meshes::MeshGroup) -> usize {
        self.renderer.flat_group_size(which)
    }
    /// Makes sure that the flats instance slice for the given mesh group and index is at least big enough to hold `num`.
    pub fn ensure_flats_size(&mut self, which: crate::meshes::MeshGroup, idx: usize, num: usize) {
        if self.renderer.flats.mesh_instance_count(which, idx) <= num {
            self.renderer.flats.resize_group_mesh(
                &self.renderer.gpu,
                which,
                idx,
                (num + 1).next_power_of_two(),
            );
        }
    }
    /// Draws a flat mesh (of the given group and mesh index) with the given [`crate::meshes::Transform3D`].
    pub fn draw_flat(
        &mut self,
        which: crate::meshes::MeshGroup,
        idx: usize,
        trf: crate::meshes::Transform3D,
    ) {
        let old_count = self.flats_used[which.index()][idx];
        self.ensure_flats_size(which, idx, old_count + 1);
        let trfs = self.renderer.flats.get_meshes_mut(which, idx);
        trfs[old_count] = trf;
        self.flats_used[which.index()][idx] += 1;
    }
    /// Gets a block of `howmany` flatmesh instances to draw into, as per [Renderer::get_flats_mut]
    pub fn draw_flats(
        &mut self,
        group: crate::meshes::MeshGroup,
        idx: usize,
        howmany: usize,
    ) -> &mut [crate::meshes::Transform3D] {
        let old_count = self.flats_used[group.index()][idx];
        self.ensure_flats_size(group, idx, old_count + howmany);
        let trfs = self.renderer.flats.get_meshes_mut(group, idx);
        let trfs = &mut trfs[old_count..(old_count + howmany)];
        trfs.fill(crate::meshes::Transform3D::ZERO);
        self.flats_used[group.index()][idx] += howmany;
        trfs
    }
    /// Returns the current geometric transform used in postprocessing (a 4x4 column-major homogeneous matrix)
    pub fn post_transform(&self) -> [f32; 16] {
        self.renderer.post_transform()
    }
    /// Returns the current color transform used in postprocessing (a 4x4 column-major homogeneous matrix)
    pub fn post_color_transform(&self) -> [f32; 16] {
        self.renderer.post_color_transform()
    }
    /// Returns the current saturation value in postprocessing (a value between -1 and 1, with 0.0 meaning an identity transformation)
    pub fn post_saturation(&self) -> f32 {
        self.renderer.post_saturation()
    }
    /// Sets all postprocessing parameters
    pub fn post_set(&mut self, trf: [f32; 16], color_trf: [f32; 16], sat: f32) {
        self.renderer.post_set(trf, color_trf, sat)
    }
    /// Sets the postprocessing geometric transform (a 4x4 column-major homogeneous matrix)
    pub fn post_set_transform(&mut self, trf: [f32; 16]) {
        self.renderer.post_set_transform(trf)
    }
    /// Sets the postprocessing color transform (a 4x4 column-major homogeneous matrix)
    pub fn post_set_color_transform(&mut self, trf: [f32; 16]) {
        self.renderer.post_set_color_transform(trf)
    }
    /// Sets the postprocessing saturation value (a number between -1 and 1, with 0.0 meaning an identity transformation)
    pub fn post_set_saturation(&mut self, sat: f32) {
        self.renderer.post_set_saturation(sat)
    }
    /// Sets the postprocessing color lookup table texture
    pub fn post_set_lut(&mut self, lut: &wgpu::Texture) {
        self.renderer.post_set_lut(lut)
    }
    /// Gets the surface configuration
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        self.renderer.config()
    }
    /// Gets a reference to the active depth texture
    pub fn depth_texture(&self) -> &wgpu::Texture {
        self.renderer.depth_texture()
    }
    /// Gets a view on the active depth texture
    pub fn depth_texture_view(&self) -> &wgpu::TextureView {
        self.renderer.depth_texture_view()
    }
    /// Get the GPU from the inner renderer
    pub fn gpu(&self) -> &WGPU {
        &self.renderer.gpu
    }
}

impl std::convert::From<Renderer> for Immediate {
    fn from(rend: Renderer) -> Self {
        Immediate::new(rend)
    }
}

pub trait Frenderer {
    fn render(&mut self);
}
impl Frenderer for Immediate {
    fn render(&mut self) {
        Immediate::render(self);
    }
}
impl Frenderer for Renderer {
    fn render(&mut self) {
        Renderer::render(self);
    }
}
