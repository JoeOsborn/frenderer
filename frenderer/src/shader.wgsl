// // A square!
// var<private> VERTICES:array<vec2<f32>,6> = array<vec2<f32>,6>(
//     // Bottom left, bottom right, top left; then top left, bottom right, top right.
//     vec2<f32>(-0.5, -0.5),
//     vec2<f32>(0.5, -0.5),
//     vec2<f32>(-0.5, 0.5),
//     vec2<f32>(-0.5, 0.5),
//     vec2<f32>(0.5, -0.5),
//     vec2<f32>(0.5, 0.5)
// );

struct Camera {
    screen_pos: vec2<f32>,
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;
@group(0) @binding(1)
var<uniform> u_vtx: array<vec4<f32>>;
@group(0) @binding(2)
var<uniform> u_uv: array<vec4<f32>>;
@group(0) @binding(3)
var<storage, read> s_world: array<vec4<f32>>;
@group(0) @binding(4)
var<storage, read> s_which: array<u32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

fn sprite_to_vert(which_vert:u32, trf:vec4<f32>, which_frame:u16) -> VertexOutput {
  let norm_corners:vec4<f32> = u_vtx[which_frame];
  let uv_corners:vec4<f32> = u_uv[which_frame];
  var norm_vert:vec2<f32> = norm_corners.xy;
  var uvs:vec2<f32> = uv_corners.xy;
  // bl, br, tl; tr, tl, br
  //  0   1   2   3   4   5
  // L: 0, 2, 4
  // R: 1, 3, 5
  // T: 2, 3, 4
  // B: 0, 1, 5
  if which_vert & 0x0000_0001u {
      norm_vert.x = norm_corners.z;
      uvs.x = uv_corners.z;
  }
  if 2 <= which_vert && which_vert <= 4 {
      norm_vert.y = norm_corners.w;
      uvs.y = uv_corners.w;
  }
  let center:vec2<f32> = trf.yz;
  let size_bits:u32 = bitcast<u32>(trf.x);
  let size:vec2<f32> = vec2(f32((size_bits & 0xFFFF0000u) >> 16u),
                            f32(size_bits & 0x0000FFFFu)
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
  let world_pos = (center+size*0.5) + rotated;
  let camera_pos = world_pos - camera.screen_pos;
  let box_pos = camera_pos / (camera.screen_size*0.5);
  let ndc_pos = vec4(box_pos.xy, 0.0, 1.0) - vec4(1.0, 1.0, 0.0, 0.0);
  return VertexOutput(ndc_pos, uvs);
}

@vertex
fn vs_storage_main(@builtin(vertex_index) in_vertex_index: u32, @builtin(instance_index) sprite_index:u32) -> VertexOutput {
  // We'll just look up the vertex data in those constant arrays
  let trf = s_world[sprite_index];
  let which_frame__rsrvd = s_which[sprite_index];
  return sprite_to_vert(in_vertex_index, trf, (which_frame__rsrvd | 0xFFFF0000u) >> 16);
}

@vertex
fn vs_storage_noinstance_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let sprite_index:u32 = in_vertex_index / u32(6);
    let vertex_index:u32 = in_vertex_index - (sprite_index * u32(6));
    let trf = s_world[sprite_index];
    let which_frame = s_which[sprite_index];
    return sprite_to_vert(vertex_index, trf, (which_frame__rsrvd | 0xFFFF0000u) >> 16);
}

@vertex
fn vs_vbuf_main(@builtin(vertex_index) in_vertex_index: u32, @location(0) trf:vec4<f32>, @location(1) which_frame__rsrvd:u32) -> VertexOutput {
  return sprite_to_vert(in_vertex_index, trf, sheet_region, u16((which_frame__rsrvd | 0xFFFF0000u) >> 16));
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
