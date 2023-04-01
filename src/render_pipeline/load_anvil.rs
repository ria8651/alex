use crate::render_pipeline::cpu_brickmap::Brick;

use super::cpu_brickmap::CpuBrickmap;
use bevy::{prelude::*, utils::HashMap};
use fastanvil::{Block, BlockData};
use std::path::PathBuf;

fn load_palette() -> HashMap<String, [u8; 4]> {
    let file = std::fs::File::open("assets/palette/blockstates.json");
    let mut json: HashMap<String, [u8; 4]> = serde_json::from_reader(file.unwrap()).unwrap();
    json.insert("".to_string(), [200, 200, 200, 127]);
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

pub fn load_anvil(
    region_path: PathBuf,
    brickmap_depth: u32,
) -> CpuBrickmap {
    let side_length_bricks = 1 << brickmap_depth;
    let mut brickmap = CpuBrickmap::new(brickmap_depth);

    // load mc palette
    let palette = load_palette();

    let mut place_section = |block_data: &BlockData<Block>, pos: IVec3| -> Result<(), String> {
        let mut node_index = 0;
        let mut node_pos = IVec3::new(0, 0, 0);
        let mut node_depth = 1;
        loop {
            let offset = IVec3::splat(1 << (brickmap_depth - node_depth));
            let mask = pos.cmpge(node_pos + offset);
            node_pos = node_pos + IVec3::select(mask, offset, IVec3::ZERO);

            let child_index = mask.x as usize * 4 + mask.y as usize * 2 + mask.z as usize;
            let index = node_index + child_index;

            let mut new_node = 8 * (brickmap.brickmap[index] & 0xFFFF) as usize;
            if new_node == 0 {
                if node_depth == brickmap_depth {
                    // place in data
                    let brick = Brick::from_block_data(block_data, &palette);
                    let brick_index = brickmap.bricks.len() as u32;
                    brickmap.brickmap[index] = brick_index << 16;
                    brickmap.bricks.push(brick);

                    return Ok(());
                } else {
                    // subdivide node and continue
                    let new_children_index = brickmap.brickmap.len() as u32;
                    let brick_index = brickmap.bricks.len() as u32;
                    brickmap.brickmap[index] = (new_children_index / 8) | (brick_index << 16);

                    brickmap.brickmap.extend(vec![0; 8]);
                    brickmap.bricks.push(Brick::empty());

                    new_node = new_children_index as usize;
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
    let half_side_length_regions = side_length_regions / 2;
    'outer: for region_x in -half_side_length_regions..half_side_length_regions.max(1) {
        for region_z in -half_side_length_regions..half_side_length_regions.max(1) {
            let path = region_path.join(format!("r.{}.{}.mca", region_x, region_z));
            info!("loading region {}", path.display());
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
                                    32 * (region_z + half_side_length_regions) + chunk_z as i32,
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

    brickmap
}
