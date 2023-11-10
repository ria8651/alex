#import bevy_pbr::{
    mesh_view_bindings::view,
    mesh_functions::{
        get_model_matrix,
        mesh_position_local_to_clip,
    },
    view_transformations::direction_clip_to_world,
    utils::coords_to_viewport_uv,
}

struct Vertex {
    // vertex data
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    // instance data
    @location(3) pos_scale: vec4<f32>,
    @location(4) brick: u32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_pos: vec3<f32>,
    @location(1) local_cam: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) brick: u32,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let position = vertex.position * vertex.pos_scale.w + vertex.pos_scale.xyz;
    let clip_pos = mesh_position_local_to_clip(
        get_model_matrix(0u),
        vec4<f32>(position, 1.0)
    );
    let camera_local = (view.world_position - vertex.pos_scale.xyz) / vertex.pos_scale.w;

    var out: VertexOutput;
    // NOTE: Passing 0 as the instance_index to get_model_matrix() is a hack
    // for this example as the instance_index builtin would map to the wrong
    // index in the Mesh array. This index could be passed in via another
    // uniform instead but it's unnecessary for the example.
    out.clip_pos = clip_pos;
    out.local_pos = vertex.position;
    out.local_cam = camera_local;
    out.normal = vertex.normal;
    out.brick = vertex.brick;

    return out;
}

struct VoxelUniforms {
    brick_map_depth: u32,
    brick_size: u32, // brick size as a power of 2
    brick_ints: u32,
}

@group(2) @binding(0)
var<uniform> voxel_uniforms: VoxelUniforms;
@group(2) @binding(1)
var<storage, read> brickmap: array<u32>;
@group(2) @binding(2)
var<storage, read_write> counters: array<u32>;
@group(2) @binding(3)
var<storage, read> bricks: array<u32>;
@group(2) @binding(4)
var color_texture: texture_storage_3d<rgba8unorm, read>;

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

fn trace_brick(index: u32, local_pos: vec3<f32>, dir: vec3<f32>, normal: ptr<function, vec3<f32>>) -> vec3<f32> {
    let r_sign = sign(dir);
    var pos_in_brick = local_pos * 0.5 + 0.5 - *normal * 0.000001;
    var initial_pos_in_brick = pos_in_brick;
    var steps = 0u;
    while steps < 500u {
        var size = 0;

        let pos_in_0 = vec3<i32>(pos_in_brick * 16.0);
        let pos_in_1 = vec3<i32>(pos_in_brick * 8.0);
        let pos_in_2 = vec3<i32>(pos_in_brick * 4.0);
        let pos_in_3 = vec3<i32>(pos_in_brick * 2.0);

        let index_0 = 0u + u32(pos_in_0.z * 16 * 16 + pos_in_0.y * 16 + pos_in_0.x);
        let index_1 = 4096u + u32(pos_in_1.z * 8 * 8 + pos_in_1.y * 8 + pos_in_1.x);
        let index_2 = 4608u + u32(pos_in_2.z * 4 * 4 + pos_in_2.y * 4 + pos_in_2.x);
        let index_3 = 4672u + u32(pos_in_3.z * 2 * 2 + pos_in_3.y * 2 + pos_in_3.x);

        let bit_0 = (bricks[index * voxel_uniforms.brick_ints + index_0 / 32u] >> (index_0 % 32u)) & 1u;
        let bit_1 = (bricks[index * voxel_uniforms.brick_ints + index_1 / 32u] >> (index_1 % 32u)) & 1u;
        let bit_2 = (bricks[index * voxel_uniforms.brick_ints + index_2 / 32u] >> (index_2 % 32u)) & 1u;
        let bit_3 = (bricks[index * voxel_uniforms.brick_ints + index_3 / 32u] >> (index_3 % 32u)) & 1u;

        if bit_0 == 0u {
            size = 16;
        }
        if bit_1 == 0u {
            size = 8;
        }
        if bit_2 == 0u {
            size = 4;
        }
        if bit_3 == 0u {
            size = 2;
        }

        if bit_0 != 0u {
            // get color of the voxel
            let brick_size = i32(1u << voxel_uniforms.brick_size);
            let dim = vec3<i32>(textureDimensions(color_texture)) / brick_size;
            let brick_pos_in_texture = vec3(
                i32(index) / (dim.z * dim.y),
                (i32(index) / dim.z) % dim.y,
                i32(index) % dim.z,
            ) * brick_size;
            let color = textureLoad(color_texture, brick_pos_in_texture + vec3<i32>(pos_in_brick * f32(brick_size))).rgb;
            return pos_in_brick;
        }

        let rounded_pos = floor(pos_in_brick * f32(size)) / f32(size);
        let t_max = (rounded_pos - initial_pos_in_brick + 0.5 * (r_sign + 1.0) / f32(size)) / dir;

        // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
        let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
        *normal = mask * -r_sign;

        let t_current = min(min(t_max.x, t_max.y), t_max.z);
        pos_in_brick = initial_pos_in_brick + dir * t_current - *normal * 0.000002;

        steps += 1u;

        if any(pos_in_brick < vec3(0.0)) || any(pos_in_brick > vec3(1.0)) {
            break;
        }
    }

    // discard;
    return vec3(f32(steps) / 10.0);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var output_color = vec3(0.0);

    let viewport_uv = coords_to_viewport_uv(in.clip_pos.xy, view.viewport);
    let clip_uv = (viewport_uv * 2.0 - 1.0) * vec2(1.0, -1.0);

    var pos: vec3<f32>;
    if all(in.local_cam < vec3(1.0)) && all(in.local_cam > vec3(-1.0)) {
        pos = in.local_cam;
    } else {
        pos = in.local_pos;
    }
    let dir = normalize(direction_clip_to_world(vec4(clip_uv, 0.0, 1.0)));

    var normal = in.normal;
    output_color = trace_brick(in.brick, pos, dir, &normal);

    var out: FragmentOutput;
    out.color = vec4<f32>(output_color, 1.0);
    out.depth = in.clip_pos.z;
    return out;
}
