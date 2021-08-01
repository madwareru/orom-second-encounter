use crate::constants::*;
use simple_tiled_wfc::grid_generation::WfcModule;
use crate::CustomBitSet;
use bitsetium::{BitEmpty, BitSet};

pub struct AvailableTiles {
    pub land: bool,
    pub grass: bool,
    pub plateau: bool,
    pub sand: bool,
    pub savannah: bool,
    pub rocks: bool,
    pub high_rocks: bool,
    pub water: bool,
    pub road: bool,
}

impl Default for AvailableTiles {
    fn default() -> Self {
        Self {
            land: true,
            grass: true,
            plateau: true,
            sand: true,
            savannah: true,
            rocks: true,
            high_rocks: true,
            water: true,
            road: true,
        }
    }
}

impl AvailableTiles {
    pub fn make_bitset(&self, tiles: &[TileInfo]) -> CustomBitSet {
        let mut bitset = CustomBitSet::empty();
        for i in 0..tiles.len() {
            let tile = &tiles[i];
            if self.land && tile.any_corner_matches(LAND) {
                bitset.set(i);
                continue;
            }
            if self.grass && tile.any_corner_matches(GRASS) {
                bitset.set(i);
                continue;
            }
            if self.plateau && tile.any_corner_matches(PLATEAU) {
                bitset.set(i);
                continue;
            }
            if self.sand && tile.any_corner_matches(SAND) {
                bitset.set(i);
                continue;
            }
            if self.savannah && tile.any_corner_matches(SAVANNAH) {
                bitset.set(i);
                continue;
            }
            if self.rocks && tile.any_corner_matches(ROCKS) {
                bitset.set(i);
                continue;
            }
            if self.high_rocks && tile.any_corner_matches(HIGH_ROCKS) {
                bitset.set(i);
                continue;
            }
            if self.water && tile.any_corner_matches(WATER) {
                bitset.set(i);
                continue;
            }
            if self.road && tile.any_corner_matches(ROAD) {
                bitset.set(i);
                continue;
            }
        }
        bitset
    }
}

pub struct TileInfo {
    pub north_west: u8,
    pub north_east: u8,
    pub south_west: u8,
    pub south_east: u8,
    pub tile_x: usize,
    pub tile_y: usize
}
impl TileInfo {
    fn any_corner_matches(&self, terrain_type: u8) -> bool {
        self.north_west == terrain_type ||
            self.north_east == terrain_type ||
            self.south_west == terrain_type ||
            self.south_east == terrain_type
    }
}

pub fn make_tiling_lookup() -> Vec<TileInfo> {
    let tile_definitions = &[
        (LAND, GRASS, 0, 0),
        (LAND, PLATEAU, 4, 0),
        (LAND, SAND, 8, 0),
        (LAND, SAVANNAH, 12, 0),
        (LAND, ROCKS, 0, 6),
        (ROCKS, PLATEAU, 4, 6),
        (SAVANNAH, GRASS, 8, 6),
        (ROCKS, HIGH_ROCKS, 12, 6),
        (LAND, WATER, 0, 12),
        (LAND, ROAD, 4, 12)
    ];

    let mut tiles = Vec::new();
    for &(outer_type, inner_type, start_tile_x, start_tile_y) in tile_definitions {
        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: outer_type,
            south_west: outer_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 0) * 32,
            tile_y: (start_tile_y + 0) * 32
        });
        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: outer_type,
            south_west: inner_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 1) * 32,
            tile_y: (start_tile_y + 0) * 32
        });
        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: outer_type,
            south_west: inner_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 2) * 32,
            tile_y: (start_tile_y + 0) * 32
        });

        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: inner_type,
            south_west: outer_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 0) * 32,
            tile_y: (start_tile_y + 1) * 32
        });
        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: inner_type,
            south_west: inner_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 1) * 32,
            tile_y: (start_tile_y + 1) * 32
        });
        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: outer_type,
            south_west: inner_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 2) * 32,
            tile_y: (start_tile_y + 1) * 32
        });

        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: inner_type,
            south_west: outer_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 0) * 32,
            tile_y: (start_tile_y + 2) * 32
        });
        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: inner_type,
            south_west: outer_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 1) * 32,
            tile_y: (start_tile_y + 2) * 32
        });
        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: outer_type,
            south_west: outer_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 2) * 32,
            tile_y: (start_tile_y + 2) * 32
        });

        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: inner_type,
            south_west: inner_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 0) * 32,
            tile_y: (start_tile_y + 3) * 32
        });
        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: inner_type,
            south_west: outer_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 1) * 32,
            tile_y: (start_tile_y + 3) * 32
        });
        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: inner_type,
            south_west: outer_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 2) * 32,
            tile_y: (start_tile_y + 3) * 32
        });

        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: outer_type,
            south_west: inner_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 0) * 32,
            tile_y: (start_tile_y + 4) * 32
        });
        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: outer_type,
            south_west: outer_type,
            south_east: outer_type,
            tile_x: (start_tile_x + 1) * 32,
            tile_y: (start_tile_y + 4) * 32
        });
        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: inner_type,
            south_west: outer_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 2) * 32,
            tile_y: (start_tile_y + 4) * 32
        });

        tiles.push(TileInfo {
            north_west: inner_type,
            north_east: outer_type,
            south_west: inner_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 0) * 32,
            tile_y: (start_tile_y + 5) * 32
        });
        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: outer_type,
            south_west: inner_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 1) * 32,
            tile_y: (start_tile_y + 5) * 32
        });
        tiles.push(TileInfo {
            north_west: outer_type,
            north_east: inner_type,
            south_west: inner_type,
            south_east: inner_type,
            tile_x: (start_tile_x + 2) * 32,
            tile_y: (start_tile_y + 5) * 32
        });

        for j in 0..6 {
            tiles.push(TileInfo {
                north_west: inner_type,
                north_east: inner_type,
                south_west: inner_type,
                south_east: inner_type,
                tile_x: (start_tile_x + 3) * 32,
                tile_y: (start_tile_y + j) * 32
            });
        }
    }
    tiles
}

pub fn make_module_set(tiles: &[TileInfo]) -> Vec<WfcModule<[u8; 30]>> {
    let modules = {
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
        modules
    };
    modules
}