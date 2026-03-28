use std::collections::HashMap;

const VGA_FONT_DATA: &[u8; 4096] = include_bytes!("../assets/IBM_VGA_8x16.bin");

pub const GLYPH_WIDTH: u32 = 8;
pub const GLYPH_HEIGHT: u32 = 16;
pub const ATLAS_COLS: u32 = 16;
pub const ATLAS_ROWS: u32 = 16;
pub const ATLAS_WIDTH: u32 = GLYPH_WIDTH * ATLAS_COLS; // 128
pub const ATLAS_HEIGHT: u32 = GLYPH_HEIGHT * ATLAS_ROWS; // 256

pub struct FontAtlas {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

pub struct FontAtlasRegistry {
    atlases: HashMap<String, FontAtlas>,
    default_key: String,
}

impl FontAtlasRegistry {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut atlases = HashMap::new();

        let default = create_atlas(device, queue, VGA_FONT_DATA);
        let default_key = "IBM VGA".to_string();
        atlases.insert(default_key.clone(), default);

        FontAtlasRegistry {
            atlases,
            default_key,
        }
    }

    /// Look up a font atlas by SAUCE font name. Falls back to IBM VGA.
    pub fn get(&self, font_name: &str) -> &FontAtlas {
        let key = if font_name.is_empty() {
            &self.default_key
        } else {
            font_name
        };

        self.atlases
            .get(key)
            .unwrap_or_else(|| self.atlases.get(&self.default_key).unwrap())
    }

    /// Get the default atlas.
    pub fn default(&self) -> &FontAtlas {
        self.atlases.get(&self.default_key).unwrap()
    }
}

fn create_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    font_data: &[u8; 4096],
) -> FontAtlas {
    let pixels = font_to_pixels(font_data);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("font_atlas"),
        size: wgpu::Extent3d {
            width: ATLAS_WIDTH,
            height: ATLAS_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(ATLAS_WIDTH),
            rows_per_image: Some(ATLAS_HEIGHT),
        },
        wgpu::Extent3d {
            width: ATLAS_WIDTH,
            height: ATLAS_HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("font_sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    FontAtlas { view, sampler }
}

/// Convert 1-bit-per-pixel VGA font data into an R8 atlas image.
fn font_to_pixels(font: &[u8; 4096]) -> Vec<u8> {
    let mut pixels = vec![0u8; (ATLAS_WIDTH * ATLAS_HEIGHT) as usize];

    for glyph in 0u32..256 {
        let atlas_col = glyph % ATLAS_COLS;
        let atlas_row = glyph / ATLAS_COLS;
        let base_x = atlas_col * GLYPH_WIDTH;
        let base_y = atlas_row * GLYPH_HEIGHT;

        for row in 0..GLYPH_HEIGHT {
            let font_byte = font[(glyph * GLYPH_HEIGHT + row) as usize];
            for bit in 0..GLYPH_WIDTH {
                let set = (font_byte >> (7 - bit)) & 1;
                let px = base_x + bit;
                let py = base_y + row;
                pixels[(py * ATLAS_WIDTH + px) as usize] = if set != 0 { 0xFF } else { 0x00 };
            }
        }
    }

    pixels
}
