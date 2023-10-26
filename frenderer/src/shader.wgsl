// A square!
var<private> VERTICES:array<vec2<f32>,6> = array<vec2<f32>,6>(
    // Bottom left, bottom right, top left; then top left, bottom right, top right.
    vec2<f32>(-0.5, -0.5),
    vec2<f32>(0.5, -0.5),
    vec2<f32>(-0.5, 0.5),
    vec2<f32>(-0.5, 0.5),
    vec2<f32>(0.5, -0.5),
    vec2<f32>(0.5, 0.5)
);

struct Camera {
    screen_pos: vec2<f32>,
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;
@group(0) @binding(1)
var<storage, read> s_world: array<vec4<f32>>;
@group(0) @binding(2)
var<storage, read> s_sheet: array<vec4<f32>>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

fn sprite_to_vert(trf:vec4<f32>, uvs:vec4<f32>, norm_vert:vec2<f32>) -> VertexOutput {
  let center:vec2<f32> = trf.yz;
  let size_bits:u32 = bitcast<u32>(trf.x);
  let size:vec2<f32> = vec2(f32(size_bits & 0x0000FFFFu),
                            f32((size_bits & 0xFFFF0000u) >> 16u)
                            );
  let rot:f32 = trf.w;
  let sinrot:f32 = sin(rot);
  let cosrot:f32 = cos(rot);
  // scale
  var scaled = norm_vert*size;
  var rotated = vec2(
                     scaled.x*cosrot-scaled.y*sinrot,
                     scaled.x*sinrot+scaled.y*cosrot
                     );
  // now translate by trf (center, size):
  let world_pos = (center) + rotated;
  let camera_pos = world_pos - camera.screen_pos;
  let box_pos = camera_pos / (camera.screen_size*0.5);
  let ndc_pos = vec4(box_pos.xy, 0.0, 1.0) - vec4(1.0, 1.0, 0.0, 0.0);
  let tex_corner = uvs.xy;
  let tex_size = uvs.zw;
  let norm_uv = vec2(norm_vert.x+0.5, 1.0-(norm_vert.y+0.5));
  return VertexOutput(ndc_pos, tex_corner + norm_uv*tex_size);
}

@vertex
fn vs_storage_main(@builtin(vertex_index) in_vertex_index: u32, @builtin(instance_index) sprite_index:u32) -> VertexOutput {
  // We'll just look up the vertex data in those constant arrays
  let trf = s_world[sprite_index];
  let uvs = s_sheet[sprite_index];
  return sprite_to_vert(trf, uvs, VERTICES[in_vertex_index]);
}

@vertex
fn vs_storage_noinstance_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let sprite_index:u32 = in_vertex_index / u32(6);
    let vertex_index:u32 = in_vertex_index - (sprite_index * u32(6));
    let trf = s_world[sprite_index];
    let uvs = s_sheet[sprite_index];
    return sprite_to_vert(trf, uvs, VERTICES[in_vertex_index]);
}

@vertex
fn vs_vbuf_main(@builtin(vertex_index) in_vertex_index: u32, @location(0) trf:vec4<f32>, @location(1) sheet_region:vec4<f32>) -> VertexOutput {
    return sprite_to_vert(trf, sheet_region, VERTICES[in_vertex_index]);
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
