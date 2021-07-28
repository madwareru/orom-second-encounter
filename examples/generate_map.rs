use miniquad::*;
use rom_media_rs::image_rendering::bmp_sprite_decorators::{TrueColorSurfaceSprite, FastBlended};
use rom_media_rs::image_rendering::blittable::{Blittable, BlitBuilder};
use std::io::Cursor;
use rom_res_rs::ResourceFile;
use rom_loaders_rs::images::sprite::BmpSprite;
use simple_tiled_wfc::grid_generation::{WfcModule, WfcContext, WfcEntropyHeuristic, DefaultEntropyHeuristic, DefaultEntropyChoiceHeuristic, WfcEntropyChoiceHeuristic};
use std::collections::VecDeque;
use bitsetium::{BitSearch, BitEmpty, BitSet, BitIntersection, BitUnion, BitTestNone};
use std::hash::Hash;
use simple_tiled_wfc::{get_bits_set_count, BitsIterator};
use rand::{thread_rng, Rng};

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

const fn get_tool_color(tool: u8) -> (f32, f32, f32) {
    match tool {
        DIRT => (0.5, 0.5, 0.55),
        GRASS => (0.0, 0.7, 0.3),
        ROAD => (0.4, 0.4, 0.4),
        SAND => (0.7, 0.7, 0.0),
        SAVANNAH => (0.0, 0.7, 0.7),
        HIGH_ROCKS => (0.4, 0.4, 0.0),
        MOUNTAIN => (1.0, 1.0, 1.0),
        WATER => (0.0, 0.0, 0.7),
        DIRT_2 => (0.65, 0.6, 0.6),
        _ => (0.0, 0.4, 0.7)
    }
}

const SCREEN_WIDTH: i32 = 1280;
const SCREEN_HEIGHT: i32 = 800;

const WIDTH: usize = 40; //80;
const HEIGHT: usize = 25; //50;

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

const fn manhattan(x1: usize, y1: usize, x2: usize, y2: usize) -> usize {
    (x1 as i64 - x2 as i64).abs() as usize + (y1 as i64 - y2 as i64).abs() as usize
}

struct LeastDistanceHeuristic {
    row: usize,
    column: usize,
}

impl<TBitSet> WfcEntropyHeuristic<TBitSet> for LeastDistanceHeuristic
    where TBitSet:
    BitSearch + BitEmpty + BitSet + BitIntersection +
    BitUnion + BitTestNone + Hash + Eq + Copy + BitIntersection<Output = TBitSet> +
    BitUnion<Output = TBitSet>
{
    fn choose_next_collapsed_slot(
        &self,
        width: usize,
        _height: usize,
        _modules: &[WfcModule<TBitSet>],
        available_indices: &[usize]
    ) -> usize {
        let (mut min_id, mut min_distance) = (available_indices.len() - 1, usize::MAX);
        for i in 0..available_indices.len() {
            let idx = available_indices[i];
            let row = idx / width;
            let column = idx % width;
            let d = manhattan(self.row, self.column, row, column);
            if d < min_distance {
                min_id = i;
                min_distance = d;
            }
        }
        min_id
    }
}

struct DrawingChoiceHeuristic<TBitSet>
    where TBitSet:
    BitSearch + BitEmpty + BitSet + BitIntersection +
    BitUnion + BitTestNone + Hash + Eq + Copy + BitIntersection<Output = TBitSet> +
    BitUnion<Output = TBitSet>
{
    fallback: DefaultEntropyChoiceHeuristic,
    preferable_bits: TBitSet
}
impl<TBitSet> WfcEntropyChoiceHeuristic<TBitSet> for DrawingChoiceHeuristic<TBitSet>
    where TBitSet:
    BitSearch + BitEmpty + BitSet + BitIntersection +
    BitUnion + BitTestNone + Hash + Eq + Copy + BitIntersection<Output = TBitSet> +
    BitUnion<Output = TBitSet>
{
    fn choose_least_entropy_bit(
        &self,
        width: usize,
        height: usize,
        row: usize,
        column: usize,
        modules: &[WfcModule<TBitSet>],
        slot_bits: &TBitSet
    ) -> usize {
        let intersection = self.preferable_bits.intersection(*slot_bits);
        if get_bits_set_count(&intersection) > 0 {
            let mut rng = thread_rng();
            let random_bit_id = rng.gen_range(0, get_bits_set_count(&intersection));
            let mut iterator = BitsIterator::new(&intersection);
            iterator.nth(random_bit_id).unwrap()
        } else {
            self.fallback.choose_least_entropy_bit(width, height, row, column, modules, slot_bits)
        }
    }
}


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
    black_square: TrueColorSurfaceSprite,
    stage_surface: TrueColorSurfaceSprite,
    tiles: Vec<TileInfo>,
    modules: Vec<WfcModule<CustomBitSet>>,
    mouse_pos: (f32, f32),
    tile_resolution: (f32, f32),
    window_size: (f32, f32),
    current_tool: u8,
    tile_modules: Vec<usize>,
    show_grid: bool,
    should_update: bool,
    should_update_iteratively: bool,
    update_history: VecDeque<(usize, CustomBitSet)>,
    mouse_down: bool,
    draw_queue: VecDeque<(usize, usize, u8)>
}

