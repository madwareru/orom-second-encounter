use {
    rom_res_rs::ResourceFile,
    rom_loaders_rs::images::sprite::BmpSprite,
    rom_media_rs::image_rendering::{
        bmp_sprite_decorators::TrueColorSurfaceSprite,
        blittable::BlitBuilder
    },
    orom_miniquad::{Texture, TextureParams, TextureFormat, TextureWrap, FilterMode, Context},
    crate::constants::{GRAPHICS_RES, GUI_TEXTURE_BYTES, INFO_TEXT_BYTES},
    std::io::Cursor
};

pub fn load_atlas_texture() -> TrueColorSurfaceSprite {
    let atlas = {
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
        for i in 0..graphics_resources.len() {
            let x = 32 * (i % 16);
            let y = 6 * 32 * (i / 16);

            BlitBuilder::try_create(&mut atlas, &graphics_resources[i])
                .expect("failed to create blit builder")
                .with_source_subrect(0, 0, 32, 6 * 32)
                .with_dest_pos(x as i32, y as i32)
                .blit();
        }
        atlas
    };
    atlas
}

pub fn load_gui_textures(ctx: &mut Context) -> Vec<Texture> {
    GUI_TEXTURE_BYTES
        .iter()
        .map(|it| {
            let mut cursor = Cursor::new(*it);
            let sprite = BmpSprite::read_from(&mut cursor).unwrap();
            match sprite {
                BmpSprite::TrueColor { width, height, colors } => {
                    let casted = bytemuck::cast_slice(&colors);
                    let mut bytes = Vec::with_capacity(width*height);
                    for offset in 0..width*height {
                        let offset = offset * 4;
                        bytes.push(casted[offset + 2]);
                        bytes.push(casted[offset + 1]);
                        bytes.push(casted[offset]);
                        bytes.push(casted[offset + 3]);
                    }
                    Texture::from_data_and_format(
                        ctx,
                        &bytes,
                        TextureParams {
                            format: TextureFormat::RGBA8,
                            wrap: TextureWrap::Clamp,
                            filter: FilterMode::Linear,
                            width: width as u32,
                            height: height as u32
                        }
                    )
                }
                _ => unreachable!()
            }
        }).collect()
}

pub fn load_info_text_texture(ctx: &mut Context) -> Texture {
    let mut cursor = Cursor::new(INFO_TEXT_BYTES);
    let sprite = BmpSprite::read_from(&mut cursor).unwrap();
    match sprite {
        BmpSprite::TrueColor { width, height, colors } => {
            let casted = bytemuck::cast_slice(&colors);
            let mut bytes = Vec::with_capacity(width*height);
            for offset in 0..width*height {
                let offset = offset * 4;
                bytes.push(casted[offset]);
            }
            Texture::from_data_and_format(
                ctx,
                &bytes,
                TextureParams {
                    format: TextureFormat::Alpha,
                    wrap: TextureWrap::Clamp,
                    filter: FilterMode::Linear,
                    width: width as u32,
                    height: height as u32
                }
            )
        }
        _ => unreachable!()
    }
}