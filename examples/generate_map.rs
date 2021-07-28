use miniquad::*;
use egui_miniquad::*;
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
use egui::{Widget, Color32};
use std::sync::mpsc::{Receiver, Sender, channel};
use simple_tiled_wfc::errors::WfcError;
use std::thread;

const GRAPHICS_RES: &[u8] = include_bytes!("assets/GRAPHICS.RES");
const GROUND: u8 = 0;
const GRASS: u8 = 1;
const ROAD: u8 = 2;
const SAND: u8 = 3;
const SAVANNAH: u8 = 4;
const HIGH_ROCKS: u8 = 5;
const MOUNTAIN: u8 = 6;
const WATER: u8 = 7;
const DIRT: u8 = 8;

#[derive(PartialEq)]
enum IterationState {
    Idle,
    Collapsing,
    Presenting
}

const fn get_tool_color(tool: u8) -> (f32, f32, f32) {
    match tool {
        GROUND => (0.5, 0.4, 0.22),
        GRASS => (0.0, 0.7, 0.3),
        ROAD => (0.44, 0.4, 0.52),
        SAND => (0.7, 0.7, 0.0),
        SAVANNAH => (0.0, 0.7, 0.7),
        HIGH_ROCKS => (0.7, 0.6, 0.0),
        MOUNTAIN => (1.0, 1.0, 1.0),
        WATER => (0.4, 0.45, 0.8),
        DIRT => (0.65, 0.6, 0.6),
        _ => (0.0, 0.4, 0.7)
    }
}

const SCREEN_WIDTH: i32 = 1280;
const SCREEN_HEIGHT: i32 = 800;

const WIDTH: usize = 40;
const HEIGHT: usize = 25;

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
    egui: EguiMq,
    pipeline: Pipeline,
    bindings: Bindings,
    atlas: TrueColorSurfaceSprite,
    black_square: TrueColorSurfaceSprite,
    stage_surface: TrueColorSurfaceSprite,
    tiles: Vec<TileInfo>,
    modules: Vec<WfcModule<CustomBitSet>>,
    tile_selection: (usize, usize),
    tile_resolution: (f32, f32),
    window_size: (f32, f32),
    current_tool: u8,
    tile_modules: Vec<usize>,
    show_grid: bool,
    should_update: bool,
    should_update_iteratively: bool,
    iterative_results_receiver: Receiver<(usize, CustomBitSet)>,
    iterative_results_transmitter: Sender<(usize, CustomBitSet)>,
    compound_results_receiver: Receiver<Result<Vec<usize>, WfcError>>,
    compound_results_transmitter: Sender<Result<Vec<usize>, WfcError>>,
    mouse_down: bool,
    draw_queue: VecDeque<(usize, usize, u8)>,
    iterative_speed: i32,
    iterative_update_state: IterationState
}

impl Stage {
    pub fn new(ctx: &mut Context) -> Stage {
        let tile_resolution = (
            WIDTH as f32,
            HEIGHT as f32
        );

        let (iterative_results_transmitter, iterative_results_receiver) = channel();
        let (compound_results_transmitter, compound_results_receiver) = channel();

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
            (GROUND, GRASS, 0, 0),
            (GROUND, ROAD, 4, 0),
            (GROUND, SAND, 8, 0),
            (GROUND, SAVANNAH, 12, 0),
            (GROUND, HIGH_ROCKS, 0, 6),
            (HIGH_ROCKS, ROAD, 4, 6),
            (SAVANNAH, GRASS, 8, 6),
            (HIGH_ROCKS, MOUNTAIN, 12, 6),
            (GROUND, WATER, 0, 12),
            (GROUND, DIRT, 4, 12)
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
            DefaultEntropyChoiceHeuristic::default(),
            None
        );

        wfc_context.collapse(100, compound_results_transmitter.clone());