impl Stage {
    pub fn new(ctx: &mut Context) -> Stage {
        let tile_resolution = (
            WIDTH as f32,
            HEIGHT as f32
        );

        let mouse_pos = (
            0.0, 0.0
        );

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
        let black_square = TrueColorSurfaceSprite::new(32, 32);
        let mut stage_surface = TrueColorSurfaceSprite::new(SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize);

        let tile_definitions = &[
            (DIRT, GRASS, 0, 0),
            (DIRT, ROAD, 4, 0),
            (DIRT, SAND, 8, 0),
            (DIRT, SAVANNAH, 12, 0),
            (DIRT, HIGH_ROCKS, 0, 6),
            (HIGH_ROCKS, ROAD, 4, 6),
            (SAVANNAH, GRASS, 8, 6),
            (HIGH_ROCKS, MOUNTAIN, 12, 6),
            (DIRT, WATER, 0, 12),
            (DIRT, DIRT_2, 4, 12)
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

        let mut wfc_context: WfcContext<CustomBitSet> = WfcContext::new(
            &modules,
            WIDTH,
            HEIGHT,
            DefaultEntropyHeuristic::default(),
            DefaultEntropyChoiceHeuristic::default()
        );

        let tile_modules = wfc_context
            .collapse(100)
            .unwrap_or_else(|_| vec![4; WIDTH * HEIGHT]);

        for idx in 0..tile_modules.len() {
            let row = idx / WIDTH;
            let column = idx % WIDTH;
            let tile_id = tile_modules[idx];
            let tile_info = &tiles[tile_id];
            BlitBuilder::try_create(&mut stage_surface, &atlas)
                .expect("failed to create blit builder")
                .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                .with_dest_pos(column as i32 * 32, row as i32 * 32)
                .blit();
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
            black_square,
            stage_surface,
            tiles,
            tile_modules,
            modules,
            mouse_pos,
            mouse_down: false,
            tile_resolution,
            window_size: (SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32),
            show_grid: true,
            current_tool: GRASS,
            should_update: false,
            should_update_iteratively: false,
            update_history: VecDeque::new(),
            draw_queue: VecDeque::new()
        }
    }

    fn enqueue_draw(&mut self) {
        let row = self.mouse_pos.1 as usize;
        let column = self.mouse_pos.0 as usize;
        if self.draw_queue.is_empty() {
            self.draw_queue.push_back((row, column, self.current_tool));
        } else {
            let mut last = self.draw_queue.pop_back().unwrap();
            if last.0 == row && last.1 == column {
                if last.2 != self.current_tool {
                    last.2 = self.current_tool;
                }
                self.draw_queue.push_back(last);
            } else {
                self.draw_queue.push_back(last);
                self.draw_queue.push_back((row, column, self.current_tool))
            }
        }
    }
}

impl EventHandler for Stage {
    fn update(&mut self, ctx: &mut Context) {
        if !self.update_history.is_empty() {
            let mut steps_per_iteration = 10;
            let alpha_blended = FastBlended { decorated: &self.atlas };
            while steps_per_iteration > 0 && !self.update_history.is_empty() {
                let (next_idx, next_prop) = self.update_history.pop_front().unwrap();
                let row = next_idx / WIDTH;
                let column = next_idx % WIDTH;
                if get_bits_set_count(&next_prop) == 1 {
                    let tile_id = next_prop.find_first_set(0).unwrap();
                    let tile_info = &self.tiles[tile_id];
                    BlitBuilder::try_create(&mut self.stage_surface, &self.atlas)
                        .expect("failed to create blit builder")
                        .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                        .with_dest_pos(column as i32 * 32, row as i32 * 32)
                        .blit();
                } else {
                    BlitBuilder::try_create(
                        &mut self.stage_surface,
                        &self.black_square
                    )
                        .expect("failed to create blit builder")
                        .with_dest_pos(column as i32 * 32, row as i32 * 32)
                        .blit();

                    for tile_id in BitsIterator::new(&next_prop) {
                        BlitBuilder::try_create(
                            &mut self.stage_surface,
                            &alpha_blended
                        )
                            .expect("failed to create blit builder")
                            .with_source_subrect(self.tiles[tile_id].tile_x, self.tiles[tile_id].tile_y, 32, 32)
                            .with_dest_pos(column as i32 * 32, row as i32 * 32)
                            .blit();
                    }
                }
                steps_per_iteration -= 1;
            }
            if self.update_history.is_empty() {
                for idx in 0..self.tile_modules.len() {
                    let row = idx / WIDTH;
                    let column = idx % WIDTH;
                    let tile_id = self.tile_modules[idx];
                    let tile_info = &self.tiles[tile_id];
                    BlitBuilder::try_create(&mut self.stage_surface, &self.atlas)
                        .expect("failed to create blit builder")
                        .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                        .with_dest_pos(column as i32 * 32, row as i32 * 32)
                        .blit();
                }
            }
            let casted = bytemuck::cast_slice(self.stage_surface.color_data());
            self.bindings.images[0].update(ctx, casted);
            return; //while we are showing a traversing we don't want anything else to be done
        }
        if self.should_update_iteratively || self.should_update {
            let mut wfc_context: WfcContext<CustomBitSet> = WfcContext::new(
                &self.modules,
                WIDTH,
                HEIGHT,
                DefaultEntropyHeuristic::default(),
                DefaultEntropyChoiceHeuristic::default()
            );

            let res = wfc_context.collapse(10);

            if self.should_update {
                if res.is_ok(){
                    self.tile_modules = res.unwrap_or_else(|_| vec![4; WIDTH * HEIGHT]);
                    for idx in 0..self.tile_modules.len() {
                        let row = idx / WIDTH;
                        let column = idx % WIDTH;
                        let tile_id = self.tile_modules[idx];
                        let tile_info = &self.tiles[tile_id];
                        BlitBuilder::try_create(&mut self.stage_surface, &self.atlas)
                            .expect("failed to create blit builder")
                            .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                            .with_dest_pos(column as i32 * 32, row as i32 * 32)
                            .blit();
                    }

                    let casted = bytemuck::cast_slice(self.stage_surface.color_data());
                    self.bindings.images[0].update(ctx, casted);
                }
            } else {
                if res.is_ok() {
                    self.tile_modules = res.unwrap_or_else(|_| vec![4; WIDTH * HEIGHT]);
                    self.update_history = wfc_context.become_history();
                }
            }
            self.should_update = false;
            self.should_update_iteratively = false;
        }
        while !self.draw_queue.is_empty() {
            let (next_row, next_column, tool) = self.draw_queue.pop_front().unwrap();
            let mut preferable_bits = CustomBitSet::empty();
            let tool_tile = match tool {
                GRASS => 4,
                ROAD => 28,
                SAND => 52,
                SAVANNAH => 76,
                HIGH_ROCKS => 100,
                MOUNTAIN => 172,
                WATER => 196,
                DIRT_2 => 220,
                _ => 13
            };
            if tool != DIRT {
                let offset = tool_tile - 4;
                for ix in 0..24 {
                    preferable_bits.set(offset + ix);
                }
            }
            let mut wfc_context = WfcContext::from_existing_collapse(
                &self.modules,
                WIDTH,
                HEIGHT,
                LeastDistanceHeuristic{ row: next_row, column: next_column},
                DrawingChoiceHeuristic{
                    fallback: DefaultEntropyChoiceHeuristic::default(),
                    preferable_bits
                },
                &self.tile_modules
            );

            match wfc_context.local_collapse(next_row, next_column, tool_tile) {
                Ok(new_tile_modules) => {
                    for idx in 0..self.tile_modules.len() {
                        let row = idx / WIDTH;
                        let column = idx % WIDTH;
                        if self.tile_modules[idx] == new_tile_modules[idx] { continue; }
                        let tile_id = new_tile_modules[idx];
                        let tile_info = &self.tiles[tile_id];
                        BlitBuilder::try_create(&mut self.stage_surface, &self.atlas)
                            .expect("failed to create blit builder")
                            .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                            .with_dest_pos(column as i32 * 32, row as i32 * 32)
                            .blit();
                    }
                    self.tile_modules = new_tile_modules;
                },
                Err(_) => {}
            }
        }

        let casted = bytemuck::cast_slice(self.stage_surface.color_data());
        self.bindings.images[0].update(ctx, casted);
        self.should_update = false;
    }

