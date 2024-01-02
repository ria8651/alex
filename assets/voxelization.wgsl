#import bevy_pbr::forward_io::VertexOutput

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

    return col;
}
