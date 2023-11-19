use anyhow::Result;
use bevy::{prelude::*, render::renderer::RenderQueue};
use std::collections::VecDeque;
use wgpu::ImageCopyTexture;

use super::{
    cpu_brickmap::Brick,
    voxel_world::{CpuVoxelWorld, VoxelData},
    BRICK_OFFSET, BRICK_SIZE,
};

#[derive(Resource)]
pub struct GpuVoxelWorld {
    pub brickmap: Vec<u32>,
    pub gpu_to_cpu: Vec<u32>,
    pub brickmap_holes: VecDeque<usize>,
    pub brick_holes: VecDeque<usize>,
    pub color_texture_size: UVec3,
    pub brickmap_depth: u32,
}

#[allow(dead_code)]
impl GpuVoxelWorld {
    /// recurse the brickmap and call f on each *node* (not just leaf nodes)
    pub fn recursive_search(&self, f: &mut dyn FnMut(usize, UVec3, u32)) {
        for i in 0..8 {
            let pos = UVec3::new(i >> 2 & 1, i >> 1 & 1, i & 1) * (1 << self.brickmap_depth - 1);
            self.recursive_search_inner(i as usize, pos, 1, f);
        }
    }

    fn recursive_search_inner(
        &self,
        node_index: usize,
        pos: UVec3,
        depth: u32,
        f: &mut dyn FnMut(usize, UVec3, u32),
    ) {
        f(node_index, pos, depth);

        let children_index = self.brickmap[node_index];
        if children_index < BRICK_OFFSET {
            for i in 0..8 {
                let half_size = 1 << self.brickmap_depth - depth - 1;
                let pos = pos + UVec3::new(i >> 2 & 1, i >> 1 & 1, i & 1) * half_size;
                let index = 8 * children_index + i;
                self.recursive_search_inner(index as usize, pos, depth + 1, f);
            }
        }
    }

    // allocate a brick and copy it to the gpu
    pub fn allocate_brick(
        &mut self,
        brick: &Brick,
        voxel_data: &VoxelData,
        render_queue: &RenderQueue,
    ) -> Result<usize> {
        let brick_index = self.brick_holes.pop_front();
        if brick_index.is_none() {
            return Err(anyhow::anyhow!("ran out of space in brickmap"));
        }

        render_queue.write_buffer(
            &voxel_data.bricks,
            (brick_index.unwrap() * 4 * Brick::brick_ints()) as u64,
            &brick.get_bitmask(),
        );

        let dim = self.color_texture_size / BRICK_SIZE;
        let brick_pos = UVec3::new(
            brick_index.unwrap() as u32 / (dim.x * dim.y),
            brick_index.unwrap() as u32 / dim.x % dim.y,
            brick_index.unwrap() as u32 % dim.x,
        ) * BRICK_SIZE;
        render_queue.write_texture(
            ImageCopyTexture {
                texture: &voxel_data.color,
                origin: wgpu::Origin3d {
                    x: brick_pos.x,
                    y: brick_pos.y,
                    z: brick_pos.z,
                },
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
            },
            unsafe { brick.to_gpu() },
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(BRICK_SIZE * 4),
                rows_per_image: Some(BRICK_SIZE),
            },
            wgpu::Extent3d {
                width: BRICK_SIZE,
                height: BRICK_SIZE,
                depth_or_array_layers: BRICK_SIZE,
            },
        );

        Ok(brick_index.unwrap())
    }

    pub fn divide_node(
        &mut self,
        index: usize,
        voxel_data: &VoxelData,
        cpu_voxel_world: &CpuVoxelWorld,
        render_queue: &RenderQueue,
    ) -> Result<()> {
        let node = self.brickmap[index];
        if node < BRICK_OFFSET {
            return Err(anyhow::anyhow!("node {} already divided", index));
        }
        if node == BRICK_OFFSET {
            return Err(anyhow::anyhow!("tried to divide an empty node. this isn't a hard error, but something went wrong with either the mipmapping, streaming or initialization. should be looked into"));
        }

        let cpu_node_index = self.gpu_to_cpu[index] as usize;
        let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
        if cpu_node.children == 0 {
            return Err(anyhow::anyhow!(
                "tried to divide node with no children on cpu"
            ));
        }

        let hole = match self.brickmap_holes.pop_front() {
            Some(hole) => hole,
            None => return Err(anyhow::anyhow!("ran out of space in brickmap")),
        };

        for i in 0..8 {
            self.brickmap[hole * 8 + i] = BRICK_OFFSET;

            let cpu_child_node_index = cpu_node.children as usize * 8 + i;
            let cpu_child_node = cpu_voxel_world.brickmap[cpu_child_node_index];
            if cpu_child_node.brick != 0 {
                let brick_index = self.allocate_brick(
                    &cpu_voxel_world.bricks[cpu_child_node.brick as usize],
                    voxel_data,
                    render_queue,
                )?;
                self.brickmap[hole * 8 + i] = BRICK_OFFSET + brick_index as u32;
            }
            self.gpu_to_cpu[hole * 8 + i] = cpu_child_node_index as u32;
        }

        self.brickmap[index] = hole as u32;

        Ok(())
    }

    pub fn cull_node(
        &mut self,
        index: usize,
        voxel_data: &VoxelData,
        cpu_voxel_world: &CpuVoxelWorld,
        render_queue: &RenderQueue,
    ) -> Result<()> {
        let node = self.brickmap[index];
        if node >= BRICK_OFFSET {
            return Err(anyhow::anyhow!("node {} already culled", index));
        }

        let children_index = 8 * node as usize;
        for i in 0..8 {
            let child_node = self.brickmap[children_index + i];
            if child_node > BRICK_OFFSET {
                self.brick_holes
                    .push_back((child_node - BRICK_OFFSET) as usize);
            }
        }

        let cpu_node_index = self.gpu_to_cpu[index] as usize;
        let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];

        let brick_index = self.allocate_brick(
            &cpu_voxel_world.bricks[cpu_node.brick as usize],
            voxel_data,
            render_queue,
        )?;

        self.brickmap[index] = BRICK_OFFSET + brick_index as u32;
        self.brickmap_holes.push_back(children_index / 8);

        Ok(())
    }
}
