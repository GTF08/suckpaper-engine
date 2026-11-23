struct GlobalUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    _pad: vec2<f32>,
};

struct SnowUniforms {
    snow_intensity: f32,
    snow_speed: f32,
    bokeh_intensity: f32,
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
@group(2) @binding(0) var<uniform> u: SnowUniforms;

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

fn snow_layer(uv: vec2<f32>, scale: f32, speed: f32, size: f32, seed: f32) -> f32 {
    var p = uv * scale;
    p.y -= g.time * speed;
    p.x += sin(g.time * 0.8 + p.y * 0.5 + seed) * 0.35;

    let cell = floor(p);
    let local = fract(p) - 0.5;

    let rnd = hash22(cell + seed) - 0.5;
    let dist = length(local - rnd * 0.6);

    return smoothstep(size, size * 0.1, dist);
}

fn bokeh_lights(uv: vec2<f32>) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let pos = vec2<f32>(
            0.5 + 0.4 * sin(g.time * (0.2 + fi * 0.05) + fi * 1.7),
            0.5 + 0.4 * cos(g.time * (0.15 + fi * 0.04) + fi * 2.3)
        );
        let d = length(uv - pos);
        let glow = smoothstep(0.25, 0.0, d) * 0.12;
        let tint = mix(vec3<f32>(1.0, 0.8, 0.5), vec3<f32>(0.5, 0.8, 1.0), fract(fi * 0.37));
        light += tint * glow;
    }
    return light;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let base_color = textureSample(t_input, s_input, in.uv);
    var color = base_color.rgb;

    let aspect = g.resolution.x / max(g.resolution.y, 1.0);
    let aspect_uv = vec2<f32>(in.uv.x * aspect, in.uv.y);

    var snow = 0.0;
    snow += snow_layer(aspect_uv, 18.0, 0.4 * u.snow_speed, 0.06, 1.0) * 0.3;
    snow += snow_layer(aspect_uv, 14.0, 0.7 * u.snow_speed, 0.08, 2.0) * 0.5;
    snow += snow_layer(aspect_uv, 10.0, 1.1 * u.snow_speed, 0.10, 3.0) * 0.7;
    snow += snow_layer(aspect_uv,  7.0, 1.6 * u.snow_speed, 0.12, 4.0) * 0.9;
    snow += snow_layer(aspect_uv,  5.0, 2.2 * u.snow_speed, 0.15, 5.0) * 1.0;

    color += vec3<f32>(snow * u.snow_intensity);
    color += bokeh_lights(aspect_uv) * u.bokeh_intensity;

    return vec4<f32>(color, base_color.a);
}