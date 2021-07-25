use miniquad::*;
use rom_media_rs::image_rendering::bmp_sprite_decorators::TrueColorSurfaceSprite;
use rom_media_rs::image_rendering::blittable::{Blittable, BlitBuilder};
use std::io::Cursor;
use rom_res_rs::ResourceFile;
use rom_loaders_rs::images::sprite::BmpSprite;
use simple_tiled_wfc::grid_generation::{WfcModule, WfcContext};

const GRAPHICS_RES: &[u8] = include_bytes!("assets/GRAPHICS.RES");
const DIRT: u8 = 0;
const GRASS: u8 = 1;
const ROAD: u8 = 2;
const SAND: u8 = 3;
const SAVANNAH: u8 = 4;
const HIGH_ROCKS: u8 = 5;
const MOUNTAIN: u8 = 6;
const WATER: u8 = 7;
const DIRT_2: u8 = 8;

#[repr(C)]
struct Vec2 {
    x: f32,
    y: f32,
}
#[repr(C)]
struct Vertex {
    pos: Vec2,
    uv: Vec2,
}

type CustomBitSet = [u8; 30];

struct TileInfo {
    north_west: u8,
    north_east: u8,
    south_west: u8,
    south_east: u8,
    tile_x: usize,
    tile_y: usize
}

struct Stage {
    pipeline: Pipeline,
    bindings: Bindings,
    atlas: TrueColorSurfaceSprite,
    stage_surface: TrueColorSurfaceSprite,
    tiles: Vec<TileInfo>,
    modules: Vec<WfcModule<CustomBitSet>>,
    should_update: bool
}

