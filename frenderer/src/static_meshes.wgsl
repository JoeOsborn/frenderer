@group(0) @binding(0)
var<uniform> projview: mat4x4<f32>;

struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) uv_which: vec3<f32>
}

struct InstanceInput {
  @location(2) translate_scale: vec4<f32>,
  @location(3) rot: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) @interpolate(flat) tex_index: u32,
}

@vertex
fn vs_main(vtx:VertexInput, inst:InstanceInput) -> VertexOutput {
  var out:VertexOutput;
  let model = mat_from_trs(inst.translate_scale.xyz, inst.rot, inst.translate_scale.w);
  let transformed = model * vec4(vtx.position,1.0);
  out.clip_position = projview * transformed;
  out.tex_coords = vtx.uv_which.xy;
  out.tex_index = bitcast<u32>(vtx.uv_which.z);
  return out;
}

fn mat_from_trs(t:vec3<f32>, r:vec4<f32>, s:f32) -> mat4x4<f32> {
  let trans =
    mat4x4<f32>
    (
     vec4<f32>(1.0,0.0,0.0,0.0),
     vec4<f32>(0.0,1.0,0.0,0.0),
     vec4<f32>(0.0,0.0,1.0,0.0),
     vec4<f32>(   t.xyz,   1.0),
     );
  let rot =
    mat4x4<f32>
    (
     vec4<f32>(2.0*(r.x*r.x+r.y*r.y) - 1.0,
               2.0*(r.y*r.z-r.x*r.w),
               2.0*(r.y*r.w+r.x*r.z),
               0.0),
     vec4<f32>(2.0*(r.y*r.z+r.x*r.w),
               2.0*(r.x*r.x+r.z*r.z) - 1.0,
               2.0*(r.z*r.w-r.x*r.y),
               0.0),
     vec4<f32>(2.0*(r.y*r.w-r.x*r.z),
               2.0*(r.z*r.w+r.x*r.y),
               2.0*(r.x*r.x+r.w*r.w) - 1.0,
               0.0),
     vec4<f32>(0.0,0.0,0.0,1.0),
     );
  let scale =
    mat4x4<f32>
    (
     vec4<f32>(  s,0.0,0.0,0.0),
     vec4<f32>(0.0,  s,0.0,0.0),
     vec4<f32>(0.0,0.0,  s,0.0),
     vec4<f32>(0.0,0.0,0.0,1.0),
     );
  return trans*rot*scale;
}

// Now our fragment shader needs two "global" inputs to be bound:
// A texture...
@group(1) @binding(0)
var t_diffuse: texture_2d_array<f32>;
// And a sampler.
@group(1) @binding(1)
var s_diffuse: sampler;
// Both are in the same binding group here since they go together naturally.

// Our fragment shader takes an interpolated `VertexOutput` as input now
@fragment
fn fs_main(in:VertexOutput) -> @location(0) vec4<f32> {
    // And we use the tex coords from the vertex output to sample from the texture.
    let color:vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords, in.tex_index);
    // if color.w < 0.2 { discard; }
    return color;
}
