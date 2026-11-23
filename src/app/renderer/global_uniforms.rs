

use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniformsData {
    pub time: f32,
    pub delta_time: f32,
    pub resolution: [f32; 2],
    pub mouse: [f32; 2],
    pub _pad: [f32; 2], // Выравнивание до 32 байт
}

pub struct GlobalUniforms {
    buffer: wgpu::Buffer,
    pub layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl GlobalUniforms {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let initial = GlobalUniformsData {
            time: 0.0,
            delta_time: 0.016,
            resolution: [width as f32, height as f32],
            mouse: [0.5, 0.5],
            _pad: [0.0, 0.0],
        };

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Uniform Buffer"),
            contents: bytemuck::bytes_of(&initial),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Global Uniform Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Uniform Bind Group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            layout,
            bind_group,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, data: &GlobalUniformsData) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(data));
    }
}