use {
    std::io::Cursor,
    rom_res_rs::*,
    rom_media_rs::{
        image_rendering::{
            blittable::{BlitBuilder, Blittable},
            bmp_sprite_decorators::TrueColorSurfaceSprite,
            ingame_sprite_decorators::{PalettedSpriteRenderingScope}
        }
    },
    rom_loaders_rs::images::ingame_sprite::{
        read_image,
        ImageType,
        read_palette,
        read_raw_palette,
        DEFAULT_RAW_PALETTE_OFFSET
    }
};

const GRAPHICS_RES: &[u8] = include_bytes!("GRAPHICS.RES");
const BUFFER_SIZE: usize = 512;
const STAGE_ATLAS_SIZE: usize = 1024;
const ATLAS_SIZE: usize = 4096;

struct AtlasSubRect {
    pub atlas_id: usize,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub padding_l: u16,
    pub padding_r: u16,
    pub padding_t: u16,
    pub padding_b: u16,
    pub is_adjusted_to_shelf: bool
}

#[derive(Clone, Copy)]
struct ShelfInfo {
    pub y_position: i32,
    pub right: i32,
    pub height: i32,
}

pub struct Picture {
    width: usize,
    height: usize,
    colors: Vec<u32>
}

impl Picture {
    pub fn new(width: usize, height: usize) -> Self {
        Picture {
            width,
            height,
            colors: vec![0; width * height]
        }
    }
    pub fn mutate<'a, F>(&'a mut self, mutator: F)
        where F: FnOnce(&'a mut[u32], usize, usize) -> ()
    {
        mutator(&mut self.colors, self.width, self.height);
    }
    pub fn print_as_ppm(&self) {
        println!("P3");
        println!("{} {}", self.width, self.height);
        println!("255");
        for j in 0..self.height {
            for i in 0..self.width {
                let c = self.colors[j * self.width + i];
                let b = c & 0xFF; let c = c / 0x100;
                let g = c & 0xFF; let c = c / 0x100;
                let r = c & 0xFF; let c = c / 0x100;
                let a = c & 0xFF;

                let b = ((0x0C * (0xFF - a) + b * a) / 0xFF) & 0xFF;
                let g = ((0x08 * (0xFF - a) + g * a) / 0xFF) & 0xFF;
                let r = ((0x0A * (0xFF - a) + r * a) / 0xFF) & 0xFF;

                print!("{}\t{}\t{}\t", r, g, b);
            }
            println!()
        }
    }
}

