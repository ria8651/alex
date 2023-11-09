use super::cpu_brickmap::{Brick, CpuBrickmap, BRICK_SIZE};
use bevy::{prelude::*, utils::HashMap};
use std::path::PathBuf;

fn load_palette() -> HashMap<String, [u8; 4]> {
    let file = std::fs::File::open("assets/palette/blockstates.json");
    let mut json: HashMap<String, [u8; 4]> = serde_json::from_reader(file.unwrap()).unwrap();
    json.insert("".to_string(), [200, 200, 200, 127]);
    json.insert("minecraft:grass".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:tall_grass".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:grass_block".to_string(), [62, 204, 18, 255]);
    json.insert("minecraft:water".to_string(), [20, 105, 201, 30]);
    json.insert("minecraft:cave_air".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:lava".to_string(), [255, 123, 0, 255]);
    json.insert("minecraft:seagrass".to_string(), [62, 204, 18, 255]);
    json.insert("minecraft:deepslate".to_string(), [77, 77, 77, 255]);
    json.insert("minecraft:oak_log".to_string(), [112, 62, 8, 255]);
    json.insert("minecraft:oak_stairs".to_string(), [112, 62, 8, 255]);

    json
}

pub fn load_anvil(region_path: PathBuf, world_depth: u32) -> CpuBrickmap {
    let side_length = 1 << world_depth;
    let mut brickmap = CpuBrickmap::new(world_depth - BRICK_SIZE.trailing_zeros());

    // load mc palette
    let palette = load_palette();

    // load chunks into the texture
    use fastanvil::{CurrentJavaChunk, Region};
    use fastnbt::from_bytes;

    let side_length_chunks = side_length / 16;
    let side_length_regions = (side_length_chunks / 32).max(1);
    let half_side_length_regions: i32 = side_length_regions / 2;
    'outer: for region_x in -half_side_length_regions..half_side_length_regions.max(1) {
        for region_z in -half_side_length_regions..half_side_length_regions.max(1) {
            let path = region_path.join(format!("r.{}.{}.mca", region_x, region_z));
            if let Ok(file) = std::fs::File::open(path.clone()) {
                info!("loading region {}", path.display());
                let mut region = Region::from_stream(file).unwrap();

                for chunk_x in 0..side_length_chunks.min(32) {
                    for chunk_z in 0..side_length_chunks.min(32) {
                        if let Some(data) = region
                            .read_chunk(chunk_x as usize, chunk_z as usize)
                            .unwrap()
                        {
                            let chunk: CurrentJavaChunk = from_bytes(data.as_slice()).unwrap();
                            let section_tower = chunk.sections.unwrap();

                            for section in section_tower.sections() {
                                if section.block_states.palette().len() <= 1 {
                                    continue;
                                }

                                let block_data = &section.block_states;
                                let pos = UVec3::new(
                                    32 * (region_x + half_side_length_regions) as u32
                                        + chunk_x as u32,
                                    (section.y as i32 + side_length_chunks / 2) as u32,
                                    32 * (region_z + half_side_length_regions) as u32
                                        + chunk_z as u32,
                                );

                                let chunk_side_length_bricks = 16 / BRICK_SIZE;
                                for brick_x in 0..chunk_side_length_bricks {
                                    for brick_y in 0..chunk_side_length_bricks {
                                        for brick_z in 0..chunk_side_length_bricks {
                                            let mut brick = Brick::empty();
                                            for x in 0..BRICK_SIZE {
                                                for y in 0..BRICK_SIZE {
                                                    for z in 0..BRICK_SIZE {
                                                        let block = block_data.at(
                                                            (brick_x * BRICK_SIZE + x) as usize,
                                                            (brick_y * BRICK_SIZE + y) as usize,
                                                            (brick_z * BRICK_SIZE + z) as usize,
                                                        );
                                                        if block.unwrap().name() == "minecraft:air"
                                                        {
                                                            continue;
                                                        }

                                                        let block_name = block.unwrap().name();
                                                        let defualt_col = palette.get("").unwrap();
                                                        let colour = palette
                                                            .get(block_name)
                                                            .unwrap_or(defualt_col);

                                                        let pos = UVec3::new(x, y, z);
                                                        brick.write(pos, *colour);
                                                    }
                                                }
                                            }

                                            match brickmap.place_brick(
                                                brick,
                                                chunk_side_length_bricks * pos
                                                    + UVec3::new(brick_x, brick_y, brick_z),
                                            ) {
                                                Ok(_) => {}
                                                Err(e) => {
                                                    println!("{}", e);
                                                    break 'outer;
                                                }
                                            };
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                info!("skipping region {}", path.display());
            }
        }
    }

    // let file = std::fs::File::open("assets/region/r.0.0.mca").unwrap();
    // let mut region = Region::from_stream(file).unwrap();
    // let data = region.read_chunk(0, 0).unwrap().unwrap();
    // let chunk: CurrentJavaChunk = from_bytes(data.as_slice()).unwrap();
    // let section_tower = chunk.sections.unwrap();
    // place_section(
    //     &section_tower.get_section_for_y(64).unwrap().block_states,
    //     IVec3::new(2, 2, 2),
    // )
    // .unwrap();

    brickmap
}

// pub fn from_block_data(block_data: &BlockData<Block>, palette: &HashMap<String, [u8; 4]>) -> Brick {
//     let mut brick = Brick::empty();
//     for x in 0..BRICK_SIZE {
//         for y in 0..BRICK_SIZE {
//             for z in 0..BRICK_SIZE {
//                 let block = block_data.at(x as usize, y as usize, z as usize);
//                 if block.unwrap().name() == "minecraft:air" {
//                     continue;
//                 }

//                 let block_name = block.unwrap().name();
//                 let defualt_col = palette.get("").unwrap();
//                 let colour = palette.get(block_name).unwrap_or(&defualt_col);

//                 let pos = UVec3::new(x, y, z);
//                 brick.write(pos, *colour);
//             }
//         }
//     }
//     brick
// }
