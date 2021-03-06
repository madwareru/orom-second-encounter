use orom_miniquad::*;

pub const TILEMAP_VERTEX: &str = //language=glsl
    r#"#version 100
    attribute vec2 pos;
    attribute vec2 uv;

    varying lowp vec2 texcoord;

    void main() {
        gl_Position = vec4(pos, 0, 1);
        texcoord = uv;
    }"#;

pub const TILEMAP_FRAGMENT: &str = //language=glsl
    r#"#version 100
    varying lowp vec2 texcoord;

    uniform sampler2D tex;

    uniform lowp vec2 mouse_pos;
    uniform lowp vec2 tile_resolution;

    uniform lowp vec3 grid_color;
    uniform lowp vec3 tool_color;

    void main() {
        lowp vec2 uv = texcoord * tile_resolution - vec2(0.5);
        lowp vec2 grid_lines = smoothstep(
            vec2(0.05),
            vec2(-0.05),
            fract(uv + vec2(0.5)) - vec2(0.05)
        );
        lowp float dist = max(abs(uv.x - mouse_pos.x), abs(uv.y - mouse_pos.y));
        lowp vec3 color =
            texture2D(tex, texcoord).zyx +
            grid_color * max(grid_lines.x, grid_lines.y) * 0.3 +
            tool_color * step(dist, 0.5);

        gl_FragColor = vec4(clamp(color, vec3(0.0), vec3(1.0)), 1.0);
    }"#;

pub const TEXT_RENDER_VERTEX: &str = //language=glsl
    r#"#version 100
    attribute vec2 pos;
    attribute vec2 uv;

    varying lowp vec2 texcoord;

    uniform lowp vec2 offset;
    uniform lowp vec2 scale;

    void main() {
        gl_Position = vec4(pos * scale + offset, 0.0, 1.0);
        texcoord = uv;
    }"#;

pub const TEXT_RENDER_FRAGMENT: &str = //language=glsl
    r#"#version 100
    varying lowp vec2 texcoord;

    uniform sampler2D tex;

    uniform lowp vec3 font_color;

    void main() {
        lowp vec4 clr = texture2D(tex, texcoord).xxxx;
        gl_FragColor = vec4(clr.xxx * font_color, clr.x);
    }"#;

pub fn tilemap_meta() -> ShaderMeta {
    ShaderMeta {
        images: vec!["tex".to_string()],
        uniforms: UniformBlockLayout {
            uniforms: vec![
                UniformDesc::new("mouse_pos", UniformType::Float2),
                UniformDesc::new("tile_resolution", UniformType::Float2),
                UniformDesc::new("grid_color", UniformType::Float3),
                UniformDesc::new("tool_color", UniformType::Float3),
            ],
        },
    }
}

#[repr(C)]
pub struct TilemapUniforms {
    pub mouse_pos: (f32, f32),
    pub tile_resolution: (f32, f32),
    pub grid_color: (f32, f32, f32),
    pub tool_color: (f32, f32, f32),
}

pub fn info_text_meta() -> ShaderMeta {
    ShaderMeta {
        images: vec!["tex".to_string()],
        uniforms: UniformBlockLayout {
            uniforms: vec![
                UniformDesc::new("offset", UniformType::Float2),
                UniformDesc::new("scale", UniformType::Float2),
                UniformDesc::new("font_color", UniformType::Float3),
            ],
        },
    }
}

#[repr(C)]
pub struct InfoTextUniforms {
    pub pos: (f32, f32),
    pub scale: (f32, f32),
    pub font_color: (f32, f32, f32)
}