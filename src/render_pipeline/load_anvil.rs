use std::path::PathBuf;

use bevy::{prelude::*, utils::HashMap};
use fastanvil::{Block, BlockData};

const DEFUALT_PALETTE_COL: [u8; 4] = [255, 255, 255, 255];

pub fn load_palette() -> HashMap<String, [u8; 4]> {
    let file = std::fs::File::open("assets/palette/blockstates.json");
    let mut json: HashMap<String, [u8; 4]> = serde_json::from_reader(file.unwrap()).unwrap();
    json.insert("minecraft:grass".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:tall_grass".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:grass_block".to_string(), [62, 204, 18, 255]);
    json.insert("minecraft:water".to_string(), [20, 105, 201, 255]);
    json.insert("minecraft:cave_air".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:lava".to_string(), [255, 123, 0, 255]);
    json.insert("minecraft:seagrass".to_string(), [62, 204, 18, 255]);
    json.insert("minecraft:deepslate".to_string(), [77, 77, 77, 255]);
    json.insert("minecraft:oak_log".to_string(), [112, 62, 8, 255]);
    json.insert("minecraft:oak_stairs".to_string(), [112, 62, 8, 255]);

    json
}

fn mult_fold(v: UVec3) -> usize {
    v.x as usize * v.y as usize * v.z as usize
}

pub fn load_anvil(region_path: PathBuf, brick_map_depth: u32, brick_texture_size: UVec3) -> (Vec<u32>, Vec<u8>) {
    let side_length_bricks = 1 << brick_map_depth;

    // initialize data structures
    let mut brick_map = vec![0; 8];
    let mut bricks = vec![0; 4 * mult_fold(brick_texture_size)];
    let mut current_brick_index = 1;

    // load mc palette
    let palette = load_palette();

    let mut add_brick = |block_data: Option<&BlockData<Block>>| -> Result<u32, String> {
        if current_brick_index >= mult_fold(brick_texture_size / 16) as u32 {
            return Err("ran out of bricks".to_string());
        }

        if let Some(block_data) = block_data {
            let dim = brick_texture_size / 16;
            let brick_offset = UVec3::new(
                current_brick_index / (dim.z * dim.y),
                current_brick_index / dim.z % dim.y,
                current_brick_index % dim.z,
            ) * 16;

            for x in 0..16 {
                for y in 0..16 {
                    for z in 0..16 {
                        let block = block_data.at(x as usize, y as usize, z as usize);
                        if block.unwrap().name() == "minecraft:air" {
                            continue;
                        }

                        let block_name = block.unwrap().name();
                        let colour = palette.get(block_name).unwrap_or(&DEFUALT_PALETTE_COL);

                        let pos = UVec3::new(x, y, z) + brick_offset;
                        let index = 4
                            * (pos.z * brick_texture_size.x * brick_texture_size.y
                                + pos.y * brick_texture_size.x
                                + pos.x) as usize;
                        bricks[index] = colour[0];
                        bricks[index + 1] = colour[1];
                        bricks[index + 2] = colour[2];
                        bricks[index + 3] = colour[3];
                    }
                }
            }
        }

        current_brick_index += 1;
        Ok(current_brick_index - 1)
    };

    let mut place_section = |block_data: &BlockData<Block>, pos: IVec3| -> Result<(), String> {
        let mut node_index = 0;
        let mut node_pos = IVec3::new(0, 0, 0);
        let mut node_depth = 1;
        loop {
            let offset = IVec3::splat(1 << (brick_map_depth - node_depth));
            let mask = pos.cmpge(node_pos + offset);
            node_pos = node_pos + IVec3::select(mask, offset, IVec3::ZERO);

            let child_index = mask.x as usize * 4 + mask.y as usize * 2 + mask.z as usize;
            let index = node_index + child_index;

            let mut new_node = brick_map[index] as usize & 0xFFFF;
            if (new_node) == 0 {
                if node_depth == brick_map_depth {
                    // place in data
                    let brick_index = add_brick(Some(block_data))?;
                    brick_map[index] = brick_index << 16;
                    return Ok(());
                } else {
                    // subdivide node and continue
                    let new_children_index = brick_map.len();
                    let brick_index = add_brick(None)?;
                    brick_map[index] = new_children_index as u32 | (brick_index << 16);
                    brick_map.extend(vec![0; 8]);

                    new_node = new_children_index;
                }
            }

            node_depth += 1;
            node_index = new_node;
        }
    };

    // load chunks into the texture
    use fastanvil::{CurrentJavaChunk, Region};
    use fastnbt::from_bytes;

    let side_length_regions = (side_length_bricks / 32).max(1);
    let half_side_length_regions = (side_length_regions / 2).max(1);
    'outer: for region_x in -half_side_length_regions..half_side_length_regions {
        for region_y in -half_side_length_regions..half_side_length_regions {
            let path = region_path.join(format!("r.{}.{}.mca", region_x, region_y));
            println!("loading {}", path.display());
            if let Ok(file) = std::fs::File::open(path) {
                let mut region = Region::from_stream(file).unwrap();

                for chunk_x in 0..side_length_bricks.min(32) {
                    for chunk_z in 0..side_length_bricks.min(32) {
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
                                let pos = IVec3::new(
                                    32 * (region_x + half_side_length_regions) + chunk_x as i32,
                                    section.y as i32 + side_length_bricks / 2,
                                    32 * (region_y + half_side_length_regions) + chunk_z as i32,
                                );
                                match place_section(block_data, pos) {
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

    println!("{:?}", current_brick_index);
    println!("{:?}", brick_map.len());

    // mip-mapping
    fn recursive_mip(
        mut brick_map: &mut Vec<u32>,
        mut bricks: &mut Vec<u8>,
        node_index: usize,
        depth: u32,
        brick_map_depth: u32,
        brick_texture_size: UVec3,
    ) {
        let children_index = brick_map[node_index] as usize & 0xFFFF;
        if children_index == 0 {
            return;
        }
        if depth < brick_map_depth - 1 {
            for i in 0..8 {
                recursive_mip(
                    &mut brick_map,
                    &mut bricks,
                    children_index + i,
                    depth + 1,
                    brick_map_depth,
                    brick_texture_size,
                );
            }
        }

        // mip the brick
        let brick_index = brick_map[node_index] >> 16;
        let dim = brick_texture_size / 16;
        let brick_offset = UVec3::new(
            brick_index / (dim.z * dim.y),
            brick_index / dim.z % dim.y,
            brick_index % dim.z,
        ) * 16;
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    let pos = UVec3::new(x, y, z);

                    // get the average of the 8 children
                    let mut colour = Vec3::ZERO;
                    let mut total_alpha = 0.0;
                    let mask = pos.cmpge(UVec3::splat(8));
                    let child_node_index = children_index as usize
                        + mask.x as usize * 4
                        + mask.y as usize * 2
                        + mask.z as usize;
                    let child_brick_index = brick_map[child_node_index] >> 16;
                    let child_brick_offset = UVec3::new(
                        child_brick_index / (dim.z * dim.y),
                        child_brick_index / dim.z % dim.y,
                        child_brick_index % dim.z,
                    ) * 16;
                    for j in 0..8 {
                        let child_pos_in_brick =
                            2 * (pos % 8) + UVec3::new(j & 1, j >> 1 & 1, j >> 2 & 1);
                        let p = child_brick_offset + child_pos_in_brick;
                        let index = 4
                            * (p.z * brick_texture_size.x * brick_texture_size.y
                                + p.y * brick_texture_size.x
                                + p.x) as usize;
                        let child_colour = Vec3::new(
                            bricks[index] as f32,
                            bricks[index + 1] as f32,
                            bricks[index + 2] as f32,
                        );
                        let alpha = bricks[index + 3] as f32;
                        colour += child_colour * alpha;
                        total_alpha += alpha;
                    }
                    colour /= total_alpha;

                    // write the average to the brick
                    let pos = brick_offset + pos;
                    let index = 4
                        * (pos.z * brick_texture_size.x * brick_texture_size.y
                            + pos.y * brick_texture_size.x
                            + pos.x) as usize;

                    bricks[index] = colour.x as u8;
                    bricks[index + 1] = colour.y as u8;
                    bricks[index + 2] = colour.z as u8;
                    bricks[index + 3] = (total_alpha / 8.0) as u8;
                }
            }
        }
    }

    for i in 0..8 {
        recursive_mip(
            &mut brick_map,
            &mut bricks,
            i,
            1,
            brick_map_depth,
            brick_texture_size,
        );
    }

    (brick_map, bricks)
}
