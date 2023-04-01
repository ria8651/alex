use bevy::{prelude::*, utils::HashMap};
use fastanvil::{Block, BlockData};

pub struct Brick {
    data: [[u8; 4]; 16 * 16 * 16],
}

pub struct CpuBrickmap {
    pub brick_map: Vec<u32>,
    pub brick_map_depth: u32,
    pub bricks: Vec<Brick>,
}

impl CpuBrickmap {
    pub fn new(brick_map_depth: u32) -> Self {
        Self {
            brick_map: vec![0; 8],
            brick_map_depth,
            bricks: vec![Brick::empty()],
        }
    }

    // returns the brick_map and gpu bricks texture
    pub fn to_gpu(&self, brick_texture_size: UVec3) -> (Vec<u32>, Vec<u8>) {
        let brick_map = self.brick_map.clone();
        let texture_length = brick_texture_size.x * brick_texture_size.y * brick_texture_size.z;
        let mut bricks = vec![0; 4 * texture_length as usize];

        let dim = brick_texture_size / 16;
        let max_bricks = (dim.x * dim.y * dim.z) as usize;

        for brick_index in 0..self.bricks.len() {
            if brick_index == max_bricks {
                warn!("ran out of bricks, skipping the rest");
                break;
            }

            let brick = &self.bricks[brick_index];
            let dim = brick_texture_size / 16;
            let brick_pos = UVec3::new(
                brick_index as u32 / (dim.x * dim.y),
                brick_index as u32 / dim.x % dim.y,
                brick_index as u32 % dim.x,
            ) * 16;
            for x in 0..16 {
                for y in 0..16 {
                    for z in 0..16 {
                        let pos = UVec3::new(x, y, z);
                        let p = brick_pos + pos;
                        let index = p.z * brick_texture_size.x * brick_texture_size.y
                            + p.y * brick_texture_size.x
                            + p.x;
                        let index = index as usize * 4;
                        let colour = brick.get(pos);
                        bricks[index..index + 4].copy_from_slice(&colour);
                    }
                }
            }
        }

        (brick_map, bricks)
    }

    pub fn recreate_mipmaps(&mut self) {
        info!("recreating mipmaps for {} bricks", self.bricks.len());

        // mip-mapping
        fn recursive_mip(mut brick_map: &mut CpuBrickmap, node_index: usize, depth: u32) {
            let children_index = brick_map.brick_map[node_index] as usize & 0xFFFF;
            if children_index == 0 {
                return;
            }
            if depth < brick_map.brick_map_depth - 1 {
                for i in 0..8 {
                    recursive_mip(&mut brick_map, children_index + i, depth + 1);
                }
            }

            // mip the brick
            let brick_index = brick_map.brick_map[node_index] >> 16;

            #[cfg(debug_assertions)]
            if brick_index as usize >= brick_map.bricks.len() {
                error!("brick index out of bounds");
            }
            #[cfg(debug_assertions)]
            if brick_index as usize == 0 {
                error!("tried to mip empty brick");
            }

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
                        let child_brick_index = brick_map.brick_map[child_node_index] >> 16;
                        if child_brick_index as usize == 0 {
                            continue;
                        }
                        #[cfg(debug_assertions)]
                        if child_brick_index as usize >= brick_map.bricks.len() {
                            error!("child brick index out of bounds");
                        }
                        for j in 0..8 {
                            let child_pos_in_brick =
                                2 * (pos % 8) + UVec3::new(j & 1, j >> 1 & 1, j >> 2 & 1);
                            let child_colour = brick_map.bricks[child_brick_index as usize]
                                .get(child_pos_in_brick);

                            let alpha = child_colour[3] as f32;
                            let child_colour = Vec3::new(
                                child_colour[0] as f32,
                                child_colour[1] as f32,
                                child_colour[2] as f32,
                            );

                            colour += child_colour * alpha;
                            total_alpha += alpha;
                        }
                        colour /= total_alpha;
                        total_alpha /= 8.0;

                        // write the average to the brick
                        let new_colour = [
                            colour.x as u8,
                            colour.y as u8,
                            colour.z as u8,
                            total_alpha as u8,
                        ];
                        brick_map.bricks[brick_index as usize].write(pos, new_colour);
                    }
                }
            }
        }

        for i in 0..8 {
            recursive_mip(self, i, 1);
        }
    }
}

impl Brick {
    pub fn empty() -> Self {
        Self {
            data: [[0; 4]; 16 * 16 * 16],
        }
    }

    pub fn get(&self, pos: UVec3) -> [u8; 4] {
        #[cfg(debug_assertions)]
        if pos.cmplt(UVec3::ZERO).any() || pos.cmpge(UVec3::splat(16)).any() {
            error!("pos out of bounds");
        }

        let index = (pos.z * 16 * 16 + pos.y * 16 + pos.x) as usize;
        self.data[index]
    }

    pub fn write(&mut self, pos: UVec3, colour: [u8; 4]) {
        #[cfg(debug_assertions)]
        if pos.cmplt(UVec3::ZERO).any() || pos.cmpge(UVec3::splat(16)).any() {
            error!("pos out of bounds");
        }

        let index = (pos.z * 16 * 16 + pos.y * 16 + pos.x) as usize;
        self.data[index] = colour;
    }

    pub fn from_block_data(
        block_data: &BlockData<Block>,
        palette: &HashMap<String, [u8; 4]>,
    ) -> Self {
        let mut brick = Brick::empty();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    let block = block_data.at(x as usize, y as usize, z as usize);
                    if block.unwrap().name() == "minecraft:air" {
                        continue;
                    }

                    let block_name = block.unwrap().name();
                    let defualt_col = palette.get("").unwrap();
                    let colour = palette.get(block_name).unwrap_or(&defualt_col);

                    let pos = UVec3::new(x, y, z);
                    brick.write(pos, *colour);
                }
            }
        }
        brick
    }
}
