// A square!
var<private> VERTICES:array<vec2<f32>,6> = array<vec2<f32>,6>(
    // Bottom left, bottom right, top left; then top left, bottom right, top right..
    vec2<f32>(0., 0.),
    vec2<f32>(1., 0.),
    vec2<f32>(0., 1.),
    vec2<f32>(0., 1.),
    vec2<f32>(1., 0.),
    vec2<f32>(1., 1.)
);

struct Camera {
    screen_pos: vec2<f32>,
    screen_size: vec2<f32>,
}

struct GPUSprite {
    to_rect:vec4<f32>,
    from_rect:vec4<f32>
}

@group(0) @binding(0)
var<uniform> camera: Camera;
@group(0) @binding(1)
var<storage, read> s_sprites: array<GPUSprite>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_storage_main(@builtin(vertex_index) in_vertex_index: u32, @builtin(instance_index) sprite_index:u32) -> VertexOutput {
    // We'll just look up the vertex data in those constant arrays
    let corner:vec4<f32> = vec4(s_sprites[sprite_index].to_rect.xy,0.,1.);
    let size:vec2<f32> = s_sprites[sprite_index].to_rect.zw;
    let tex_corner:vec2<f32> = s_sprites[sprite_index].from_rect.xy;
    let tex_size:vec2<f32> = s_sprites[sprite_index].from_rect.zw;
    let which_vtx:vec2<f32> = VERTICES[in_vertex_index];
    let which_uv: vec2<f32> = vec2(VERTICES[in_vertex_index].x, 1.0 - VERTICES[in_vertex_index].y);
    return VertexOutput(
        ((corner + vec4(which_vtx*size,0.,0.) - vec4(camera.screen_pos,0.,0.)) / vec4(camera.screen_size/2., 1.0, 1.0)) - vec4(1.0, 1.0, 0.0, 0.0),
        tex_corner + which_uv*tex_size
    );
}

@vertex
fn vs_storage_noinstance_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let sprite_index:u32 = in_vertex_index / u32(6);
    let vertex_index:u32 = in_vertex_index - (sprite_index * u32(6));
    // We'll just look up the vertex data in those constant arrays
    let corner:vec4<f32> = vec4(s_sprites[sprite_index].to_rect.xy,0.,1.);
    let size:vec2<f32> = s_sprites[sprite_index].to_rect.zw;
    let tex_corner:vec2<f32> = s_sprites[sprite_index].from_rect.xy;
    let tex_size:vec2<f32> = s_sprites[sprite_index].from_rect.zw;
    let which_vtx:vec2<f32> = VERTICES[vertex_index];
    let which_uv: vec2<f32> = vec2(VERTICES[vertex_index].x, 1.0 - VERTICES[vertex_index].y);
    return VertexOutput(
        ((corner + vec4(which_vtx*size,0.,0.) - vec4(camera.screen_pos,0.,0.)) / vec4(camera.screen_size/2., 1.0, 1.0)) - vec4(1.0, 1.0, 0.0, 0.0),
        tex_corner + which_uv*tex_size
    );
}

struct InstanceInput {
    @location(0) to_rect: vec4<f32>,
    @location(1) from_rect: vec4<f32>,
};

@vertex
fn vs_vbuf_main(@builtin(vertex_index) in_vertex_index: u32, sprite_data:InstanceInput) -> VertexOutput {
    // We'll still just look up the vertex positions in those constant arrays
    let corner:vec4<f32> = vec4(sprite_data.to_rect.xy,0.,1.);
    let size:vec2<f32> = sprite_data.to_rect.zw;
    let tex_corner:vec2<f32> = sprite_data.from_rect.xy;
    let tex_size:vec2<f32> = sprite_data.from_rect.zw;
    let which_vtx:vec2<f32> = VERTICES[in_vertex_index];
    let which_uv: vec2<f32> = vec2(VERTICES[in_vertex_index].x, 1.0 - VERTICES[in_vertex_index].y);
    return VertexOutput(
        ((corner + vec4(which_vtx*size,0.,0.) - vec4(camera.screen_pos,0.,0.)) / vec4(camera.screen_size/2., 1.0, 1.0)) - vec4(1.0, 1.0, 0.0, 0.0),
        tex_corner + which_uv*tex_size
    );
}


// Now our fragment shader needs two "global" inputs to be bound:
// A texture...
@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
// And a sampler.
@group(1) @binding(1)
var s_diffuse: sampler;
// Both are in the same binding group here since they go together naturally.

// Our fragment shader takes an interpolated `VertexOutput` as input now
@fragment
fn fs_main(in:VertexOutput) -> @location(0) vec4<f32> {
    // And we use the tex coords from the vertex output to sample from the texture.
    let color:vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    if color.w < 0.2 { discard; }
    return color;
}
