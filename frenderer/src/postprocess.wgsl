var<private> VERTICES:array<vec4<f32>,6> = array<vec4<f32>,6>(
    vec4<f32>(-1., -1., 0., 1.),
    vec4<f32>(1., -1., 0., 1.),
    vec4<f32>(-1., 1., 0., 1.),
    vec4<f32>(-1., 1., 0., 1.),
    vec4<f32>(1., -1., 0., 1.),
    vec4<f32>(1., 1., 0., 1.)
);
var<private> TEX_COORDS:array<vec2<f32>,6> = array<vec2<f32>,6>(
    vec2<f32>(0., 1.),
    vec2<f32>(1., 1.),
    vec2<f32>(0., 0.),
    vec2<f32>(0., 0.),
    vec2<f32>(1., 1.),
    vec2<f32>(1., 0.)
);

struct Transform {
   a: vec4<f32>,
   b: vec4<f32>,
   c: vec4<f32>,
   d: vec4<f32>,
}

struct ColorTransform {
   a: vec4<f32>,
   b: vec4<f32>,
   c: vec4<f32>,
   d: vec4<f32>,
   saturation_padding:vec4<f32>,
}

@group(0) @binding(0)
var<uniform> u_transform: Transform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_vbuf_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
  let trf_mat = mat4x4<f32>(u_transform.a, u_transform.b, u_transform.c, u_transform.d);
  var out:VertexOutput;
  out.clip_position = trf_mat * VERTICES[in_vertex_index];
  out.tex_coords = TEX_COORDS[in_vertex_index];
  return out;
}


// Color mod info
@group(1) @binding(0)
var<uniform> u_color: ColorTransform;
// A color texture...
@group(1) @binding(1)
var t_diffuse: texture_2d<f32>;
// And a sampler.
@group(1) @binding(2)
var s_diffuse: sampler;
/*
  //Later: 
// A color LUT texture...
@group(1) @binding(3)
var t_lut: texture_3d<f32>;
// And a sampler.
@group(1) @binding(4)
var s_lut: sampler;
*/
@fragment
fn fs_main(in:VertexOutput) -> @location(0) vec4<f32> {
    var color:vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    // apply colormod matrix
    color = mat4x4<f32>(u_color.a, u_color.b, u_color.c, u_color.d) * color;
    // apply saturation/desaturation
    let intensity:f32 = (color.x + color.y + color.z) / 3.0;
    let dev:vec4<f32> = vec4<f32>(intensity-color.x, intensity-color.y, intensity-color.z, 1.0);
    color += dev * -u_color.saturation_padding.x;
    color.a = 1.0;
    return color;
}
