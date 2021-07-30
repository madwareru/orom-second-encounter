pub const GRAPHICS_RES: &[u8] = include_bytes!("../../assets/GRAPHICS.RES");
pub const GUI_TEXTURE_BYTES: &[&[u8]] = &[
    include_bytes!("../../assets/land.bmp"),
    include_bytes!("../../assets/grass.bmp"),
    include_bytes!("../../assets/plateau.bmp"),
    include_bytes!("../../assets/sand.bmp"),
    include_bytes!("../../assets/savannah.bmp"),
    include_bytes!("../../assets/rocks.bmp"),
    include_bytes!("../../assets/highrock.bmp"),
    include_bytes!("../../assets/water.bmp"),
    include_bytes!("../../assets/road.bmp")
];
pub const INFO_TEXT_BYTES: &[u8] = include_bytes!("../../assets/info_text.bmp");
pub const JETBRAINS_MONO_FONT: &[u8] = include_bytes!("../../assets/JetBrainsMono-Medium.ttf");

pub const LAND: u8 = 0;
pub const GRASS: u8 = 1;
pub const PLATEAU: u8 = 2;
pub const SAND: u8 = 3;
pub const SAVANNAH: u8 = 4;
pub const ROCKS: u8 = 5;
pub const HIGH_ROCKS: u8 = 6;
pub const WATER: u8 = 7;
pub const ROAD: u8 = 8;

pub const fn get_tool_color(tool: u8) -> (f32, f32, f32) {
    match tool {
        LAND => (0.5, 0.4, 0.22),
        GRASS => (0.0, 0.7, 0.3),
        PLATEAU => (0.44, 0.4, 0.52),
        SAND => (0.7, 0.7, 0.0),
        SAVANNAH => (0.0, 0.7, 0.7),
        ROCKS => (0.7, 0.6, 0.0),
        HIGH_ROCKS => (1.0, 1.0, 1.0),
        WATER => (0.4, 0.45, 0.8),
        ROAD => (0.65, 0.6, 0.6),
        _ => (0.0, 0.4, 0.7)
    }
}

pub const SCREEN_WIDTH: i32 = 1280;
pub const SCREEN_HEIGHT: i32 = 800;

pub const WIDTH: usize = 40;
pub const HEIGHT: usize = 25;

#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
pub struct Vertex {
    pub pos: Vec2,
    pub uv: Vec2,
}

#[rustfmt::skip]
pub const VERTICES: [Vertex; 4] = [
    Vertex { pos : Vec2 { x: -1., y: -1. }, uv: Vec2 { x: 0., y: 1. } },
    Vertex { pos : Vec2 { x:  1., y: -1. }, uv: Vec2 { x: 1., y: 1. } },
    Vertex { pos : Vec2 { x:  1., y:  1. }, uv: Vec2 { x: 1., y: 0. } },
    Vertex { pos : Vec2 { x: -1., y:  1. }, uv: Vec2 { x: 0., y: 0. } },
];