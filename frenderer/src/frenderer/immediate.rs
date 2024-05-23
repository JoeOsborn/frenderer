use super::*;

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
    /// Create a new sprite group sized to fit `count_estimate`.
    /// Returns the sprite group index corresponding to this group.
    pub fn sprite_group_add(
        &mut self,
        tex: &wgpu::Texture,
        count_estimate: usize,
        camera: crate::sprites::Camera2D,
    ) -> usize {
        let group_count = self.renderer.sprite_group_add(
            tex,
            vec![crate::sprites::Transform::ZERO; count_estimate],
            vec![crate::sprites::SheetRegion::ZERO; count_estimate],
            camera,
        );
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
    pub fn draw_text(
        &mut self,
        group: usize,
        bitfont: &crate::bitfont::BitFont,
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
impl Frenderer for Immediate {
    fn render(&mut self) {
        Immediate::render(self);
    }
}
