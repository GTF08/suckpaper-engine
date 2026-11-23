struct GlobalUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    _pad: vec2<f32>,
};

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@group(0) @binding(0) var t_sprite: texture_2d<f32>;
@group(0) @binding(1) var s_sprite: sampler;
@group(1) @binding(0) var<uniform> g: GlobalUniforms;

@vertex
fn vs_main(
    @builtin(vertex_index) v_idx: u32,
    @location(0) inst_pos: vec2<f32>,
    @location(1) inst_size: f32,
    @location(2) inst_rotation: f32,
    @location(3) inst_alpha: f32,
) -> VertexOut {
    var quad = array<vec2<f32>, 4>(
        vec2<f32>(-0.5,  0.5),
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5,  0.5),
        vec2<f32>( 0.5, -0.5),
    );
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );

    let local = quad[v_idx];

    // Поворот вершины квада
    let c = cos(inst_rotation);
    let s = sin(inst_rotation);
    let rotated = vec2<f32>(
        local.x * c - local.y * s,
        local.x * s + local.y * c
    );

    // Перевод размера из пикселей в координаты NDC (-1..1)
    let size_ndc = (rotated * inst_size * 2.0) / max(g.resolution, vec2<f32>(1.0));

    // Перевод позиции (0..1, Y вниз) в NDC (-1..1, Y вверх)
    let center_ndc = vec2<f32>(
        inst_pos.x * 2.0 - 1.0,
        1.0 - inst_pos.y * 2.0
    );

    var out: VertexOut;
    out.pos = vec4<f32>(center_ndc + size_ndc, 0.0, 1.0);
    out.uv = uvs[v_idx];
    out.alpha = inst_alpha;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let tex = textureSample(t_sprite, s_sprite, in.uv);
    return vec4<f32>(tex.rgb, tex.a * in.alpha);
}