mod sprite_files {
    pub const PATHS: &[&str] = &[
        "projectiles/acid/sprites.16a",
        "projectiles/bless/sprites.16a",
        "projectiles/chain/sprites.16a",
        "projectiles/curse/sprites.16a",
        "projectiles/drain/sprites.16a",
        "projectiles/fireball/sprites.16a",
        "projectiles/firebolt/sprites.16a",
        "projectiles/fireexpl/sprites.16a",
        "projectiles/firewall/sprites.16a",
        "projectiles/healing/sprites.16a",
        "projectiles/lightnin/sprites.16a",
        "projectiles/meteor/sprites.16a",
        "projectiles/p_air/sprites.16a",
        "projectiles/p_earth/sprites.16a",
        "projectiles/p_fire/sprites.16a",
        "projectiles/p_water/sprites.16a",
        "projectiles/poison/sprites.16a",
        "projectiles/poison_d/sprites.16a",
        "projectiles/shield/sprites.16a",
        "projectiles/smallxpl/sprites.16a",
        "projectiles/smoke0/sprites.16a",
        "projectiles/smoke1/sprites.16a",
        "projectiles/steam/sprites.16a",
        "projectiles/teleport/sprites.16a",
        "projectiles/wall/sprites.16a",
        "projectiles/xbowman/arrow.256",
        "projectiles/archer/arrow.256",
        "projectiles/catap1/sprites.256",
        "projectiles/catap2/sprites.256",
        "projectiles/firebolt/sprites.256",
        "projectiles/goblin/arrow.256",
        "projectiles/orc/arrow.256",
        "structures/bridge1v/house.256",
        "structures/bridge2/house.256",
        "structures/bridge3/house.256",
        "structures/bridge4/house.256",
        "structures/campfire/house.256",
        "structures/castle/house.256",
        "structures/cave/house.256",
        "structures/church/house.256",
        "structures/grave1/house.256",
        "structures/grave2/house.256",
        "structures/grave3/house.256",
        "structures/grave4/house.256",
        "structures/hangman/house.256",
        "units/monsters/ogre/sprites.256",
        "units/monsters/squirrel/sprites.256",
        "units/monsters/star/sprites.256",
        "objects/bush3/dead/sprites.256",
        "objects/bush3/sprites.256",
        "objects/elka1/dead/sprites.256",
        "objects/elka1/sprites.256",
        "objects/elka2/dead/sprites.256",
        "objects/elka2/sprites.256",
        "objects/elka3/dead/sprites.256",
        "objects/elka3/sprites.256",
        "objects/fence/sprites.256",
        "objects/iva1/dead/sprites.256",
        "objects/iva1/sprites.256",
        "objects/iva2/dead/sprites.256",
        "objects/iva2/sprites.256",
        "objects/iva3/dead/sprites.256",
        "objects/iva3/sprites.256",
        "objects/maple1/dead/sprites.256",
        "objects/maple1/sprites.256",
        "objects/maple2/dead/sprites.256",
        "objects/maple2/sprites.256",
        "objects/maple3/dead/sprites.256",
        "objects/maple3/sprites.256",
        "objects/oak1/dead/sprites.256",
        "objects/oak1/sprites.256",
        "objects/oak2/dead/sprites.256",
        "objects/oak2/sprites.256",
        "objects/oak3/dead/sprites.256",
        "objects/oak3/sprites.256",
        "objects/palka/sprites.256",
        "objects/pine1/dead/sprites.256",
        "objects/pine1/sprites.256",
        "objects/pine2/dead/sprites.256",
        "objects/pine2/sprites.256",
        "objects/pine3/dead/sprites.256",
        "objects/pine3/sprites.256",
        "objects/pointer/sprites.256",
        "objects/statue/sprites.256",
        "objects/stones/sprites.256",
        "objects/totem/sprites.256",
        "objects/vallen1/dead/sprites.256",
        "objects/vallen1/sprites.256",
        "objects/vallen2/dead/sprites.256",
        "objects/vallen2/sprites.256",
        "objects/vallen3/dead/sprites.256",
        "objects/vallen3/sprites.256",
        "units/monsters/troll/sprites.256",
        "units/monsters/turtle/sprites.256",
        "objects/bush2/dead/sprites.256",
        "objects/bush2/sprites.256",
        "objects/bones/sprites.256",
        "objects/bush1/dead/sprites.256",
        "objects/bush1/sprites.256",
        "units/monsters/legg/sprites.256",
        "units/monsters/ghost/sprites.256",
        "units/monsters/bat/sprites.256",
        "units/monsters/bee/sprites.256",
        "units/monsters/dragon/sprites.256",
        "units/heroes/archer/sprites.256",
        "units/heroes/axeman/sprites.256",
        "units/heroes/axeman2h/sprites.256",
        "units/heroes/axeman_/sprites.256",
        "units/heroes/clubman/sprites.256",
        "units/heroes/clubman_/sprites.256",
        "units/heroes/mage/sprites.256",
        "units/heroes/mage_st/sprites.256",
        "units/heroes/pikeman/sprites.256",
        "units/heroes/pikeman_/sprites.256",
        "units/heroes/swordsman/sprites.256",
        "units/heroes/swordsman2h/sprites.256",
        "units/heroes/swordsman_/sprites.256",
        "units/heroes/unarmed/sprites.256",
        "units/heroes/unarmed_/sprites.256",
        "units/heroes/xbowman/sprites.256",
        "units/heroes_l/archer/sprites.256",
        "units/heroes_l/axeman/sprites.256",
        "units/heroes_l/axeman2h/sprites.256",
        "units/heroes_l/axeman_/sprites.256",
        "units/heroes_l/clubman/sprites.256",
        "units/heroes_l/clubman_/sprites.256",
        "units/heroes_l/pikeman/sprites.256",
        "units/heroes_l/pikeman_/sprites.256",
        "units/heroes_l/swordsman/sprites.256",
        "units/heroes_l/swordsman2h/sprites.256",
        "units/heroes_l/swordsman_/sprites.256",
        "units/heroes_l/unarmed/sprites.256",
        "units/heroes_l/unarmed_/sprites.256",
        "units/heroes_l/xbowman/sprites.256",
        "units/humans/archer/archer.256",
        "units/humans/axeman/axeman.256",
        "units/humans/axeman_2hd/axeman_2hd.256",
        "units/humans/catapult1/sprites.256",
        "units/humans/catapult2/sprites.256",
        "units/humans/cavalrypike/sprites.256",
        "units/humans/cavalrysword/sprites.256",
        "units/humans/clubman/clubman.256",
        "units/humans/clubman_sh/clubman_sh.256",
        "units/humans/mage/sprites.256",
        "units/humans/mage_st/mage_st.256",
        "units/humans/pikeman_/sprites.256",
        "units/humans/swordsman/swordsman.256",
        "units/humans/swordsman2/swordsman2.256",
        "units/humans/swordsman_/swordsman_.256",
        "units/humans/unarmed/sprites.256",
        "units/humans/xbowman/xbowman.256",
        "units/monsters/orc_s/sprites.256",
        "units/monsters/orc/sprites.256",
        "units/monsters/goblin/sprites.256",
        "units/monsters/goblin_s/sprites.256",
        "structures/hut1/house.256",
        "structures/hut2/house.256",
        "structures/hut3/house.256",
        "structures/hut4/house.256",
        "structures/hut5/house.256",
        "structures/hut6/house.256",
        "structures/hut7(h3)/house.256",
        "structures/hut8(h3)/house.256",
        "structures/hut9(h2)/house.256",
        "structures/huta(h2)/house.256",
        "structures/hutb(o0)/house.256",
        "structures/hutc(o1)/house.256",
        "structures/hutd(o2)/house.256",
        "structures/hute(b0)/house.256",
        "structures/hutf(b1)/house.256",
        "structures/inn1/house.256",
        "structures/inn2/house.256",
        "structures/inn3/house.256",
        "structures/leg's/house.256",
        "structures/magic/house.256",
        "structures/mill1/house.256",
        "structures/mill2/house.256",
        "structures/mill3/house.256",
        "structures/ruins1/house.256",
        "structures/ruins2/house.256",
        "structures/ruins3/house.256",
        "structures/shop1/house.256",
        "structures/shop2/house.256",
        "structures/sphinx1/house.256",
        "structures/sphinx2/house.256",
        "structures/sphinx3/house.256",
        "structures/sphinx4/house.256",
        "structures/sphinx5/house.256",
        "structures/sphinx6/house.256",
        "structures/sphinx7/house.256",
        "structures/sphinx8/house.256",
        "structures/switch1/house.256",
        "structures/switch2/house.256",
        "structures/teleport/house.256",
        "structures/tower1/house.256",
        "structures/tower2/house.256",
        "structures/tower_1/house.256",
        "structures/tower_2/house.256",
        "structures/tower_m/house.256",
        "structures/tower_s1/house.256",
        "structures/tower_s2/house.256",
        "structures/train1/house.256",
        "structures/train2/house.256",
        "structures/train3/house.256",
        "structures/well1/house.256",
        "structures/well2/house.256",
        "structures/well3/house.256",
    ];
}

