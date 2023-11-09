#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

const BRICK_OFFSET: u32 = 2147483648u;
const COUNTER_BITS: u32 = 32u;

struct VoxelUniforms {
    brick_map_depth: u32,
    brick_size: u32, // brick size as a power of 2
    brick_ints: u32,
}

struct MainPassUniforms {
    camera: mat4x4<f32>,
    camera_inverse: mat4x4<f32>,
    time: f32,
    show_ray_steps: u32,
    indirect_lighting: u32,
    shadows: u32,
    super_pixel_size: u32,
    misc_bool: u32,
    misc_float: f32,
};

@group(0) @binding(0)
var<uniform> voxel_uniforms: VoxelUniforms;
@group(0) @binding(1)
var<storage, read> brickmap: array<u32>;
@group(0) @binding(2)
var<storage, read_write> counters: array<u32>;
@group(0) @binding(3)
var<storage, read> bricks: array<u32>;
@group(0) @binding(4)
var color_texture: texture_storage_3d<rgba8unorm, read>;

@group(1) @binding(0)
var<uniform> uniforms: MainPassUniforms;
@group(1) @binding(1)
var beam_texture: texture_storage_2d<rgba16float, read>;

struct Ray {
    pos: vec3<f32>,
    dir: vec3<f32>,
};

struct RayBoxDist {
    min: f32,
    max: f32,
    normal: vec3<f32>,
}

// returns the closest intersection and the furthest intersection
fn ray_box_dist(r: Ray, vmin: vec3<f32>, vmax: vec3<f32>) -> RayBoxDist {
    let v1 = (vmin.x - r.pos.x) / r.dir.x;
    let v2 = (vmax.x - r.pos.x) / r.dir.x;
    let v3 = (vmin.y - r.pos.y) / r.dir.y;
    let v4 = (vmax.y - r.pos.y) / r.dir.y;
    let v5 = (vmin.z - r.pos.z) / r.dir.z;
    let v6 = (vmax.z - r.pos.z) / r.dir.z;
    let v7 = max(max(min(v1, v2), min(v3, v4)), min(v5, v6));
    let v8 = min(min(max(v1, v2), max(v3, v4)), max(v5, v6));
    if v8 < 0.0 || v7 > v8 {
        return RayBoxDist(0.0, 0.0, vec3(0.0));
    }

    let t_max = vec3<f32>(min(v1, v2), min(v3, v4), min(v5, v6));
    let mask = vec3<f32>(t_max.xyz >= max(t_max.yzx, t_max.zxy));
    let normal = mask * -sign(r.dir);
    return RayBoxDist(v7, v8, normal);
}

struct Brick {
    index: u32,
    pos: vec3<i32>,
    depth: u32,
    node_index: u32,
}

fn find_brick(pos: vec3<i32>, count: bool) -> Brick {
    var node_index = 0u;
    var node_pos = vec3(0);
    var depth = 1u;
    loop {
        let offset = vec3(1 << (voxel_uniforms.brick_map_depth - depth));
        let mask = vec3<i32>(pos >= node_pos + offset);
        node_pos += mask * offset;

        let child_index = mask.x * 4 + mask.y * 2 + mask.z;
        let new_node_index = node_index + u32(child_index);
        let new_node = brickmap[new_node_index];

        if count {
            counters[new_node_index] += 1u;
        }

        if new_node >= BRICK_OFFSET {
            return Brick(new_node - BRICK_OFFSET, node_pos, depth, new_node_index);
        }

        depth = depth + 1u;
        node_index = 8u * new_node;
    }

    return Brick(0u, vec3(0), 0u, 0u);
}

// maps a point form the -1 to 1 cube to a point in the cube l to u
fn unit_to(l: vec3<f32>, u: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    return l + (u - l) * (p + 1.0) * 0.5;
}

// maps a point from the cube l to u to a point in the -1 to 1 cube
fn to_unit(l: vec3<f32>, u: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    return (p - l) / (u - l) * 2.0 - 1.0;
}