impl Stage {
    pub fn new(ctx: &mut Context) -> Stage {
        let mut resource_file = ResourceFile::new(Cursor::new(GRAPHICS_RES))
            .expect(&format!("failed to open VIDEO4.RES"));

        let mut graphics_resources = Vec::new();

        for j in 1..=2 {
            for i in 0..16 {
                let terrain_tile_name = format!("terrain/tile{}-{:#02}.bmp", j, i);
                let mut resource = Cursor::new(resource_file
                    .get_resource_bytes(&terrain_tile_name)
                    .expect(&format!("failed to load resource {}", &terrain_tile_name))
                );
                let sprite = BmpSprite::read_from(&mut resource).unwrap();
                graphics_resources.push(sprite);
            }
        }
        for j in 3..=4 {
            for i in 0..4 {
                let terrain_tile_name = format!("terrain/tile{}-{:#02}.bmp", j, i);
                let mut resource = Cursor::new(resource_file
                    .get_resource_bytes(&terrain_tile_name)
                    .expect(&format!("failed to load resource {}", &terrain_tile_name))
                );
                let sprite = BmpSprite::read_from(&mut resource).unwrap();
                graphics_resources.push(sprite);
            }
        }
        let mut atlas = TrueColorSurfaceSprite::new(1024, 1024);
        let mut stage_surface = TrueColorSurfaceSprite::new(1280 * 2, 800 * 2);

        let tile_definitions = &[
            (DIRT, GRASS, 0, 0),
            (DIRT, ROAD, 4, 0),
            (DIRT, SAND, 8, 0),
            (DIRT, SAVANNAH, 12, 0),
            (DIRT, HIGH_ROCKS, 0, 6),
            (HIGH_ROCKS, ROAD, 4, 6),
            (HIGH_ROCKS, MOUNTAIN, 8, 6),
            (DIRT, WATER, 12, 6),
            (DIRT, DIRT_2, 0, 12)
        ];

        let mut tiles = Vec::new();
        for &(outer_type, inner_type, start_tile_x, start_tile_y) in tile_definitions {
            tiles.push(TileInfo {
                north_west: outer_type, north_east: outer_type,
                south_west: outer_type, south_east: inner_type,
                tile_x: (start_tile_x + 0) * 32, tile_y: (start_tile_y + 0) * 32
            });
            tiles.push(TileInfo {
                north_west: outer_type, north_east: outer_type,
                south_west: inner_type, south_east: inner_type,
                tile_x: (start_tile_x + 1) * 32, tile_y: (start_tile_y + 0) * 32
            });
            tiles.push(TileInfo {
                north_west: outer_type, north_east: outer_type,
                south_west: inner_type, south_east: outer_type,
                tile_x: (start_tile_x + 2) * 32, tile_y: (start_tile_y + 0) * 32
            });

            tiles.push(TileInfo {
                north_west: outer_type, north_east: inner_type,
                south_west: outer_type, south_east: inner_type,
                tile_x: (start_tile_x + 0) * 32, tile_y: (start_tile_y + 1) * 32
            });
            tiles.push(TileInfo {
                north_west: inner_type, north_east: inner_type,
                south_west: inner_type, south_east: inner_type,
                tile_x: (start_tile_x + 1) * 32, tile_y: (start_tile_y + 1) * 32
            });
            tiles.push(TileInfo {
                north_west: inner_type, north_east: outer_type,
                south_west: inner_type, south_east: outer_type,
                tile_x: (start_tile_x + 2) * 32, tile_y: (start_tile_y + 1) * 32
            });

            tiles.push(TileInfo {
                north_west: outer_type, north_east: inner_type,
                south_west: outer_type, south_east: outer_type,
                tile_x: (start_tile_x + 0) * 32, tile_y: (start_tile_y + 2) * 32
            });
            tiles.push(TileInfo {
                north_west: inner_type, north_east: inner_type,
                south_west: outer_type, south_east: outer_type,
                tile_x: (start_tile_x + 1) * 32, tile_y: (start_tile_y + 2) * 32
            });
            tiles.push(TileInfo {
                north_west: inner_type, north_east: outer_type,
                south_west: outer_type, south_east: outer_type,
                tile_x: (start_tile_x + 2) * 32, tile_y: (start_tile_y + 2) * 32
            });

            tiles.push(TileInfo {
                north_west: inner_type, north_east: inner_type,
                south_west: inner_type, south_east: outer_type,
                tile_x: (start_tile_x + 0) * 32, tile_y: (start_tile_y + 3) * 32
            });
            tiles.push(TileInfo {
                north_west: inner_type, north_east: inner_type,
                south_west: outer_type, south_east: outer_type,
                tile_x: (start_tile_x + 1) * 32, tile_y: (start_tile_y + 3) * 32
            });
            tiles.push(TileInfo {
                north_west: inner_type, north_east: inner_type,
                south_west: outer_type, south_east: inner_type,
                tile_x: (start_tile_x + 2) * 32, tile_y: (start_tile_y + 3) * 32
            });

            tiles.push(TileInfo {
                north_west: inner_type, north_east: outer_type,
                south_west: inner_type, south_east: outer_type,
                tile_x: (start_tile_x + 0) * 32, tile_y: (start_tile_y + 4) * 32
            });
            tiles.push(TileInfo {
                north_west: outer_type, north_east: outer_type,
                south_west: outer_type, south_east: outer_type,
                tile_x: (start_tile_x + 1) * 32, tile_y: (start_tile_y + 4) * 32
            });
            tiles.push(TileInfo {
                north_west: outer_type, north_east: inner_type,
                south_west: outer_type, south_east: inner_type,
                tile_x: (start_tile_x + 2) * 32, tile_y: (start_tile_y + 4) * 32
            });

            tiles.push(TileInfo {
                north_west: inner_type, north_east: outer_type,
                south_west: inner_type, south_east: inner_type,
                tile_x: (start_tile_x + 0) * 32, tile_y: (start_tile_y + 5) * 32
            });
            tiles.push(TileInfo {
                north_west: outer_type, north_east: outer_type,
                south_west: inner_type, south_east: inner_type,
                tile_x: (start_tile_x + 1) * 32, tile_y: (start_tile_y + 5) * 32
            });
            tiles.push(TileInfo {
                north_west: outer_type, north_east: inner_type,
                south_west: inner_type, south_east: inner_type,
                tile_x: (start_tile_x + 2) * 32, tile_y: (start_tile_y + 5) * 32
            });

            for j in 0..6 {
                tiles.push(TileInfo {
                    north_west: inner_type, north_east: inner_type,
                    south_west: inner_type, south_east: inner_type,
                    tile_x: (start_tile_x + 3) * 32, tile_y: (start_tile_y + j) * 32
                });
            }
        }

        for i in 0..graphics_resources.len() {
            let x = 32 * (i % 16);
            let y = 6 * 32 * (i / 16);

            BlitBuilder::try_create(&mut atlas, &graphics_resources[i])
                .expect("failed to create blit builder")
                .with_source_subrect(0, 0, 32, 6 * 32)
                .with_dest_pos(x as i32, y as i32)
                .blit();
        }

        let mut modules = Vec::new();
        for i in 0..tiles.len() {
            let mut module: WfcModule<CustomBitSet> = WfcModule::new();
            for j in 0..tiles.len() {
                if tiles[i].north_east == tiles[j].south_east &&
                    tiles[i].north_west == tiles[j].south_west {
                    module.add_north_neighbour(j);
                }
                if tiles[i].south_east == tiles[j].north_east &&
                    tiles[i].south_west == tiles[j].north_west {
                    module.add_south_neighbour(j);
                }
                if tiles[i].north_east == tiles[j].north_west &&
                    tiles[i].south_east == tiles[j].south_west {
                    module.add_east_neighbour(j);
                }
                if tiles[i].north_west == tiles[j].north_east &&
                    tiles[i].south_west == tiles[j].south_east {
                    module.add_west_neighbour(j);
                }
            }
            modules.push(module);
        }

        let mut wfc_context: WfcContext<CustomBitSet> = WfcContext::new(&modules, 80, 50);
        match wfc_context.collapse(100) {
            Ok(result_tile_indices) => {
                for idx in 0..result_tile_indices.len() {
                    let row = idx / 80;
                    let column = idx % 80;
                    let tile_id = result_tile_indices[idx];
                    let tile_info = &tiles[tile_id];
                    BlitBuilder::try_create(&mut stage_surface, &atlas)
                        .expect("failed to create blit builder")
                        .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                        .with_dest_pos(column as i32 * 32, row as i32 * 32)
                        .blit();
                }
            }
            Err(_) => {
                for row in 0..50 {
                    for column in 0..80 {
                        let tile_info = &tiles[4];
                        BlitBuilder::try_create(&mut stage_surface, &atlas)
                            .expect("failed to create blit builder")
                            .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                            .with_dest_pos(column as i32 * 32, row as i32 * 32)
                            .blit();
                    }
                }
            }
        }

        let casted = bytemuck::cast_slice(stage_surface.color_data());
        let texture = Texture::from_data_and_format(
            ctx,
            &casted,
            TextureParams {
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Clamp,
                filter: FilterMode::Linear,
                width: stage_surface.get_width() as u32,
                height: stage_surface.get_height() as u32
            }
        );

        #[rustfmt::skip]
            let vertices: [Vertex; 4] = [
            Vertex { pos : Vec2 { x: -1., y: -1. }, uv: Vec2 { x: 0., y: 1. } },
            Vertex { pos : Vec2 { x:  1., y: -1. }, uv: Vec2 { x: 1., y: 1. } },
            Vertex { pos : Vec2 { x:  1., y:  1. }, uv: Vec2 { x: 1., y: 0. } },
            Vertex { pos : Vec2 { x: -1., y:  1. }, uv: Vec2 { x: 0., y: 0. } },
        ];
        let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);

        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &indices);

        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer,
            images: vec![texture],
        };

        let shader = Shader::new(ctx, shader::VERTEX, shader::FRAGMENT, shader::meta()).unwrap();

        let pipeline = Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("pos", VertexFormat::Float2),
                VertexAttribute::new("uv", VertexFormat::Float2),
            ],
            shader,
        );

        Stage {
            pipeline,
            bindings,
            atlas,
            stage_surface,
            tiles,
            modules,
            should_update: false
        }
    }
}

