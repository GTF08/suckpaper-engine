

pub struct RenderTargets {
    pub current_view: wgpu::TextureView,
    pub next_view: wgpu::TextureView,
    current_texture: wgpu::Texture,
    next_texture: wgpu::Texture,
    format: wgpu::TextureFormat,
}

impl RenderTargets {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let (current_texture, current_view) = Self::create_target(device, width, height, format, "Target A");
        let (next_texture, next_view) = Self::create_target(device, width, height, format, "Target B");

        Self {
            current_view,
            next_view,
            current_texture,
            next_texture,
            format,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        *self = Self::new(device, width, height, self.format);
    }

    pub fn swap(&mut self) {
        std::mem::swap(&mut self.current_texture, &mut self.next_texture);
        std::mem::swap(&mut self.current_view, &mut self.next_view);
    }

    fn create_target(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            // RENDER_ATTACHMENT — чтобы писать в текстуру,
            // TEXTURE_BINDING — чтобы читать из неё в следующем шейдере
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&Default::default());
        (texture, view)
    }
}