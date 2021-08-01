mod constants;
mod heuristics;
mod tiling;
mod shaders;
mod resource_loading;

use {
    std::collections::VecDeque,
    std::sync::mpsc::{Receiver, Sender, channel},
    std::thread,
    crate::{
        constants::*,
        heuristics::*,
        tiling::*,
        resource_loading::*
    },
    miniquad::*,
    egui_miniquad::*,
    egui::{Color32, TextureId, TextStyle},
    bitsetium::{BitSearch, BitEmpty, BitSet},
    rom_media_rs::image_rendering::{
        bmp_sprite_decorators::{TrueColorSurfaceSprite, FastBlended},
        blittable::{Blittable, BlitBuilder}
    },
    simple_tiled_wfc::{get_bits_set_count, BitsIterator, errors::WfcError},
    simple_tiled_wfc::grid_generation::{
        WfcModule,
        WfcContext,
        DefaultEntropyHeuristic,
        DefaultEntropyChoiceHeuristic
    }
};
use egui::{FontDefinitions, FontFamily, Align2};

#[derive(PartialEq)]
enum IterationState {
    Idle,
    Collapsing,
    Presenting
}

#[derive(PartialEq)]
enum GeneralCommand {
    Collapse(CustomBitSet),
    CollapseIteratively(CustomBitSet)
}

type CustomBitSet = [u8; 30];

struct Surfaces {
    atlas: TrueColorSurfaceSprite,
    black_square: TrueColorSurfaceSprite,
    stage_surface: TrueColorSurfaceSprite
}

struct Stage {
    egui: EguiMq,
    tilemap_bindings: Bindings,
    tilemap_pipeline: Pipeline,
    info_text_bindings: Bindings,
    info_text_pipeline: Pipeline,
    surfaces: Surfaces,
    terrain_gui_textures: Vec<Texture>,
    tiles: Vec<TileInfo>,
    modules: Vec<WfcModule<CustomBitSet>>,
    available_tiles: AvailableTiles,
    tile_selection: (usize, usize),
    tile_resolution: (f32, f32),
    current_tool: u8,
    tile_modules: Vec<usize>,
    show_grid: bool,
    show_ui: bool,
    iterative_results_receiver: Receiver<(usize, CustomBitSet)>,
    iterative_results_transmitter: Sender<(usize, CustomBitSet)>,
    compound_results_receiver: Receiver<Result<Vec<usize>, WfcError>>,
    compound_results_transmitter: Sender<Result<Vec<usize>, WfcError>>,
    mouse_down: bool,
    command_queue: VecDeque<GeneralCommand>,
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

        let black_square = TrueColorSurfaceSprite::new(32, 32);
        let atlas = load_atlas_texture();
        let tiles = make_tiling_lookup();
        let modules = make_module_set(&tiles);

        let (stage_surface, tile_modules) = {
            let mut stage_surface = TrueColorSurfaceSprite::new(
                SCREEN_WIDTH as usize,
                SCREEN_HEIGHT as usize
            );
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
            (stage_surface, tile_modules)
        };

        let terrain_gui_textures = load_gui_textures(ctx);

