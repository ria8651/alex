use bevy::prelude::*;

pub const BRICK_SIZE: u32 = 16;

pub struct Brick {
    data: [[u8; 4]; (BRICK_SIZE * BRICK_SIZE * BRICK_SIZE) as usize],
}

#[derive(Copy, Clone)]
pub struct Node {
    pub children: u32,
    pub brick: u32,
}

impl Node {
    const ZERO: Self = Self {
        children: 0,
        brick: 0,
    };
}

pub struct CpuBrickmap {
    pub brickmap: Vec<Node>,
    pub brickmap_depth: u32,
    pub bricks: Vec<Brick>,
}

#[allow(dead_code)]
impl CpuBrickmap {
    pub fn new(brickmap_depth: u32) -> Self {
        Self {
            brickmap: vec![Node::ZERO; 8],
            brickmap_depth,
            bricks: vec![Brick::empty()],
        }
    }

    pub fn place_brick(&mut self, brick: Brick, pos: UVec3) -> Result<(), String> {
        let mut node_index = 0;
        let mut node_pos = UVec3::new(0, 0, 0);
        let mut node_depth = 1;
        loop {
            let offset = UVec3::splat(1 << (self.brickmap_depth - node_depth));
            let mask = pos.cmpge(node_pos + offset);
            node_pos = node_pos + UVec3::select(mask, offset, UVec3::ZERO);

            let child_index = mask.x as usize * 4 + mask.y as usize * 2 + mask.z as usize;
            let index = node_index + child_index;

            let mut new_node = 8 * self.brickmap[index].children as usize;
            if new_node == 0 {
                if node_depth == self.brickmap_depth {
                    // place in data
                    let brick_index = self.bricks.len() as u32;
                    self.brickmap[index].brick = brick_index;
                    self.bricks.push(brick);

                    return Ok(());
                } else {
                    // subdivide node and continue
                    let new_children_index = self.brickmap.len() as u32;
                    let brick_index = self.bricks.len() as u32;
                    self.brickmap[index] = Node {
                        children: new_children_index / 8,
                        brick: brick_index,
                    };

                    self.brickmap.extend(vec![Node::ZERO; 8]);
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

            let new_node = 8 * self.brickmap[index].children as usize;
            if new_node == 0 || node_depth >= max_depth.unwrap_or(u32::MAX) {
                return (index, node_pos, node_depth);
            }

            node_depth += 1;
            node_index = new_node;
        }
    }

    // returns the brickmap and gpu bricks texture
    pub fn to_gpu(&self, brick_texture_size: UVec3) -> (Vec<u32>, Vec<u8>) {
        let mut brickmap = vec![0; self.brickmap.len()];
        for (i, node) in self.brickmap.iter().enumerate() {
            brickmap[i] = node.children | node.brick << 16;
        }

        let texture_length = brick_texture_size.x * brick_texture_size.y * brick_texture_size.z;
        let mut bricks = vec![0; 4 * texture_length as usize];

        let dim = brick_texture_size / BRICK_SIZE;
        let max_bricks = (dim.x * dim.y * dim.z) as usize;

        for brick_index in 0..self.bricks.len() {
            if brick_index == max_bricks {
                warn!("ran out of bricks, skipping the rest");
                break;
            }

            let brick = &self.bricks[brick_index];
            let dim = brick_texture_size / BRICK_SIZE;
            let brick_pos = UVec3::new(
                brick_index as u32 / (dim.x * dim.y),
                brick_index as u32 / dim.x % dim.y,
                brick_index as u32 % dim.x,
            ) * BRICK_SIZE;
            for x in 0..BRICK_SIZE {
                for y in 0..BRICK_SIZE {
                    for z in 0..BRICK_SIZE {
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
            let children_index = 8 * brickmap.brickmap[node_index].children as usize;
            if children_index == 0 {
                return;
            }
            if depth < brickmap.brickmap_depth - 1 {
                for i in 0..8 {
                    recursive_mip(&mut brickmap, children_index + i, depth + 1);
                }
            }

            // mip the brick
            let brick_index = brickmap.brickmap[node_index].brick;

            #[cfg(debug_assertions)]
            if brick_index as usize >= brickmap.bricks.len() {
                error!("brick index out of bounds");
            }
            #[cfg(debug_assertions)]
            if brick_index as usize == 0 {
                error!("tried to mip empty brick");
            }

            for x in 0..BRICK_SIZE {
                for y in 0..BRICK_SIZE {
                    for z in 0..BRICK_SIZE {
                        let pos = UVec3::new(x, y, z);

                        // get the average of the 8 children
                        let mut colour = Vec3::ZERO;
                        let mut total_alpha = 0.0;
                        let mask = pos.cmpge(UVec3::splat(BRICK_SIZE / 2));
                        let child_node_index = children_index as usize
                            + mask.x as usize * 4
                            + mask.y as usize * 2
                            + mask.z as usize;
                        let child_brick_index = brickmap.brickmap[child_node_index].brick;
                        if child_brick_index as usize == 0 {
                            continue;
                        }
                        #[cfg(debug_assertions)]
                        if child_brick_index as usize >= brickmap.bricks.len() {
                            error!("child brick index out of bounds");
                        }
                        for j in 0..8 {
                            let child_pos_in_brick = 2 * (pos % (BRICK_SIZE / 2))
                                + UVec3::new(j & 1, j >> 1 & 1, j >> 2 & 1);
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
            data: [[0; 4]; (BRICK_SIZE * BRICK_SIZE * BRICK_SIZE) as usize],
        }
    }

    pub fn get(&self, pos: UVec3) -> [u8; 4] {
        #[cfg(debug_assertions)]
        if pos.cmplt(UVec3::ZERO).any() || pos.cmpge(UVec3::splat(BRICK_SIZE)).any() {
            error!("pos out of bounds");
        }

        let index = (pos.z * BRICK_SIZE * BRICK_SIZE + pos.y * BRICK_SIZE + pos.x) as usize;
        self.data[index]
    }

    pub fn write(&mut self, pos: UVec3, colour: [u8; 4]) {
        #[cfg(debug_assertions)]
        if pos.cmplt(UVec3::ZERO).any() || pos.cmpge(UVec3::splat(BRICK_SIZE)).any() {
            error!("pos out of bounds");
        }

        let index = (pos.z * BRICK_SIZE * BRICK_SIZE + pos.y * BRICK_SIZE + pos.x) as usize;
        self.data[index] = colour;
    }

    pub unsafe fn to_gpu(&self) -> &[u8] {
        let (_head, data, _tail) = unsafe { self.data.align_to::<u8>() };
        #[cfg(debug_assertions)]
        {
            assert!(_head.is_empty());
            assert!(_tail.is_empty());
        }
        data
    }

    pub fn brick_ints() -> usize {
        (2..=BRICK_SIZE.trailing_zeros())
            .map(|v| (1usize << v).pow(3))
            .sum::<usize>()
            / 32
    }

    fn size_offset() -> Vec<(u32, usize)> {
        (2..=BRICK_SIZE.trailing_zeros())
            .rev()
            .scan(0, |acc, x| {
                let size: usize = 1 << x;
                let output = (size as u32, *acc);
                *acc += size.pow(3);
                Some(output)
            })
            .collect()
    }

    pub fn get_bitmask(&self) -> Vec<u8> {
        let mut bitmask = vec![0; 4 * Self::brick_ints()];
        for x in 0..BRICK_SIZE {
            for y in 0..BRICK_SIZE {
                for z in 0..BRICK_SIZE {
                    let index = (z * BRICK_SIZE * BRICK_SIZE + y * BRICK_SIZE + x) as usize;
                    let colour = self.data[index];
                    if colour[3] != 0 {
                        for (size, offset) in Self::size_offset() {
                            let pos = UVec3::new(x, y, z) * size / BRICK_SIZE;
                            let sub_index = pos.x * size * size + pos.y * size + pos.z;
                            let index = offset + sub_index as usize;
                            bitmask[index / 8] |= 1 << (index % 8);
                        }
                    }
                }
            }
        }
        bitmask
    }
}
