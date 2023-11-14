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
    @location(1) pos_scale: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) brick: u32,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    // the 0.9999 avoids z-fighting with the backface
    let position = 0.9999 * vertex.position * vertex.pos_scale.w + vertex.pos_scale.xyz;

    // NOTE: Passing 0 as the instance_index to get_model_matrix() is a hack
    // for this example as the instance_index builtin would map to the wrong
    // index in the Mesh array. This index could be passed in via another
    // uniform instead but it's unnecessary for the example.
    let clip_pos = mesh_position_local_to_clip(get_model_matrix(0u), vec4<f32>(position, 1.0));

    var out: VertexOutput;
    out.clip_pos = clip_pos;
    out.local_pos = vertex.position;
    out.pos_scale = vertex.pos_scale;
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

// local_pos ranges from (0,0,0) to (1,1,1) inside the brick
fn trace_brick(index: u32, local_pos: ptr<function, vec3<f32>>, dir: vec3<f32>, normal: ptr<function, vec3<f32>>) -> vec3<f32> {
    let r_sign = sign(dir);
    var initial_pos = *local_pos;
    var steps = 0u;
    while steps < 50u {
        var size = 0;

        let lookup_pos = *local_pos - *normal * 0.000001;
        if any(lookup_pos < vec3(0.0)) || any(lookup_pos > vec3(1.0)) {
            if steps == 0u {
                return lookup_pos;
            }
            break;
        }

        let pos_in_0 = vec3<i32>(lookup_pos * 16.0);
        let pos_in_1 = vec3<i32>(lookup_pos * 8.0);
        let pos_in_2 = vec3<i32>(lookup_pos * 4.0);
        let pos_in_3 = vec3<i32>(lookup_pos * 2.0);

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
            let color = textureLoad(color_texture, brick_pos_in_texture + vec3<i32>(lookup_pos * f32(brick_size))).rgb;
            return color;
            // return vec3(f32(steps) / 2.0);
        }

        let rounded_pos = floor(lookup_pos * f32(size)) / f32(size);
        let t_max = (rounded_pos - initial_pos + 0.5 * (r_sign + 1.0) / f32(size)) / dir;

        // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
        let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
        *normal = mask * -r_sign;

        let t_current = min(min(t_max.x, t_max.y), t_max.z);
        *local_pos = initial_pos + dir * t_current;

        steps += 1u;
    }

    discard;
    // return vec3(f32(steps) / 10.0);
}

// https://www.shadertoy.com/view/ldl3DS
fn check_voxel(brick: u32, pos: vec3<i32>) -> f32 {
    let brick_size = i32(1u << voxel_uniforms.brick_size);
    if any(pos < vec3(0)) || any(pos >= vec3(brick_size)) {
        return 0.0;
    }
    
    let index = u32(pos.z * brick_size * brick_size + pos.y * brick_size + pos.x);
    let bit = (bricks[brick * voxel_uniforms.brick_ints + index / 32u] >> (index % 32u)) & 1u;
    return f32(bit);
}
fn vertex_ao(side: vec2<f32>, corner: f32) -> f32 {
    return (side.x + side.y + max(corner, side.x * side.y)) / 3.1;
}
fn voxel_ao(pos: vec3<i32>, normal: vec3<i32>, brick: u32) -> vec4<f32> {
    let d1 = normal.zxy;
    let d2 = normal.yzx;
    let side = vec4(check_voxel(brick, pos + d1), check_voxel(brick, pos + d2), check_voxel(brick, pos - d1), check_voxel(brick, pos - d2));
    let corner = vec4(
        check_voxel(brick, pos + d1 + d2),
        check_voxel(brick, pos - d1 + d2),
        check_voxel(brick, pos - d1 - d2),
        check_voxel(brick, pos + d1 - d2)
    );

    var ao: vec4<f32>;
    ao.x = vertex_ao(side.xy, corner.x);
    ao.y = vertex_ao(side.yz, corner.y);
    ao.z = vertex_ao(side.zw, corner.z);
    ao.w = vertex_ao(side.wx, corner.w);

    return 1.0 - ao;
}
fn glmod(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    return x - y * floor(x / y);
}

const light_dir = vec3<f32>(0.8, -1.0, 0.8);

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    // @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) facing: bool) -> FragmentOutput {
    var output_color = vec3(0.0);

    // get ray direction
    let viewport_uv = coords_to_viewport_uv(in.clip_pos.xy, view.viewport);
    let clip_uv = (viewport_uv * 2.0 - 1.0) * vec2(1.0, -1.0);
    let dir = normalize(direction_clip_to_world(vec4(clip_uv, 0.0, 1.0)));

    // get local position
    var pos: vec3<f32>;
    let local_cam = (view.world_position - in.pos_scale.xyz) / in.pos_scale.w;
    if all(local_cam < vec3(1.0)) && all(local_cam > vec3(0.0)) {
        pos = local_cam;
    } else {
        if !facing {
            discard;
        }
        pos = in.local_pos;
    }

    // shoot ray
    var normal = in.normal;
    let color = trace_brick(in.brick, &pos, dir, &normal);

    // diffuse
    let diffuse = max(dot(normal, -normalize(light_dir)), 0.0);

    // indirect lighting
    let bick_size = f32(1u << voxel_uniforms.brick_size);
    let ao_pos = vec3<i32>(pos * bick_size + normal * 0.5);
    let ao = voxel_ao(ao_pos, vec3<i32>(normal), in.brick);
    let uv = glmod(
        vec2(
            dot(normal * pos.yzx, vec3(1.0)),
            dot(normal * pos.zxy, vec3(1.0))
        ),
        vec2(1.0 / bick_size)
    ) * bick_size;
    let interpolated_ao = mix(mix(ao.z, ao.w, uv.x), mix(ao.y, ao.x, uv.x), uv.y);
    let indirect = pow(interpolated_ao, 1.0 / 3.0) * 0.3;

    output_color = color * (diffuse + indirect);
    // output_color = in.local_pos;
    
    var out: FragmentOutput;
    out.color = vec4<f32>(output_color, 1.0);
    // out.depth = 0.0;
    return out;
}