fn main() {
    let cursor = Cursor::new(GRAPHICS_RES);
    let mut resource_file = ResourceFile::new(cursor)
        .expect(&format!("failed to open GRAPHICS.RES"));

    let mut sub_rects = Vec::new();
    let stage_atlases = {
        let mut stage_atlases = Vec::new();
        let mut atlas_id = 0;

        let mut stage_atlas = TrueColorSurfaceSprite::new(STAGE_ATLAS_SIZE, STAGE_ATLAS_SIZE);

        let mut x_pos = 0;
        let mut y_pos = 0;
        let mut max_h = 0;

        let mut sp = TrueColorSurfaceSprite::new(
            BUFFER_SIZE,
            BUFFER_SIZE
        );
        let blk = TrueColorSurfaceSprite::new(
            BUFFER_SIZE,
            BUFFER_SIZE
        );

        let default_projectile_pal_resource = resource_file
            .get_resource_bytes("projectiles/projectiles.pal")
            .expect(&format!("failed to load resource {}", "projectiles/projectiles.pal"));
        let proj_cursor = &mut Cursor::new(default_projectile_pal_resource);

        let default_projectile_pal =
            read_raw_palette(proj_cursor, DEFAULT_RAW_PALETTE_OFFSET)
                .unwrap()
                .unwrap();

        for unit_path in sprite_files::PATHS {
            let unit_resource = resource_file
                .get_resource_bytes(unit_path)
                .expect(&format!("failed to load resource {}", unit_path));

            let image_type = if unit_path.ends_with("16a") {
                ImageType::Dot16a
            } else if unit_path.ends_with("16") {
                ImageType::Dot16
            } else {
                ImageType::Dot256
            };

            let unit_sprite =
                read_image(
                    &mut Cursor::new(unit_resource),
                    image_type
                ).expect(&format!("failed to load resource bmp content"));

            let palette =
                read_palette(
                    &mut Cursor::new(unit_resource),
                    image_type
                ).unwrap();

            for i in 0..unit_sprite.frames.len() {
                let frame = &(unit_sprite.frames[i]);
                BlitBuilder::try_create(&mut sp, &blk).unwrap()
                    .with_source_subrect(0, 0, frame.width as usize, frame.height as usize)
                    .blit(); // clear background

                let scope = &PalettedSpriteRenderingScope{
                    image_data: &unit_sprite,
                    palette: if let Some(pal) = &palette {
                        &pal
                    } else {
                        &default_projectile_pal
                    },
                    img_id: i
                };

                BlitBuilder::try_create(&mut sp, scope).unwrap().blit();
                let (mut min_i, mut min_j) = (frame.width as usize - 1, frame.height as usize - 1);
                let (mut max_i, mut max_j) = (0, 0);
                let colors = sp.color_data();
                for jj in 0..frame.height as usize {
                    for ii in 0..frame.width as usize {
                        let offset = sp.get_width() * jj + ii;
                        if colors[offset] == 0 { continue; }
                        min_i = min_i.min(ii);
                        min_j = min_j.min(jj);
                        max_i = max_i.max(ii);
                        max_j = max_j.max(jj);
                    }
                }
                if min_i >= max_i || min_j >= max_j {continue;}
                let true_h = max_j - min_j + 1;
                let true_w = max_i - min_i + 1;

                if (x_pos + true_w as i32) as usize >= STAGE_ATLAS_SIZE {
                    x_pos = 0;
                    y_pos += max_h;
                    max_h = 0;
                }

                if (y_pos + true_h as i32) as usize>= STAGE_ATLAS_SIZE {
                    atlas_id += 1;
                    x_pos = 0;
                    y_pos = 0;
                    max_h = 0;
                    let staged = stage_atlas;
                    stage_atlas = TrueColorSurfaceSprite::new(STAGE_ATLAS_SIZE, STAGE_ATLAS_SIZE);
                    stage_atlases.push(staged)
                }

                BlitBuilder::try_create(&mut stage_atlas, &sp)
                    .unwrap()
                    .with_dest_pos(x_pos, y_pos)
                    .with_source_subrect(min_i, min_j, true_w, true_h)
                    .blit();

                let sub_rect = AtlasSubRect {
                    atlas_id,
                    x: x_pos as usize,
                    y: y_pos as usize,
                    w: true_w as usize,
                    h: true_h as usize,
                    padding_l: min_i as u16,
                    padding_r: (frame.width as usize - true_w - min_i) as u16,
                    padding_t: min_j as u16,
                    padding_b: (frame.height as usize - true_h - min_j) as u16,
                    is_adjusted_to_shelf: false
                };

                sub_rects.push(sub_rect);

                x_pos += true_w as i32;
                max_h = max_h.max(true_h as i32);
            }
        }
        stage_atlases.push(stage_atlas);
        stage_atlases
    };

    sub_rects.sort_by(|l, r|{
        if l.h == r.h {
            r.w.cmp(&(l.w))
        }
        else {
            r.h.cmp(&(l.h))
        }
    });

    let (colors, _sub_rects) = {
        let mut shelfs: Vec<ShelfInfo> = Vec::new();
        let mut current_shelf = ShelfInfo {y_position: 0, right: 0, height: 0 };

        let mut new_colors = TrueColorSurfaceSprite::new(ATLAS_SIZE, ATLAS_SIZE);
        let mut new_sub_rects = Vec::new();

        for sub_rect in sub_rects.iter() {

            let true_h = sub_rect.h as usize;
            let true_w = sub_rect.w as usize;

            if let Some(matched_shelf) = shelfs
                .iter_mut()
                .find(|s| {
                    (s.height >= true_h as i32) &&
                        ((s.right + true_w as i32) as usize) < ATLAS_SIZE
                })
            {
                let y_pos = matched_shelf.y_position;
                let x_pos = matched_shelf.right;

                BlitBuilder::try_create(&mut new_colors, &stage_atlases[sub_rect.atlas_id])
                    .unwrap()
                    .with_dest_pos(x_pos, y_pos)
                    .with_source_subrect(
                        sub_rect.x,
                        sub_rect.y,
                        true_w,
                        true_h
                    ).blit();
                matched_shelf.right += true_w as i32;
                let new_sub_rect = AtlasSubRect {
                    atlas_id: 0,
                    x: x_pos as usize,
                    y: y_pos as usize,
                    w: true_w as usize,
                    h: true_h as usize,
                    padding_l: sub_rect.padding_l,
                    padding_r: sub_rect.padding_r,
                    padding_t: sub_rect.padding_t,
                    padding_b: sub_rect.padding_b,
                    is_adjusted_to_shelf: true
                };
                new_sub_rects.push(new_sub_rect);
            } else {
                if (current_shelf.right + true_w as i32) as usize >= ATLAS_SIZE {
                    shelfs.push(current_shelf);
                    current_shelf.right = 0;
                    current_shelf.y_position += current_shelf.height;
                    current_shelf.height = 0;
                }
                let y_pos = current_shelf.y_position;
                let x_pos = current_shelf.right;
                BlitBuilder::try_create(&mut new_colors, &stage_atlases[sub_rect.atlas_id])
                    .unwrap()
                    .with_dest_pos(x_pos, y_pos)
                    .with_source_subrect(
                        sub_rect.x,
                        sub_rect.y,
                        true_w,
                        true_h
                    ).blit();
                current_shelf.right += true_w as i32;
                current_shelf.height = current_shelf.height.max(true_h as i32);

                let new_sub_rect = AtlasSubRect {
                    atlas_id: 0,
                    x: x_pos as usize,
                    y: y_pos as usize,
                    w: true_w as usize,
                    h: true_h as usize,
                    padding_l: sub_rect.padding_l,
                    padding_r: sub_rect.padding_r,
                    padding_t: sub_rect.padding_t,
                    padding_b: sub_rect.padding_b,
                    is_adjusted_to_shelf: false
                };
                new_sub_rects.push(new_sub_rect);
            }
        }
        (new_colors, new_sub_rects)
    };

    let mut pic = Picture::new(ATLAS_SIZE, ATLAS_SIZE);
    pic.mutate(|buf, _, _| {
        for (cd, cs) in (&mut buf[..])
            .iter_mut()
            .zip(colors.color_data())
        {
            *cd = *cs
        }
    });
    pic.print_as_ppm();
}