        let tilemap_bindings = {
            let texture = {
                let casted = bytemuck::cast_slice(stage_surface.color_data());
                Texture::from_data_and_format(
                    ctx,
                    &casted,
                    TextureParams {
                        format: TextureFormat::RGBA8,
                        wrap: TextureWrap::Clamp,
                        filter: FilterMode::Linear,
                        width: stage_surface.get_width() as u32,
                        height: stage_surface.get_height() as u32
                    }
                )
            };

            let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &VERTICES);
            let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &[0u16, 1, 2, 0, 2, 3]);
            Bindings {
                vertex_buffers: vec![vertex_buffer],
                index_buffer,
                images: vec![texture],
            }
        };

        let tilemap_pipeline = {
            let shader = Shader::new(
                ctx,
                shaders::TILEMAP_VERTEX,
                shaders::TILEMAP_FRAGMENT,
                shaders::tilemap_meta()
            ).unwrap();

            Pipeline::new(
                ctx,
                &[BufferLayout::default()],
                &[
                    VertexAttribute::new("pos", VertexFormat::Float2),
                    VertexAttribute::new("uv", VertexFormat::Float2),
                ],
                shader,
            )
        };

        let info_text_bindings = {
            let texture = load_info_text_texture(ctx);

            let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &VERTICES);
            let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &[0u16, 1, 2, 0, 2, 3]);
            Bindings {
                vertex_buffers: vec![vertex_buffer],
                index_buffer,
                images: vec![texture],
            }
        };

        let info_text_pipeline = {
            let shader = Shader::new(
                ctx,
                shaders::TEXT_RENDER_VERTEX,
                shaders::TEXT_RENDER_FRAGMENT,
                shaders::info_text_meta()
            ).unwrap();

            Pipeline::with_params(
                ctx,
                &[BufferLayout::default()],
                &[
                    VertexAttribute::new("pos", VertexFormat::Float2),
                    VertexAttribute::new("uv", VertexFormat::Float2),
                ],
                shader,
                PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                }
            )
        };

        let egui = EguiMq::new(ctx);
        let mut fonts = FontDefinitions::default();
        fonts.font_data
            .insert("JetBrains Mono".to_owned(), std::borrow::Cow::Borrowed(JETBRAINS_MONO_FONT));
        fonts.fonts_for_family
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "JetBrains Mono".to_owned());
        fonts.fonts_for_family
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .insert(0, "JetBrains Mono".to_owned());
        egui.egui_ctx().set_fonts(fonts);

        Stage {
            tilemap_bindings,
            tilemap_pipeline,
            info_text_bindings,
            info_text_pipeline,
            surfaces: Surfaces { atlas, black_square, stage_surface},
            terrain_gui_textures,
            tiles,
            tile_modules,
            modules,
            available_tiles: AvailableTiles::default(),
            tile_selection: (0, 0),
            mouse_down: false,
            tile_resolution,
            show_grid: true,
            show_ui: true,
            current_tool: GRASS,
            iterative_results_receiver,
            iterative_results_transmitter,
            compound_results_receiver,
            compound_results_transmitter,
            command_queue: VecDeque::new(),
            draw_queue: VecDeque::new(),
            egui,
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
        if !self.show_ui { return; }

        let egui_ctx = self.egui.egui_ctx().clone();

        egui::Window::new("general")
            .default_width(130.0)
            .resizable(false)
            .anchor(Align2::LEFT_TOP, [0.0, 0.0])
            .show(&egui_ctx, |ui| {
                {
                    ui.vertical_centered_justified(|ui| {
                        if ui.button("Collapse").clicked() {
                            self.command_queue.push_back(
                                GeneralCommand::Collapse(self.available_tiles.make_bitset())
                            );
                        }
                        if ui.button("Collapse iteratively").clicked() {
                            self.command_queue.push_back(
                                GeneralCommand::CollapseIteratively(self.available_tiles.make_bitset())
                            );
                        }
                        ui.separator();
                        ui.add(egui::Checkbox::new(&mut self.show_grid, "Show grid (space)"));
                        ui.separator();
                        ui.label("Iterative speed:");
                        ui.add(egui::Slider::new(&mut self.iterative_speed, 1..=300));
                        ui.separator();

                        if ui.button("Quit (esc)").clicked() {
                            std::process::exit(0);
                        }
                    });
                }
            });

        egui::Window::new("brush")
            .min_width(40.0)
            .default_width(40.0)
            .resizable(false)
            .anchor(Align2::RIGHT_TOP, [0.0, 0.0])
            .show(&egui_ctx, |ui| {
                {
                    ui.vertical_centered_justified(|ui| {
                        for setting in &[
                            (LAND,       "     Land (1)"),
                            (GRASS,      "    Grass (2)"),
                            (PLATEAU,    "  Plateau (3)"),
                            (SAND,       "    Sands (4)"),
                            (SAVANNAH,   " Savannah (5)"),
                            (ROCKS,      "    Rocks (6)"),
                            (HIGH_ROCKS, "High rocks(7)"),
                            (WATER,      "     Water(8)"),
                            (ROAD,       "     Road (9)")
                        ] {
                            ui.horizontal(|ui| {
                                let color = get_tool_color(setting.0);
                                let coeff = if setting.0 == self.current_tool { 255.0 } else { 200.0 };
                                let text_color = Color32::from_rgb(
                                    (color.0 * coeff).min(coeff) as u8,
                                    (color.1 * coeff).min(coeff) as u8,
                                    (color.2 * coeff).min(coeff) as u8
                                );

                                let tint_color = if setting.0 == self.current_tool {
                                    Color32::from_rgb(0xFF, 0xFF, 0xFF)
                                } else {
                                    Color32::from_rgb(0x77, 0x77, 0xAA)
                                };

                                let texture_id = self
                                    .terrain_gui_textures[setting.0 as usize]
                                    .gl_internal_id() as u64;

                                let image_button = egui::ImageButton::new(
                                    TextureId::User(texture_id),
                                    [24.0, 24.0]
                                ).tint(tint_color);

                                ui.add(egui::Label::new(setting.1)
                                    .strong()
                                    .text_style(TextStyle::Monospace)
                                    .text_color(text_color));
                                if ui.add(image_button).clicked() {
                                    self.current_tool = setting.0;
                                }
                            });
                        }
                    });
                }
            });

        egui::Window::new("generation settings")
            .anchor(Align2::CENTER_BOTTOM, [0.0, 0.0])
            .show(&egui_ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[LAND as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.land, "");
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[GRASS as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.grass, "");
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[PLATEAU as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.plateau, "");
                    });
                    ui.spacing();
                    ui.horizontal(|ui| {
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[SAND as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.sand, "");
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[SAVANNAH as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.savannah, "");
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[ROCKS as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.rocks, "");
                    });
                    ui.spacing();
                    ui.horizontal(|ui| {
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[HIGH_ROCKS as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.high_rocks, "");
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[WATER as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.water, "");
                        ui.image(
                            TextureId::User(
                                self.terrain_gui_textures[ROAD as usize]
                                    .gl_internal_id() as u64
                            ),
                            [20.0, 20.0]
                        );
                        ui.checkbox(&mut self.available_tiles.road, "");
                    });
                });
            });
    }
}