    fn draw(&mut self, ctx: &mut Context) {
        ctx.begin_default_pass(Default::default());

        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);

        ctx.apply_uniforms(&shader::Uniforms {
            offset: (0.0, 0.0),
            mouse_pos: self.mouse_pos,
            tile_resolution: self.tile_resolution,
            grid_color: if self.show_grid {(0.0, 0.4, 0.7)} else {(0.0, 0.0, 0.0)} ,
            tool_color: get_tool_color(self.current_tool)
        });

        ctx.draw(0, 6, 1);

        ctx.end_render_pass();

        ctx.commit_frame();
    }

    fn resize_event(&mut self, _: &mut Context, width: f32, height: f32) {
        self.window_size = (width, height);
    }

    fn mouse_motion_event(&mut self, ctx: &mut Context, x: f32, y: f32) {
        self.mouse_pos = (
            (x / ctx.dpi_scale() / self.window_size.0 * WIDTH as f32).trunc(),
            (y / ctx.dpi_scale() / self.window_size.1 * HEIGHT as f32).trunc()
        );
        if self.mouse_down {
            self.enqueue_draw();
        }
    }

    fn mouse_button_down_event(
        &mut self,
        _ctx: &mut Context,
        button: MouseButton,
        _x: f32,
        _y: f32,
    ) {
        if let MouseButton::Left = button {
            self.mouse_down = true;
            self.enqueue_draw();
        }
    }

