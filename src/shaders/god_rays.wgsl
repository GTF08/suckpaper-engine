struct GlobalUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    _pad: vec2<f32>,
};

struct GodRaysUniforms {
    source_pos: vec2<f32>,   // Координаты солнца (можно выводить за экран, например -0.2 .. 1.2)
    intensity: f32,          // Общая яркость
    speed: f32,              // Скорость переливания лучей
    density: f32,            // Частота (количество) лучей
    falloff: f32,            // Насколько быстро лучи затухают с расстоянием
    color: vec3<f32>,        // RGB цвет лучей
    _pad: f32,               // Выравнивание до 48 байт (кратно 16)
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

@group(0) @binding(0) var<uniform> g: GlobalUniforms;
@group(1) @binding(0) var<uniform> u: GodRaysUniforms;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let aspect = g.resolution.x / max(g.resolution.y, 1.0);

    // Вектор от источника света к текущему пикселю (с учетом пропорций экрана)
    var dir = in.uv - u.source_pos;
    dir.x *= aspect;

    let dist = length(dir);
    let angle = atan2(dir.y, dir.x);
    let t = g.time * u.speed;

    // Несколько несинхронных волн по углу создают живые переливы света сквозь листву
    let w1 = sin(angle * u.density + t * 0.7);
    let w2 = sin(angle * (u.density * 1.73) - t * 0.45 + 1.3);
    let w3 = cos(angle * (u.density * 2.91) + t * 0.25 + 2.7);

    // Переводим сумму волн в диапазон [0.0 .. 1.0] и делаем лучи более контрастными
    let raw_rays = (w1 * 0.5 + w2 * 0.35 + w3 * 0.15) * 0.5 + 0.5;
    let rays = pow(clamp(raw_rays, 0.0, 1.0), 2.2);

    // Затухание света по мере удаления от источника
    let attenuation = exp(-dist * u.falloff);

    // Мягкое гашение у самых краев экрана, чтобы не было резких границ
    let edge = smoothstep(0.0, 0.08, in.uv.x)
             * smoothstep(1.0, 0.92, in.uv.x)
             * smoothstep(0.0, 0.08, in.uv.y)
             * smoothstep(1.0, 0.92, in.uv.y);

    let alpha = clamp(rays * attenuation * u.intensity * edge, 0.0, 1.0);

    return vec4<f32>(u.color, alpha);
}