impl EventHandler for Stage {
    fn update(&mut self, ctx: &mut Context) {
        if self.iterative_update_state != IterationState::Idle {
            self.flush_iterated_queue(ctx);
            return; // While we are showing a traversing we don't want anything else to be done
        }
        if let Some(command) = self.command_queue.pop_front() {
            match command {
                GeneralCommand::Collapse(tileset) => {
                    self.collapse(ctx, tileset);
                }
                GeneralCommand::CollapseIteratively(tileset) => {
                    self.initiate_iterative_collapse(tileset);
                }
            }
            return; // Process one command at a time. Do not flush draw queue if there was a command
        }
        if !self.draw_queue.is_empty() {
            self.flush_draw_queue(ctx);
        }
    }

    fn draw(&mut self, ctx: &mut Context) {
        {
            ctx.begin_default_pass(Default::default());
            ctx.apply_pipeline(&self.tilemap_pipeline);
            ctx.apply_bindings(&self.tilemap_bindings);

            ctx.apply_uniforms(&shaders::TilemapUniforms {
                mouse_pos: (self.tile_selection.0 as f32, self.tile_selection.1 as f32),
                tile_resolution: self.tile_resolution,
                grid_color: if self.show_grid {(0.0, 0.4, 0.7)} else {(0.0, 0.0, 0.0)} ,
                tool_color: get_tool_color(self.current_tool)
            });

            ctx.draw(0, 6, 1);

            ctx.end_render_pass();
        }

        {
            ctx.begin_default_pass(PassAction::Nothing);

            ctx.apply_pipeline(&self.info_text_pipeline);
            ctx.apply_bindings(&self.info_text_bindings);

            ctx.apply_uniforms(&shaders::InfoTextUniforms {
                pos: (0.001, 0.979),
                scale: (0.1977 * 0.7, 0.034 * 0.7),
                font_color: (0.15, 0.15, 0.16)
            });

            ctx.draw(0, 6, 1);

            ctx.end_render_pass();
        }
        {
            ctx.begin_default_pass(PassAction::Nothing);

            ctx.apply_pipeline(&self.info_text_pipeline);
            ctx.apply_bindings(&self.info_text_bindings);

            ctx.apply_uniforms(&shaders::InfoTextUniforms {
                pos: (-0.001, 0.981),
                scale: (0.1977 * 0.7, 0.034 * 0.7),
                font_color: (0.6, 0.6, 0.82)
            });

            ctx.draw(0, 6, 1);

            ctx.end_render_pass();
        }


        self.egui.begin_frame(ctx);
        self.ui();
        self.egui.end_frame(ctx);

        // Draw things behind egui here

        self.egui.draw(ctx);

        // Draw things in front of egui here

        ctx.commit_frame();
    }