fn ray_plane(r: Ray, pos: vec3<f32>, normal: vec3<f32>) -> vec4<f32> {
    let denom = dot(normal, r.dir);
    if denom < 0.00001 {
        let t = dot(normal, pos - r.pos) / denom;
        if t >= 0.0 {
            let pos = r.pos + r.dir * t;
            return vec4(pos, t);
        }
    }
    return vec4(0.0);
}

struct Voxel {
    col: vec4<f32>,
    pos: vec3<f32>,
    half_size: f32,
};

struct HitInfo {
    hit: bool,
    voxel: Voxel,
    pos: vec3<f32>,
    normal: vec3<f32>,
    steps: u32,
};

fn in_bounds(v: vec3<f32>) -> bool {
    return !(any(v < vec3(0.0)) || any(v >= vec3(f32(1u << voxel_uniforms.brick_map_depth))));
}

fn shoot_ray(r: Ray, maximum_ratio: f32) -> HitInfo {
    var pos = r.pos + f32(1u << voxel_uniforms.brick_map_depth) / 2.0;
    var dir = r.dir;
    var normal = vec3<f32>(0.0);
    let ray_origin = pos;

    if !in_bounds(pos) {
        let ray_box = ray_box_dist(Ray(pos, dir), vec3(0.0), vec3(f32(1u << voxel_uniforms.brick_map_depth)));
        if ray_box.min == 0.0 {
            return HitInfo(false, Voxel(vec4(0.0), vec3(0.0), 0.0), vec3(0.0), vec3(0.0), 0u);
        }

        pos = pos + dir * ray_box.min;
        normal = ray_box.normal;
    }

    // step through the octree using multilevel dda
    var r_sign = sign(dir);
    var tcpotr = pos - vec3(0.00004); // the current position of the ray
    var steps = 1u;
    var brick = Brick(0u, vec3(0), 0u, 0u);
    while steps < 500u {
        brick = find_brick(vec3<i32>(tcpotr), true);

        if brick.index > 0u {
            // step through the brick using dda
            let brick_size = i32(1u << voxel_uniforms.brick_size);
            let annoying_factor = f32(1u << (voxel_uniforms.brick_map_depth - brick.depth));
            let initial_pos_in_brick = (tcpotr - vec3<f32>(brick.pos)) / annoying_factor;
            var pos_in_brick = initial_pos_in_brick;

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

                let bit_0 = (bricks[brick.index * voxel_uniforms.brick_ints + index_0 / 32u] >> (index_0 % 32u)) & 1u;
                let bit_1 = (bricks[brick.index * voxel_uniforms.brick_ints + index_1 / 32u] >> (index_1 % 32u)) & 1u;
                let bit_2 = (bricks[brick.index * voxel_uniforms.brick_ints + index_2 / 32u] >> (index_2 % 32u)) & 1u;
                let bit_3 = (bricks[brick.index * voxel_uniforms.brick_ints + index_3 / 32u] >> (index_3 % 32u)) & 1u;

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

                // beam optimization
                let ray_length = length(ray_origin - tcpotr + (initial_pos_in_brick - pos_in_brick) * annoying_factor);
                let ray_size = ray_length * maximum_ratio;
                let voxel_size = annoying_factor / f32(size);

                if bit_0 != 0u || voxel_size < ray_size {
                    // get world space pos of the hit
                    let world_pos = vec3<f32>(brick.pos) + (pos_in_brick + normal * 0.0001) * annoying_factor - f32(1u << voxel_uniforms.brick_map_depth) / 2.0;

                    // get voxel info
                    let half_size = annoying_factor / f32(brick_size);
                    let voxel_pos = (floor(pos_in_brick * f32(brick_size)) + 0.5) / f32(brick_size) * annoying_factor + vec3<f32>(brick.pos) - f32(1u << voxel_uniforms.brick_map_depth) / 2.0;

                    // get color of the voxel
                    var col = vec4(1.0);
                    if maximum_ratio == 0.0 {
                        let dim = vec3<i32>(textureDimensions(color_texture)) / brick_size;
                        let brick_pos_in_texture = vec3(
                            i32(brick.index) / (dim.z * dim.y),
                            (i32(brick.index) / dim.z) % dim.y,
                            i32(brick.index) % dim.z,
                        ) * brick_size;
                        col = textureLoad(color_texture, brick_pos_in_texture + vec3<i32>(pos_in_brick * f32(brick_size)));
                    }

                    // let counter_value = f32(counters[brick.node_index]) / 100.0;
                    return HitInfo(true, Voxel(vec4(col), voxel_pos, half_size), world_pos, normal, steps);
                }

                let rounded_pos = floor(pos_in_brick * f32(size)) / f32(size);
                let t_max = (rounded_pos - initial_pos_in_brick + 0.5 * (r_sign + 1.0) / f32(size)) / dir;

                // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
                let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
                normal = mask * -r_sign;

                let t_current = min(min(t_max.x, t_max.y), t_max.z);
                pos_in_brick = initial_pos_in_brick + dir * t_current - normal * 0.000002;

                steps += 1u;

                if any(pos_in_brick < vec3(0.0)) || any(pos_in_brick >= vec3(1.0)) {
                    break;
                }
            }
        }

        let brick_half_size = f32(1 << voxel_uniforms.brick_map_depth) / f32(1u << (brick.depth + 1u));
        let t_max = (vec3<f32>(brick.pos) + brick_half_size - pos + r_sign * brick_half_size) / dir;

        // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
        let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
        normal = mask * -r_sign;

        let t_current = min(min(t_max.x, t_max.y), t_max.z);
        tcpotr = pos + dir * t_current - normal * 0.00004;

        if !in_bounds(tcpotr) {
            return HitInfo(false, Voxel(vec4(0.0), vec3(0.0), 0.0), vec3(0.0), vec3(0.0), steps);
        }

        steps += 1u;
    }

    return HitInfo(false, Voxel(vec4(0.0), vec3(0.0), 0.0), vec3(0.0), vec3(0.0), steps);
}

