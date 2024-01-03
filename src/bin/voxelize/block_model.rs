use bevy::{prelude::*, render::mesh::Indices};
use minecraft_assets::schemas::models::BlockFace;
use wgpu::PrimitiveTopology;

pub struct BlockModel {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    uvs: Vec<Vec2>,
    indices: Vec<u32>,
}

impl BlockModel {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn to_mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
            .with_indices(Some(Indices::U32(self.indices)))
    }

    pub fn push_face(
        &mut self,
        c1: Vec3,
        c2: Vec3,
        face: BlockFace,
        uv1: Vec2,
        uv2: Vec2,
        rot: Quat,
        rot_pos: Vec3,
    ) {
        let b = self.positions.len() as u32;
        self.indices
            .extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 3, b]);
        match face {
            BlockFace::North => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c2.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, 0.0, 1.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                ]);
            }
            BlockFace::South => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c1.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, 0.0, -1.0),
                    Vec3::new(0.0, 0.0, -1.0),
                    Vec3::new(0.0, 0.0, -1.0),
                    Vec3::new(0.0, 0.0, -1.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv2.x, uv2.y),
                ]);
            }
            BlockFace::East => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c2.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv1.x, uv2.y),
                ]);
            }
            BlockFace::West => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c1.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(-1.0, 0.0, 0.0),
                    Vec3::new(-1.0, 0.0, 0.0),
                    Vec3::new(-1.0, 0.0, 0.0),
                    Vec3::new(-1.0, 0.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv1.x, uv2.y),
                ]);
            }
            BlockFace::Up => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c2.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv2.x, uv1.y),
                ]);
            }
            BlockFace::Down => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c1.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, -1.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                ]);
            }
        }
    }
}
