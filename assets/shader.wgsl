#import bevy_core_pipeline::fullscreen_vertex_shader

const BRICK_SIZE: u32 = 4u; // brick size as a power of 2

struct MainPassUniforms {
    camera: mat4x4<f32>,
    camera_inverse: mat4x4<f32>,
    time: f32,
    show_ray_steps: u32,
    indirect_lighting: u32,
    shadows: u32,
    misc_bool: u32,
    misc_float: f32,
};

@group(0) @binding(1)
var<storage, read_write> brickmap: array<u32>;
@group(0) @binding(2)
var bricks: texture_storage_3d<rgba8unorm, read_write>;

@group(1) @binding(0)
var<uniform> uniforms: MainPassUniforms;

struct Ray {
    pos: vec3<f32>,
    dir: vec3<f32>,
};

// returns the closest intersection and the furthest intersection
fn ray_box_dist(r: Ray, vmin: vec3<f32>, vmax: vec3<f32>) -> vec2<f32> {
    let v1 = (vmin.x - r.pos.x) / r.dir.x;
    let v2 = (vmax.x - r.pos.x) / r.dir.x;
    let v3 = (vmin.y - r.pos.y) / r.dir.y;
    let v4 = (vmax.y - r.pos.y) / r.dir.y;
    let v5 = (vmin.z - r.pos.z) / r.dir.z;
    let v6 = (vmax.z - r.pos.z) / r.dir.z;
    let v7 = max(max(min(v1, v2), min(v3, v4)), min(v5, v6));
    let v8 = min(min(max(v1, v2), max(v3, v4)), max(v5, v6));
    if (v8 < 0.0 || v7 > v8) {
        return vec2(0.0);
    }

    return vec2(v7, v8);
}

fn in_bounds(v: vec3<f32>) -> bool {
    return max(max(abs(v.x), abs(v.y)), abs(v.z)) < 1.0;
}

struct Brick {
    index: u32,
    pos: vec3<f32>,
    depth: u32,
}

