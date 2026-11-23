struct GlobalUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    _pad: vec2<f32>,
};

struct RippleUniforms {
    strength: f32,
    frequency: f32,
    speed: f32,
    _pad: f32, // Выравнивание до 16 байт
};

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOut {
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
    );
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );

    var out: VertexOut;
    out.pos = vec4<f32>(positions[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;
@group(1) @binding(0) var<uniform> g: GlobalUniforms;
@group(2) @binding(0) var<uniform> u: RippleUniforms;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let t = g.time * u.speed;

    // Плавное затухание у краёв экрана, чтобы не было артефактов на границах UV
    let edge_fade = sin(in.uv.x * 3.14159) * sin(in.uv.y * 3.14159);

    let wave_x = sin(in.uv.y * u.frequency + t) * cos(in.uv.x * u.frequency * 0.5 + t * 0.7);
    let wave_y = cos(in.uv.x * u.frequency + t * 0.8) * sin(in.uv.y * u.frequency * 0.5 + t);

    let offset = vec2<f32>(wave_x, wave_y) * (u.strength * 0.02) * edge_fade;
    let distorted_uv = clamp(in.uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));

    return textureSample(t_input, s_input, distorted_uv);
}