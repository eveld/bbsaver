// VGA color palette (16 standard colors) as uniform
struct Uniforms {
    screen_size: vec2<f32>,    // window width, height in pixels
    cell_size: vec2<f32>,      // cell width, height in pixels
    scroll_offset: f32,        // fractional row offset for smooth scrolling
    margin_left: f32,          // horizontal offset in pixels to center art
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var font_texture: texture_2d<f32>;
@group(0) @binding(2) var font_sampler: sampler;

// VGA 16-color palette
const PALETTE: array<vec3<f32>, 16> = array<vec3<f32>, 16>(
    vec3<f32>(0.0,    0.0,    0.0),    // 0: black
    vec3<f32>(0.667,  0.0,    0.0),    // 1: red (AA0000)
    vec3<f32>(0.0,    0.667,  0.0),    // 2: green
    vec3<f32>(0.667,  0.333,  0.0),    // 3: brown/yellow (AA5500)
    vec3<f32>(0.0,    0.0,    0.667),  // 4: blue
    vec3<f32>(0.667,  0.0,    0.667),  // 5: magenta
    vec3<f32>(0.0,    0.667,  0.667),  // 6: cyan
    vec3<f32>(0.667,  0.667,  0.667),  // 7: light gray
    vec3<f32>(0.333,  0.333,  0.333),  // 8: dark gray
    vec3<f32>(1.0,    0.333,  0.333),  // 9: bright red
    vec3<f32>(0.333,  1.0,    0.333),  // 10: bright green
    vec3<f32>(1.0,    1.0,    0.333),  // 11: bright yellow
    vec3<f32>(0.333,  0.333,  1.0),    // 12: bright blue
    vec3<f32>(1.0,    0.333,  1.0),    // 13: bright magenta
    vec3<f32>(0.333,  1.0,    1.0),    // 14: bright cyan
    vec3<f32>(1.0,    1.0,    1.0),    // 15: white
);

struct InstanceInput {
    @location(0) grid_pos: vec2<u32>,  // col, row (in grid coordinates)
    @location(1) glyph: u32,
    @location(2) colors: u32,          // fg in low byte, bg in high byte
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec3<f32>,
    @location(2) bg_color: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    // Unit quad: 2 triangles from 6 vertices
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );

    let pos = positions[vertex_index];

    // Cell position in pixels (offset by margin to center art)
    let cell_x = f32(instance.grid_pos.x) * uniforms.cell_size.x + uniforms.margin_left;
    let cell_y = (f32(instance.grid_pos.y) - uniforms.scroll_offset) * uniforms.cell_size.y;

    // Pixel position of this vertex within the cell
    let px = cell_x + pos.x * uniforms.cell_size.x;
    let py = cell_y + pos.y * uniforms.cell_size.y;

    // Convert to NDC (-1..1, Y flipped)
    let ndc_x = (px / uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / uniforms.screen_size.y) * 2.0;

    // UV into font atlas (16x16 grid of glyphs)
    let atlas_col = f32(instance.glyph % 16u);
    let atlas_row = f32(instance.glyph / 16u);
    let uv_x = (atlas_col + pos.x) / 16.0;
    let uv_y = (atlas_row + pos.y) / 16.0;

    let fg_idx = instance.colors & 0xFFu;
    let bg_idx = (instance.colors >> 8u) & 0xFFu;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = vec2<f32>(uv_x, uv_y);
    out.fg_color = PALETTE[fg_idx];
    out.bg_color = PALETTE[bg_idx];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let glyph_alpha = textureSample(font_texture, font_sampler, in.uv).r;
    let color = mix(in.bg_color, in.fg_color, glyph_alpha);
    return vec4<f32>(color, 1.0);
}