        let tile_modules = compound_results_receiver.recv()
            .unwrap()
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
            tile_selection: (0, 0),
            mouse_down: false,
            tile_resolution,
            window_size: (
                SCREEN_WIDTH as f32 * ctx.dpi_scale(),
                SCREEN_HEIGHT as f32 * ctx.dpi_scale()
            ),
            show_grid: true,
            current_tool: GRASS,
            should_update: false,
            should_update_iteratively: false,
            iterative_results_receiver,
            iterative_results_transmitter,
            compound_results_receiver,
            compound_results_transmitter,
            draw_queue: VecDeque::new(),
            egui: EguiMq::new(ctx),
            iterative_speed: 10,
            iterative_update_state: IterationState::Idle
        }
    }

    fn enqueue_draw(&mut self) {
        let row = self.tile_selection.1 as usize;
        let column = self.tile_selection.0 as usize;
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

    fn ui(&mut self) {
        let egui_ctx = self.egui.egui_ctx().clone();

        egui::Window::new("general")
            .min_width(130.0)
            .default_width(130.0)
            .resizable(false)
            .show(&egui_ctx, |ui| {
                {
                    ui.vertical_centered_justified(|ui| {
                        if ui.button("Collapse").clicked() {
                            self.should_update = true;
                        }
                        if ui.button("Collapse iteratively").clicked() {
                            self.should_update_iteratively = true;
                        }
                        ui.separator();
                        ui.label("Iterative speed:");
                        egui::Slider::new(&mut self.iterative_speed, 1..=300).ui(ui);
                        ui.separator();

                        if ui.button("Quit (esc)").clicked() {
                            std::process::exit(0);
                        }
                    });
                }
            });

        egui::Window::new("brush")
            .min_width(100.0)
            .default_width(100.0)
            .resizable(false)
            .show(&egui_ctx, |ui| {
            {
                ui.vertical_centered_justified(|ui| {
                    for setting in &[
                        (GROUND,     "Ground (0)"),
                        (GRASS,      "Grass (1)"),
                        (ROAD,       "Road (2)"),
                        (SAND,       "Sand (3)"),
                        (SAVANNAH,   "Savannah (4)"),
                        (HIGH_ROCKS, "High rocks (5)"),
                        (MOUNTAIN,   "Mountains (6)"),
                        (WATER,      "Water (7)"),
                        (DIRT,       "Dirt (8)")
                    ] {
                        let color = get_tool_color(setting.0);
                        let text_color = Color32::from_rgb(
                            (color.0 * 255.0).min(255.0) as u8,
                            (color.1 * 255.0).min(255.0) as u8,
                            (color.2 * 255.0).min(255.0) as u8
                        );
                        let fill_color = if setting.0 == self.current_tool {
                            Color32::from_rgb(0x40, 0x40, 0x40)
                        } else {
                            Color32::from_rgb(0x30, 0x30, 0x30)
                        };

                        if egui::Button::new(setting.1)
                            .fill(fill_color)
                            .text_color(text_color)
                            .ui(ui)
                            .clicked() {
                            self.current_tool = setting.0;
                        }
                    }
                });
            }
        });
    }
}

impl EventHandler for Stage {
    fn update(&mut self, ctx: &mut Context) {
        if self.iterative_update_state != IterationState::Idle {
            let mut steps_per_iteration = self.iterative_speed;
            let alpha_blended = FastBlended { decorated: &self.atlas };
            loop {
                if steps_per_iteration == 0 {
                    if self.iterative_update_state == IterationState::Collapsing
                    {
                        match self.compound_results_receiver.try_recv() {
                            Ok(Ok(r)) => {
                                self.tile_modules = r;
                                self.iterative_update_state = IterationState::Presenting;
                            }
                            Ok(Err(_)) => {
                                self.iterative_update_state = IterationState::Presenting;
                            }
                            _ => {}
                        }
                    }
                    break;
                }
                let possible_res = self.iterative_results_receiver.try_recv();
                if let Ok((next_idx, next_prop)) = possible_res {
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
                } else {
                    if self.iterative_update_state == IterationState::Presenting {
                        self.iterative_update_state = IterationState::Idle;
                    } else if self.iterative_update_state == IterationState::Collapsing {
                        match self.compound_results_receiver.try_recv() {
                            Ok(Ok(r)) => {
                                self.tile_modules = r;
                                self.iterative_update_state = IterationState::Idle;
                            }
                            Ok(Err(_)) => {
                                self.iterative_update_state = IterationState::Idle;
                            }
                            _ => {}
                        }
                    }
                    break;
                }
            }
            let casted = bytemuck::cast_slice(self.stage_surface.color_data());
            self.bindings.images[0].update(ctx, casted);

            return; //while we are showing a traversing we don't want anything else to be done
        }
        if self.should_update_iteratively {
            self.should_update_iteratively = false;
            self.iterative_update_state = IterationState::Collapsing;

            let tx1 = self.iterative_results_transmitter.clone();
            let tx2 = self.compound_results_transmitter.clone();
            let mdls = self.modules.clone();

            thread::spawn(move|| {
                let mdls = mdls;
                let mut wfc_context: WfcContext<CustomBitSet> = WfcContext::new(
                    &mdls,
                    WIDTH,
                    HEIGHT,
                    DefaultEntropyHeuristic::default(),
                    DefaultEntropyChoiceHeuristic::default(),
                    Some(tx1)
                );

                wfc_context.collapse(10, tx2);

                //let res = self.compound_results_receiver.recv().unwrap();

                //self.is_updating_iteratively.store(false, Ordering::Relaxed);
                // if res.is_ok() {
                //     self.tile_modules = res.unwrap_or_else(|_| vec![4; WIDTH * HEIGHT]);
                // }
            });
        }
        if self.should_update {
            let mut wfc_context: WfcContext<CustomBitSet> = WfcContext::new(
                &self.modules,
                WIDTH,
                HEIGHT,
                DefaultEntropyHeuristic::default(),
                DefaultEntropyChoiceHeuristic::default(),
                None
            );

            wfc_context.collapse(10, self.compound_results_transmitter.clone());

            let res = self.compound_results_receiver.recv().unwrap();
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
                DIRT => 220,
                _ => 13
            };
            if tool != GROUND {
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
                &self.tile_modules,
                None
            );

            wfc_context.local_collapse(
                next_row,
                next_column,
                tool_tile,
                self.compound_results_transmitter.clone()
            );

            match self.compound_results_receiver.recv().unwrap() {
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
            mouse_pos: (self.tile_selection.0 as f32, self.tile_selection.1 as f32),
            tile_resolution: self.tile_resolution,
            grid_color: if self.show_grid {(0.0, 0.4, 0.7)} else {(0.0, 0.0, 0.0)} ,
            tool_color: get_tool_color(self.current_tool)
        });