    fn mouse_motion_event(&mut self, ctx: &mut Context, x: f32, y: f32) {
        let screen_size = ctx.screen_size();
        self.egui.mouse_motion_event(ctx, x, y);
        if !self.egui.egui_ctx().is_pointer_over_area() {
            let tile_selection = (
                (x / screen_size.0 * WIDTH as f32)
                    .trunc()
                    .max(0.0) as usize,
                (y / screen_size.1 * HEIGHT as f32)
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

    fn char_event(&mut self, _ctx: &mut Context, character: char, _keymods: KeyMods, repeat: bool) {
        self.egui.char_event(character);
        if self.egui.egui_ctx().wants_keyboard_input() { return; }
        if character == '~' && !repeat {
            self.show_ui = !self.show_ui;
        }
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: KeyCode, keymods: KeyMods, _repeat: bool) {
        self.egui.key_down_event(ctx, keycode, keymods);
    }

    fn key_up_event(&mut self, ctx: &mut Context, keycode: KeyCode, keymods: KeyMods) {
        self.egui.key_up_event(keycode, keymods);
        match keycode {
            KeyCode::Key1 => self.current_tool = LAND,
            KeyCode::Key2 => self.current_tool = GRASS,
            KeyCode::Key3 => self.current_tool = PLATEAU,
            KeyCode::Key4 => self.current_tool = SAND,
            KeyCode::Key5 => self.current_tool = SAVANNAH,
            KeyCode::Key6 => self.current_tool = ROCKS,
            KeyCode::Key7 => self.current_tool = HIGH_ROCKS,
            KeyCode::Key8 => self.current_tool = WATER,
            KeyCode::Key9 => self.current_tool = ROAD,
            KeyCode::Space => self.show_grid = !self.show_grid,
            KeyCode::Escape => ctx.quit(),
            _ => {}
        }
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

impl Stage { // Drawing related stuff
    fn flush_iterated_queue(&mut self, ctx: &mut Context) {
        let mut steps_per_iteration = self.iterative_speed;
        let alpha_blended = FastBlended { decorated: &self.surfaces.atlas };
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
                    BlitBuilder::try_create(&mut self.surfaces.stage_surface, &self.surfaces.atlas)
                        .expect("failed to create blit builder")
                        .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                        .with_dest_pos(column as i32 * 32, row as i32 * 32)
                        .blit();
                } else {
                    BlitBuilder::try_create(
                        &mut self.surfaces.stage_surface,
                        &self.surfaces.black_square
                    )
                        .expect("failed to create blit builder")
                        .with_dest_pos(column as i32 * 32, row as i32 * 32)
                        .blit();

                    for tile_id in BitsIterator::new(&next_prop) {
                        BlitBuilder::try_create(
                            &mut self.surfaces.stage_surface,
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
        let casted = bytemuck::cast_slice(self.surfaces.stage_surface.color_data());
        self.tilemap_bindings.images[0].update(ctx, casted);
    }

    fn initiate_iterative_collapse(&mut self, tileset: CustomBitSet) {
        self.iterative_update_state = IterationState::Collapsing;

        let tx1 = self.iterative_results_transmitter.clone();
        let tx2 = self.compound_results_transmitter.clone();
        let modules = self.modules.clone();

        thread::spawn(move || {
            let mut wfc_context: WfcContext<CustomBitSet,
                DefaultEntropyHeuristic,
                StrictDrawingChoiceHeuristic<CustomBitSet>
            > = WfcContext::new(
                &modules,
                WIDTH,
                HEIGHT,
                DefaultEntropyHeuristic::default(),
                StrictDrawingChoiceHeuristic { preferable_bits: tileset },
                Some(tx1)
            );

            wfc_context.collapse(10, tx2);
        });
    }

    fn collapse(&mut self, ctx: &mut Context, tileset: CustomBitSet) {
        let mut wfc_context: WfcContext<CustomBitSet,
            DefaultEntropyHeuristic,
            StrictDrawingChoiceHeuristic<CustomBitSet>
        > = WfcContext::new(
            &self.modules,
            WIDTH,
            HEIGHT,
            DefaultEntropyHeuristic::default(),
            StrictDrawingChoiceHeuristic {
                preferable_bits: tileset
            },
            None
        );

        wfc_context.collapse(10, self.compound_results_transmitter.clone());
        if let Ok(tile_modules) = self.compound_results_receiver.recv().unwrap() {
            self.tile_modules = tile_modules;
            for idx in 0..self.tile_modules.len() {
                let row = idx / WIDTH;
                let column = idx % WIDTH;
                let tile_id = self.tile_modules[idx];
                let tile_info = &self.tiles[tile_id];
                BlitBuilder::try_create(&mut self.surfaces.stage_surface, &self.surfaces.atlas)
                    .expect("failed to create blit builder")
                    .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                    .with_dest_pos(column as i32 * 32, row as i32 * 32)
                    .blit();
            }

            let casted = bytemuck::cast_slice(self.surfaces.stage_surface.color_data());
            self.tilemap_bindings.images[0].update(ctx, casted);
        }
    }

    fn flush_draw_queue(&mut self, ctx: &mut Context) {
        while !self.draw_queue.is_empty() {
            let (next_row, next_column, tool) = self.draw_queue.pop_front().unwrap();
            let mut preferable_bits = CustomBitSet::empty();
            let tool_tile = match tool {
                GRASS => 4,
                PLATEAU => 28,
                SAND => 52,
                SAVANNAH => 76,
                ROCKS => 100,
                HIGH_ROCKS => 172,
                WATER => 196,
                ROAD => 220,
                _ => 13
            };
            if tool != LAND {
                let offset = tool_tile - 4;
                for ix in 0..24 {
                    preferable_bits.set(offset + ix);
                }
            }
            let mut wfc_context = WfcContext::from_existing_collapse(
                &self.modules,
                WIDTH,
                HEIGHT,
                LeastDistanceHeuristic { row: next_row, column: next_column },
                DrawingChoiceHeuristic {
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

            if let Ok(new_tile_modules) = self.compound_results_receiver.recv().unwrap() {
                for idx in 0..new_tile_modules.len() {
                    let row = idx / WIDTH;
                    let column = idx % WIDTH;
                    if self.tile_modules[idx] == new_tile_modules[idx] { continue; }
                    let tile_id = new_tile_modules[idx];
                    let tile_info = &self.tiles[tile_id];
                    BlitBuilder::try_create(&mut self.surfaces.stage_surface, &self.surfaces.atlas)
                        .expect("failed to create blit builder")
                        .with_source_subrect(tile_info.tile_x, tile_info.tile_y, 32, 32)
                        .with_dest_pos(column as i32 * 32, row as i32 * 32)
                        .blit();
                }
                self.tile_modules = new_tile_modules;
            }
        }

        let casted = bytemuck::cast_slice(self.surfaces.stage_surface.color_data());
        self.tilemap_bindings.images[0].update(ctx, casted);
    }
}