impl EventHandler for Stage {
    fn update(&mut self, ctx: &mut Context) {
        if self.should_update {
            let mut wfc_context: WfcContext<CustomBitSet> = WfcContext::new(&self.modules, 80, 50);
            match wfc_context.collapse(10) {
                Ok(result_tile_indices) => {
                    for idx in 0..result_tile_indices.len() {
                        let row = idx / 80;
                        let column = idx % 80;
                        let tile_id = result_tile_indices[idx];
                        let tile_info = &self.tiles[tile_id];
                        BlitBuilder::try_create(&mut self.stage_surface, &self.atlas)
                            .expect("failed to create blit builder")
                            .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                            .with_dest_pos(column as i32 * 32, row as i32 * 32)
                            .blit();
                    }
                }
                Err(_) => {
                    for row in 0..50 {
                        for column in 0..80 {
                            let tile_info = &self.tiles[4];
                            BlitBuilder::try_create(&mut self.stage_surface, &self.atlas)
                                .expect("failed to create blit builder")
                                .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                                .with_dest_pos(column as i32 * 32, row as i32 * 32)
                                .blit();
                        }
                    }
                }
            }
            let casted = bytemuck::cast_slice(self.stage_surface.color_data());
            self.bindings.images[0].update(ctx, casted);
            self.should_update = false;
        }
    }

    fn draw(&mut self, ctx: &mut Context) {
        ctx.begin_default_pass(Default::default());

        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);

        ctx.apply_uniforms(&shader::Uniforms {
            offset: (0.0, 0.0),
        });

        ctx.draw(0, 6, 1);

        ctx.end_render_pass();

        ctx.commit_frame();
    }

    fn key_up_event(&mut self, ctx: &mut Context, keycode: KeyCode, _keymods: KeyMods) {
        match keycode {
            KeyCode::Escape => {
                ctx.quit();
            },
            KeyCode::Space => {
                self.should_update = true;
            },
            _ => {}
        }
    }
}

mod shader {
    use miniquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec2 pos;
    attribute vec2 uv;

    uniform vec2 offset;

    varying lowp vec2 texcoord;

    void main() {
        gl_Position = vec4(pos + offset, 0, 1);
        texcoord = uv;
    }"#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec2 texcoord;

    uniform sampler2D tex;

    void main() {
        gl_FragColor = vec4(texture2D(tex, texcoord).zyx, 1.0);
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: vec![UniformDesc::new("offset", UniformType::Float2)],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        pub offset: (f32, f32),
    }
}

fn main() {
    miniquad::start(conf::Conf {
        window_width: 1280,
        window_height: 800,
        window_title: "generate_map".to_string(),
        ..Default::default()
    }, |mut ctx| {
        UserData::owning(Stage::new(&mut ctx), ctx)
    });
}