const light_dir = vec3<f32>(0.8, -1.0, 0.8);
const light_colour = vec3<f32>(1.0, 1.0, 0.8);

fn calculate_direct(material: vec4<f32>, pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    // diffuse
    let diffuse = max(dot(normal, -normalize(light_dir)), 0.0);

    // shadow
    var shadow = 1.0;
    if uniforms.shadows != 0u {
        let shadow_ray = Ray(pos, -light_dir);
        let shadow_hit = shoot_ray(shadow_ray, 0.0);
        shadow = f32(!shadow_hit.hit);
    }

    return diffuse * shadow * light_colour;
}

fn check_voxel(p: vec3<f32>) -> f32 {
    let pos = p + f32(1u << voxel_uniforms.brick_map_depth) / 2.0;
    if !in_bounds(pos) {
        return 0.0;
    }

    let brick = find_brick(vec3<i32>(pos), false);
    if brick.index == 0u {
        return 0.0;
    }

    let brick_size = i32(1u << voxel_uniforms.brick_size);
    let annoying_factor = f32(1u << (voxel_uniforms.brick_map_depth - brick.depth));
    let pos_in_brick = vec3<i32>((pos - vec3<f32>(brick.pos)) / annoying_factor * f32(brick_size));
    let index = u32(pos_in_brick.z * brick_size * brick_size + pos_in_brick.y * brick_size + pos_in_brick.x);
    let bit = (bricks[brick.index * voxel_uniforms.brick_ints + index / 32u] >> (index % 32u)) & 1u;

    return f32(bit);
}
// https://www.shadertoy.com/view/ldl3DS
fn vertex_ao(side: vec2<f32>, corner: f32) -> f32 {
    return (side.x + side.y + max(corner, side.x * side.y)) / 3.1;
}
fn voxel_ao(pos: vec3<f32>, d1: vec3<f32>, d2: vec3<f32>) -> vec4<f32> {
    let side = vec4(check_voxel(pos + d1), check_voxel(pos + d2), check_voxel(pos - d1), check_voxel(pos - d2));
    let corner = vec4(
        check_voxel(pos + d1 + d2),
        check_voxel(pos - d1 + d2),
        check_voxel(pos - d1 - d2),
        check_voxel(pos + d1 - d2)
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

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let clip_space = vec2(1.0, -1.0) * vec2<f32>(in.uv * 2.0 - 1.0);
    var output_colour = vec3(0.0);

    let pos4 = uniforms.camera_inverse * vec4(clip_space.x, clip_space.y, 1.0, 1.0);
    let dir4 = uniforms.camera_inverse * vec4(clip_space.x, clip_space.y, 0.01, 1.0);
    let pos = pos4.xyz / pos4.w;
    let dir = normalize(dir4.xyz / dir4.w - pos);
    var ray = Ray(pos, dir);

    // beam optimization
    let beam_texture_size = textureDimensions(beam_texture);
    if all(beam_texture_size > vec2(1u)) {
        let beam_texture_pos = in.uv * vec2<f32>(beam_texture_size) - 0.5;
        let dist1 = textureLoad(beam_texture, vec2<i32>(beam_texture_pos) + vec2(0, 0)).r;
        let dist2 = textureLoad(beam_texture, vec2<i32>(beam_texture_pos) + vec2(0, 1)).r;
        let dist3 = textureLoad(beam_texture, vec2<i32>(beam_texture_pos) + vec2(1, 1)).r;
        let dist4 = textureLoad(beam_texture, vec2<i32>(beam_texture_pos) + vec2(1, 0)).r;
        let dist = min(min(dist1, dist2), min(dist3, dist4));
        let offset = dist * 1.5 / f32(beam_texture_size.x);
        ray.pos += ray.dir * max((dist - offset), 0.0);

        let hit = shoot_ray(ray, 0.0);
        if hit.hit {
            // direct lighting
            let direct_lighting = calculate_direct(hit.voxel.col, hit.pos, hit.normal);

            // aproximate indirect with ambient and voxel ao
            var indirect_lighting = vec3(0.3);
            if uniforms.indirect_lighting != 0u {
                let offset = hit.normal * hit.voxel.half_size;
                let ao = voxel_ao(hit.voxel.pos + offset, offset.zxy, offset.yzx);
                let uv = glmod(
                    vec2(
                        dot(hit.normal * hit.pos.yzx, vec3(1.0)),
                        dot(hit.normal * hit.pos.zxy, vec3(1.0))
                    ),
                    vec2(hit.voxel.half_size)
                ) / (hit.voxel.half_size);

                var interpolated_ao = mix(mix(ao.z, ao.w, uv.x), mix(ao.y, ao.x, uv.x), uv.y);
                interpolated_ao = pow(interpolated_ao, 1.0 / 3.0);

                indirect_lighting = vec3(interpolated_ao * 0.3);
            }

            // final blend
            output_colour = (direct_lighting + indirect_lighting) * hit.voxel.col.rgb;
        } else {
            output_colour = vec3(0.2);
        }

        // let posawoeigh = in.uv * vec2<f32>(beam_texture_size);
        // output_colour = textureLoad(beam_texture, vec2<i32>(posawoeigh)).rgb;

        if uniforms.show_ray_steps != 0u {
            output_colour = vec3(f32(hit.steps) / 100.0);
            // let v = min(f32(hit.steps) / 200.0, 0.3);
            // output_colour = 0.6 - 0.6 * cos(6.3 * 2.0 * v + vec3(0.0, 23.0, 21.0));
        }
    } else {
        let maximum_ratio = 0.02 / f32(beam_texture_size.x);
        let hit = shoot_ray(ray, maximum_ratio);
        if hit.hit {
            output_colour = vec3(length(hit.pos - pos));
            // output_colour = hit.voxel.col.rgb;
        } else {
            output_colour = vec3(10000000.0);
            // output_colour = vec3(0.2);
        }
    }

    output_colour = max(output_colour, vec3(0.0));
    return vec4<f32>(output_colour, 1.0);
}