    fn mouse_button_up_event(
        &mut self,
        _ctx: &mut Context,
        button: MouseButton,
        _x: f32,
        _y: f32,
    ) {
        if let MouseButton::Left = button {
            self.mouse_down = false;
        }
    }

    fn key_up_event(&mut self, ctx: &mut Context, keycode: KeyCode, keymods: KeyMods) {
        match keycode {
            KeyCode::Key0 => self.current_tool = DIRT,
            KeyCode::Key1 => self.current_tool = GRASS,
            KeyCode::Key2 => self.current_tool = ROAD,
            KeyCode::Key3 => self.current_tool = SAND,
            KeyCode::Key4 => self.current_tool = SAVANNAH,
            KeyCode::Key5 => self.current_tool = HIGH_ROCKS,
            KeyCode::Key6 => self.current_tool = MOUNTAIN,
            KeyCode::Key7 => self.current_tool = WATER,
            KeyCode::Key8 => self.current_tool = DIRT_2,
            KeyCode::Space => self.show_grid = !self.show_grid,
            KeyCode::Enter => {
                if keymods.shift {
                    self.should_update_iteratively = true;
                } else {
                    self.should_update = true
                }
            },
            KeyCode::Escape =>  ctx.quit(),
            _ => {}
        }
    }
}


mod shader {
    use miniquad::*;

    pub const VERTEX: &str = //language=glsl
    r#"#version 100
    attribute vec2 pos;
    attribute vec2 uv;

    uniform vec2 offset;

    varying lowp vec2 texcoord;

    void main() {
        gl_Position = vec4(pos + offset, 0, 1);
        texcoord = uv;
    }"#;

    pub const FRAGMENT: &str = //language=glsl
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

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: vec![
                    UniformDesc::new("offset", UniformType::Float2),
                    UniformDesc::new("mouse_pos", UniformType::Float2),
                    UniformDesc::new("tile_resolution", UniformType::Float2),
                    UniformDesc::new("grid_color", UniformType::Float3),
                    UniformDesc::new("tool_color", UniformType::Float3),
                ],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        pub offset: (f32, f32),
        pub mouse_pos: (f32, f32),
        pub tile_resolution: (f32, f32),
        pub grid_color: (f32, f32, f32),
        pub tool_color: (f32, f32, f32),
    }
}

fn main() {
    miniquad::start(conf::Conf {
        window_resizable: false,
        window_width: SCREEN_WIDTH,
        window_height: SCREEN_HEIGHT,
        window_title: "generate_map".to_string(),
        high_dpi: true,
        ..Default::default()
    }, |mut ctx| {
        UserData::owning(Stage::new(&mut ctx), ctx)
    });
}
