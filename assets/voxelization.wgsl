#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::VertexOutput,
    view_transformations::{frag_coord_to_ndc, position_ndc_to_world},
}

@group(1) @binding(0) var color_texture: texture_2d<f32>;
@group(1) @binding(1) var color_sampler: sampler;
@group(1) @binding(2) var<storage, read_write> output_voxels: array<u32>;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    let col = textureSample(color_texture, color_sampler, mesh.uv);

    if col.a < 0.5 {
        discard;
    }

    if all(view.world_position == vec3(0.5, 0.5, 0.0))
    || all(view.world_position == vec3(0.0, 0.5, 0.5))
    || all(view.world_position == vec3(0.5, 0.0, 0.5)) { // terrible way of checking for the voxelization cameras
        let ndc = frag_coord_to_ndc(mesh.position);
        let world_pos = position_ndc_to_world(ndc);

        let voxel_pos = vec3<u32>(world_pos * 16.0);
        if voxel_pos.x < 0u || voxel_pos.x >= 16u || voxel_pos.y < 0u || voxel_pos.y >= 16u || voxel_pos.z < 0u || voxel_pos.z >= 16u {
            discard;
        }
        let voxel_index = voxel_pos.x + voxel_pos.y * 16u + voxel_pos.z * 16u * 16u;
        let red = u32(col.r * 255.0);
        let green = u32(col.g * 255.0) << 8u;
        let blue = u32(col.b * 255.0) << 16u;
        let alpha = u32(col.a * 255.0) << 24u;
        output_voxels[voxel_index] = red | green | blue | alpha;
    }

    return vec4(col.rgb, 1.0);
}
