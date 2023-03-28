use bevy::{prelude::*, utils::HashMap};
use fastanvil::{Block, BlockData};

pub fn load_palette() -> HashMap<String, [u8; 4]> {
    let file = std::fs::File::open("assets/palette/blockstates.json");
    let mut json: HashMap<String, [u8; 4]> = serde_json::from_reader(file.unwrap()).unwrap();
    json.insert("minecraft:grass".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:tall_grass".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:grass_block".to_string(), [62, 204, 18, 255]);
    json.insert("minecraft:water".to_string(), [20, 105, 201, 255]);
    json.insert("minecraft:cave_air".to_string(), [0, 0, 0, 0]);
    json.insert("minecraft:lava".to_string(), [255, 123, 0, 0]);
    json.insert("minecraft:seagrass".to_string(), [62, 204, 18, 0]);
    json.insert("minecraft:deepslate".to_string(), [77, 77, 77, 0]);
    json.insert("minecraft:oak_log".to_string(), [112, 62, 8, 0]);
    json.insert("minecraft:oak_stairs".to_string(), [112, 62, 8, 0]);

    json
}

pub fn load_anvil() -> (Vec<u32>, Vec<u8>) {
    let depth = 5;
    let side_length_bricks = 1 << depth;
    // for now i'm assuming brick size is 2^4 (16)
    // let brick_size = 4;

    // initialize data structures
    let mut brick_map = vec![0; 8];
    let mut bricks = vec![0; 256 * 256 * 256 * 4];
    let mut current_brick_index = 1;

    // load mc palette
    let palette = load_palette();

    let mut add_brick = |block_data: &BlockData<Block>| -> Result<u32, String> {
        if current_brick_index >= (256u32 / 16).pow(3) {
            return Err("ran out of bricks".to_string());
        }

        let dim = 256 / 16;
        let brick_offset = UVec3::new(
            current_brick_index / (dim * dim),
            current_brick_index / dim % dim,
            current_brick_index % dim,
        ) * 16;

        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    let block = block_data.at(x as usize, y as usize, z as usize);
                    if block.unwrap().name() == "minecraft:air" {
                        continue;
                    }

                    let block_name = block.unwrap().name();
                    let colour = palette.get(block_name).unwrap_or(&[255, 0, 0, 0]);

                    let pos = UVec3::new(x, y, z) + brick_offset;
                    let index =
                        4 * (pos.z as usize * 65536 + pos.y as usize * 256 + pos.x as usize);
                    bricks[index] = colour[0];
                    bricks[index + 1] = colour[1];
                    bricks[index + 2] = colour[2];
                    bricks[index + 3] = colour[3];
                }
            }
        }

        current_brick_index += 1;
        Ok(current_brick_index - 1)
    };

    let mut place_section = |block_data: &BlockData<Block>, pos: IVec3| -> Result<(), String> {
        let mut node_index = 0;
        let mut node_pos = IVec3::new(0, 0, 0);
        let mut node_depth = 0;
        loop {
            let p = pos.cmpge(node_pos);
            let o = IVec3::new(p.x as i32, p.y as i32, p.z as i32) * 2 - 1;
            node_depth = node_depth + 1;
            node_pos = node_pos + o * (1 << (depth - node_depth - 1));

            let child_index = p.x as usize * 4 + p.y as usize * 2 + p.z as usize;
            let index = node_index + child_index;

            let mut new_node = brick_map[index] as usize;
            if (new_node & 0xFFFF) == 0 {
                if node_depth == depth {
                    // place in data
                    let brick_index = add_brick(block_data)?;
                    brick_map[index] = brick_index << 16;
                    return Ok(());
                } else {
                    // subdivide node and continue
                    let new_children_index = brick_map.len();
                    brick_map[index] = new_children_index as u32;
                    brick_map.extend(vec![0; 8]);

                    new_node = new_children_index;
                }
            }

            node_index = new_node & 0xFFFF;
        }
    };

    // load chunks into the texture
    use fastanvil::{CurrentJavaChunk, Region};
    use fastnbt::from_bytes;

    let file = std::fs::File::open("assets/r.0.0.mca").unwrap();
    let mut region = Region::from_stream(file).unwrap();

    'outer: for chunk_x in 0..side_length_bricks {
        for chunk_z in 0..side_length_bricks {
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
                        chunk_x as i32 - side_length_bricks / 2,
                        section.y as i32,
                        chunk_z as i32 - side_length_bricks / 2,
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

    println!("{:?}", current_brick_index);
    println!("{:?}", brick_map.len());

    (brick_map, bricks)
}