fn find_brick(pos: vec3<f32>) -> Brick {
    var node_index = 0u;
    var node_pos = vec3<f32>(0.0);
    var depth = 0u;
    loop {
        let p = vec3<u32>(
            u32(pos.x >= node_pos.x),
            u32(pos.y >= node_pos.y),
            u32(pos.z >= node_pos.z)
        );

        depth = depth + 1u;
        node_pos = node_pos + (vec3<f32>(p) * 2.0 - 1.0) / f32(1u << depth);

        let child_index = p.x * 4u + p.y * 2u + p.z;
        let new_node_index = node_index + child_index;
        let new_node = brickmap[new_node_index];
        if ((new_node & 0xFFFFu) == 0u) {
            return Brick(new_node >> 16u, node_pos, depth);
        } else {
            node_index = new_node & 0xFFFFu;
        }
    }

    // unreachable (hopefully)
    return Brick(0u, vec3(1.0, 0.0, 0.0), 0u);

    // let dim = f32(textureDimensions(bricks).x) / f32(1u << BRICK_SIZE);
    // let depth = u32(log2(dim));
    // let brick_pos = vec3<u32>((pos * 0.5 + 0.5) * f32(1u << depth));
    // let rounded_pos = (vec3<f32>(brick_pos) + 0.5) / f32(1u << depth) * 2.0 - 1.0;
    // let index = brick_pos.x * (1u << (2u * depth)) + brick_pos.y * (1u << depth) + brick_pos.z;

    // return Brick(index, rounded_pos, depth);
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
    if (denom < 0.00001) {
        let t = dot(normal, pos - r.pos) / denom;
        if (t >= 0.0) {
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

fn shoot_ray(r: Ray) -> HitInfo {
    var pos = r.pos;
    var dir = r.dir;

    if (!in_bounds(pos)) {
        // Get position on surface of the octree
        let dist = ray_box_dist(Ray(pos, dir), vec3(-1.0), vec3(1.0)).x;
        if (dist == 0.0) {
            return HitInfo(false, Voxel(vec4(0.0), vec3(0.0), 0.0), vec3(0.0), vec3(0.0), 0u);
        }

        pos = pos + dir * dist;
    }

    // step through the octree using multilevel dda
    var r_sign = sign(dir);
    var normal = trunc(pos * 1.00001);
    var tcpotr = pos * 0.999999; // the current position of the ray
    var steps = 1u;
    var brick = Brick(0u, vec3(0.0), 0u);
    while (steps < 500u) {
        brick = find_brick(tcpotr);

        if (brick.index != 0u) {
            // step through the brick using dda
            let dim = textureDimensions(bricks).x / i32(1u << BRICK_SIZE);
            let brick_pos_in_texture = vec3(
                i32(brick.index) / (dim * dim),
                (i32(brick.index) / dim) % dim,
                i32(brick.index) % dim,
            ) * i32(1u << BRICK_SIZE);
            let pos_in_brick_float = (tcpotr - brick.pos) * f32(1u << brick.depth) * 0.5 + 0.5;
            var pos_in_brick = vec3<i32>(pos_in_brick_float * f32(1u << BRICK_SIZE));
            let pos_in_brick_float_rounded = vec3<f32>(pos_in_brick);
            var t_max_inner = (pos_in_brick_float_rounded - pos_in_brick_float * f32(1u << BRICK_SIZE) + 0.5 + r_sign * 0.5) / dir;
            while (steps < 500u) {
                let col = textureLoad(bricks, brick_pos_in_texture + pos_in_brick);
                if (any(col.rgb != vec3(0.0))) {
                    // undo preperation for next step
                    t_max_inner += normal / dir;

                    // get world space pos of the hit
                    let t_current_inner = min(min(t_max_inner.x, t_max_inner.y), t_max_inner.z);
                    let pos_in_brick_world = pos_in_brick_float * f32(1u << BRICK_SIZE) + dir * t_current_inner - normal * 0.000002;
                    tcpotr = brick.pos + (pos_in_brick_world / f32(1u << BRICK_SIZE) * 2.0 - 1.0) / f32(1u << brick.depth);

                    let half_size = 1.0 / f32(1u << (brick.depth + BRICK_SIZE));
                    return HitInfo(true, Voxel(col, tcpotr, half_size), tcpotr + normal * 0.000003, normal, steps);
                }

                // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
                let mask = vec3<f32>(t_max_inner.xyz <= min(t_max_inner.yzx, t_max_inner.zxy));
                normal = mask * -r_sign;

                t_max_inner += -normal / dir;
                pos_in_brick += vec3<i32>(-normal);

                steps += 1u;

                if (any(pos_in_brick < vec3(0)) || any(pos_in_brick >= vec3(i32(1u << BRICK_SIZE)))) {
                    break;
                }
            }
        }

        let brick_half_size = 1.0 / f32(1u << brick.depth);
        let t_max = (brick.pos - pos + r_sign * brick_half_size) / dir;

        // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
        let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
        normal = mask * -r_sign;

        let t_current = min(min(t_max.x, t_max.y), t_max.z);
        tcpotr = pos + dir * t_current - normal * 0.000002;

        if (!in_bounds(tcpotr)) {
            return HitInfo(false, Voxel(vec4(0.0), vec3(0.0), 0.0), vec3(0.0), vec3(0.0), steps);
        }

        steps += 1u;
    }

    return HitInfo(false, Voxel(vec4(0.0), vec3(0.0), 0.0), vec3(0.0), vec3(0.0), steps);
}

const light_dir = vec3<f32>(0.8, -1.0, 0.8);
const light_colour = vec3<f32>(1.0, 1.0, 1.0);

fn calculate_direct(material: vec4<f32>, pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    // diffuse
    let diffuse = max(dot(normal, -normalize(light_dir)), 0.0);

    // shadow
    var shadow = 1.0;
    if (uniforms.shadows != 0u) {
        let shadow_ray = Ray(pos, -light_dir);
        let shadow_hit = shoot_ray(shadow_ray);
        shadow = f32(!shadow_hit.hit);
    }

    return diffuse * shadow * light_colour;
}

fn check_voxel(pos: vec3<f32>) -> f32 {
    let brick = find_brick(pos);
    let reletive_pos = (pos - brick.pos) * f32(1u << brick.depth);
    let dim = textureDimensions(bricks).x / i32(1u << BRICK_SIZE);
    let texture_pos = vec3(
        i32(brick.index) / (dim * dim),
        i32(brick.index) / dim % dim,
        i32(brick.index) % dim,
    ) * i32(1u << BRICK_SIZE);

    let texture_offset = vec3<i32>(f32(1u << BRICK_SIZE) * (reletive_pos * 0.5 + 0.5));
    let data = textureLoad(bricks, texture_pos + texture_offset);
    return f32(any(data.rgb != vec3(0.0)));
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
    let clip_space = vec2(1.0, -1.0) * (in.uv * 2.0 - 1.0);
    var output_colour = vec3(0.0);

    let pos4 = uniforms.camera_inverse * vec4(clip_space.x, clip_space.y, 1.0, 1.0);
    let dir4 = uniforms.camera_inverse * vec4(clip_space.x, clip_space.y, 0.01, 1.0);
    let pos = pos4.xyz / pos4.w;
    let dir = normalize(dir4.xyz / dir4.w - pos);
    var ray = Ray(pos, dir);

    let hit = shoot_ray(ray);
    if (hit.hit) {
        // direct lighting
        let direct_lighting = calculate_direct(hit.voxel.col, hit.pos, hit.normal);

        // aproximate indirect with ambient and voxel ao
        var indirect_lighting = vec3(0.3);
        // if (uniforms.indirect_lighting != 0u) {
            let coords = hit.pos + hit.normal * hit.voxel.half_size;
            let scaled_offset = 2.0 * hit.normal * hit.voxel.half_size;
            let ao = voxel_ao(coords, scaled_offset.zxy, scaled_offset.yzx);
            let uv = glmod(vec2(
                dot(hit.normal * hit.pos.yzx, vec3(1.0)), 
                dot(hit.normal * hit.pos.zxy, vec3(1.0))
            ), vec2(2.0 * hit.voxel.half_size)) / (2.0 * hit.voxel.half_size);

            var interpolated_ao = mix(mix(ao.z, ao.w, uv.x), mix(ao.y, ao.x, uv.x), uv.y);
            interpolated_ao = pow(interpolated_ao, 1.0 / 3.0);

            indirect_lighting = vec3(interpolated_ao * 0.3);
        // }

        // final blend
        output_colour = (direct_lighting + indirect_lighting) * hit.voxel.col.rgb;
        // output_colour = hit.pos;
        // output_colour = hit.voxel.col.rgb;
    } else {
        output_colour = vec3(0.4);
    }

    if (uniforms.show_ray_steps != 0u) {
        output_colour = vec3(f32(hit.steps) / 100.0);
    }

    output_colour = max(output_colour, vec3(0.0));
    return vec4<f32>(output_colour, 1.0);
}
