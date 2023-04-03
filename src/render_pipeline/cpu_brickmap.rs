use bevy::{prelude::*, utils::HashMap};
use fastanvil::{Block, BlockData};

pub struct Brick {
    data: [[u8; 4]; 16 * 16 * 16],
}

pub struct CpuBrickmap {
    pub brickmap: Vec<u32>,
    pub brickmap_depth: u32,
    pub bricks: Vec<Brick>,
}

#[allow(dead_code)]
impl CpuBrickmap {
    pub fn new(brickmap_depth: u32) -> Self {
        Self {
            brickmap: vec![0; 8],
            brickmap_depth,
            bricks: vec![Brick::empty()],
        }
    }

    pub fn place_brick(
        &mut self,
        block_data: &BlockData<Block>,
        pos: UVec3,
        palette: &HashMap<String, [u8; 4]>,
    ) -> Result<(), String> {
        let mut node_index = 0;
        let mut node_pos = UVec3::new(0, 0, 0);
        let mut node_depth = 1;
        loop {
            let offset = UVec3::splat(1 << (self.brickmap_depth - node_depth));
            let mask = pos.cmpge(node_pos + offset);
            node_pos = node_pos + UVec3::select(mask, offset, UVec3::ZERO);

            let child_index = mask.x as usize * 4 + mask.y as usize * 2 + mask.z as usize;
            let index = node_index + child_index;

            let mut new_node = 8 * (self.brickmap[index] & 0xFFFF) as usize;
            if new_node == 0 {
                if node_depth == self.brickmap_depth {
                    // place in data
                    let brick = Brick::from_block_data(block_data, &palette);
                    let brick_index = self.bricks.len() as u32;
                    self.brickmap[index] = brick_index << 16;
                    self.bricks.push(brick);

                    return Ok(());
                } else {
                    // subdivide node and continue
                    let new_children_index = self.brickmap.len() as u32;
                    let brick_index = self.bricks.len() as u32;
                    self.brickmap[index] = (new_children_index / 8) | (brick_index << 16);

                    self.brickmap.extend(vec![0; 8]);
                    self.bricks.push(Brick::empty());

                    new_node = new_children_index as usize;
                }
            }

            node_depth += 1;
            node_index = new_node;
        }
    }

    pub fn get_node(&self, pos: UVec3, max_depth: Option<u32>) -> (usize, UVec3, u32) {
        let mut node_index = 0;
        let mut node_pos = UVec3::new(0, 0, 0);
        let mut node_depth = 1;
        loop {
            let offset = UVec3::splat(1 << (self.brickmap_depth - node_depth));
            let mask = pos.cmpge(node_pos + offset);
            node_pos = node_pos + UVec3::select(mask, offset, UVec3::ZERO);

            let child_index = mask.x as usize * 4 + mask.y as usize * 2 + mask.z as usize;
            let index = node_index + child_index;

            let new_node = 8 * (self.brickmap[index] & 0xFFFF) as usize;
            if new_node == 0 || node_depth >= max_depth.unwrap_or(u32::MAX) {
                return (index, node_pos, node_depth);
            }

            node_depth += 1;
            node_index = new_node;
        }
    }

    // returns the brickmap and gpu bricks texture
    pub fn to_gpu(&self, brick_texture_size: UVec3) -> (Vec<u32>, Vec<u8>) {
        let brickmap = self.brickmap.clone();
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

        (brickmap, bricks)
    }

    pub fn recreate_mipmaps(&mut self) {
        info!("recreating mipmaps for {} bricks", self.bricks.len());

        // mip-mapping
        fn recursive_mip(mut brickmap: &mut CpuBrickmap, node_index: usize, depth: u32) {
            let children_index = 8 * (brickmap.brickmap[node_index] as usize & 0xFFFF);
            if children_index == 0 {
                return;
            }
            if depth < brickmap.brickmap_depth - 1 {
                for i in 0..8 {
                    recursive_mip(&mut brickmap, children_index + i, depth + 1);
                }
            }

            // mip the brick
            let brick_index = brickmap.brickmap[node_index] >> 16;

            #[cfg(debug_assertions)]
            if brick_index as usize >= brickmap.bricks.len() {
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
                        let child_brick_index = brickmap.brickmap[child_node_index] >> 16;
                        if child_brick_index as usize == 0 {
                            continue;
                        }
                        #[cfg(debug_assertions)]
                        if child_brick_index as usize >= brickmap.bricks.len() {
                            error!("child brick index out of bounds");
                        }
                        for j in 0..8 {
                            let child_pos_in_brick =
                                2 * (pos % 8) + UVec3::new(j & 1, j >> 1 & 1, j >> 2 & 1);
                            let child_colour =
                                brickmap.bricks[child_brick_index as usize].get(child_pos_in_brick);

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
                        brickmap.bricks[brick_index as usize].write(pos, new_colour);
                    }
                }
            }
        }

        for i in 0..8 {
            recursive_mip(self, i, 1);
        }
    }
}

#[allow(dead_code)]
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

    #[allow(unused_variables)]
    pub fn to_gpu(&self) -> &[u8] {
        let (head, data, tail) = unsafe { self.data.align_to::<u8>() };
        #[cfg(debug_assertions)]
        {
            assert!(head.is_empty());
            assert!(tail.is_empty());
        }
        data
    }
}