        ctx.draw(0, 6, 1);

        ctx.end_render_pass();

        self.egui.begin_frame(ctx);
        self.ui();
        self.egui.end_frame(ctx);

        // Draw things behind egui here

        self.egui.draw(ctx);

        // Draw things in front of egui here

        ctx.commit_frame();
    }

    fn resize_event(&mut self, _: &mut Context, width: f32, height: f32) {
        self.window_size = (width, height);
    }

    fn mouse_motion_event(&mut self, ctx: &mut Context, x: f32, y: f32) {
        self.egui.mouse_motion_event(ctx, x, y);
        if !self.egui.egui_ctx().is_pointer_over_area() {
            let tile_selection = (
                (x / self.window_size.0 * WIDTH as f32)
                    .trunc()
                    .max(0.0) as usize,
                (y / self.window_size.1 * HEIGHT as f32)
                    .trunc()
                    .max(0.0) as usize
            );

            self.tile_selection = (
                tile_selection.0.min(WIDTH - 1),
                tile_selection.1.min(HEIGHT - 1)
            );
            if self.mouse_down {
                self.enqueue_draw();
            }
        }
    }

    fn mouse_wheel_event(&mut self, ctx: &mut Context, dx: f32, dy: f32) {
        self.egui.mouse_wheel_event(ctx, dx, dy);
    }

    fn mouse_button_down_event(
        &mut self,
        ctx: &mut Context,
        button: MouseButton,
        x: f32,
        y: f32,
    ) {
        self.egui.mouse_button_down_event(ctx, button, x, y);
        if !self.egui.egui_ctx().is_pointer_over_area() {
            if let MouseButton::Left = button {
                self.mouse_down = true;
                self.enqueue_draw();
            }
        }
    }

    fn mouse_button_up_event(
        &mut self,
        ctx: &mut Context,
        button: MouseButton,
        x: f32,
        y: f32,
    ) {
        self.egui.mouse_button_up_event(ctx, button, x, y);
        if !self.egui.egui_ctx().is_pointer_over_area() {
            if let MouseButton::Left = button {
                self.mouse_down = false;
            }
        }
    }

    fn char_event(&mut self, _ctx: &mut Context, character: char, _keymods: KeyMods, _repeat: bool) {
        self.egui.char_event(character);
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: KeyCode, keymods: KeyMods, _repeat: bool) {
        self.egui.key_down_event(ctx, keycode, keymods);
    }

    fn key_up_event(&mut self, ctx: &mut Context, keycode: KeyCode, keymods: KeyMods) {
        self.egui.key_up_event(keycode, keymods);
        if self.egui.egui_ctx().wants_keyboard_input() { return; }
        match keycode {
            KeyCode::Key0 => self.current_tool = GROUND,
            KeyCode::Key1 => self.current_tool = GRASS,
            KeyCode::Key2 => self.current_tool = ROAD,
            KeyCode::Key3 => self.current_tool = SAND,
            KeyCode::Key4 => self.current_tool = SAVANNAH,
            KeyCode::Key5 => self.current_tool = HIGH_ROCKS,
            KeyCode::Key6 => self.current_tool = MOUNTAIN,
            KeyCode::Key7 => self.current_tool = WATER,
            KeyCode::Key8 => self.current_tool = DIRT,
            KeyCode::Space => self.show_grid = !self.show_grid,
            KeyCode::Escape => ctx.quit(),
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
        sample_count: 0,
        ..Default::default()
    }, |mut ctx| {
        UserData::owning(Stage::new(&mut ctx), ctx)
    });
}
