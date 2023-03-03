#import bevy_core_pipeline::fullscreen_vertex_shader

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
var bricks: texture_storage_3d<rgba8unorm, read_write>;
@group(0) @binding(2)
var<storage, read_write> brickmap: array<u32>;

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
    let s = step(vec3<f32>(-1.0), v) - step(vec3<f32>(1.0), v);
    return (s.x * s.y * s.z) > 0.5;
}

struct HitInfo {
    hit: bool,
    data: vec4<f32>,
    pos: vec3<f32>,
    normal: vec3<f32>,
    steps: u32,
};

struct Voxel {
    data: vec4<f32>,
    pos: vec3<f32>,
    grid_size: u32,
};

fn get_value(pos: vec3<f32>) -> Voxel {
    let pos = vec3<i32>(16.0 * (pos * 0.5 + 0.5));
    let data = textureLoad(bricks, pos);
    let return_pos = (vec3<f32>(pos) + 0.5) / 16.0 * 2.0 - 1.0;

    return Voxel(data, return_pos, 16u);
}

fn shoot_ray(r: Ray) -> HitInfo {
    var pos = r.pos;
    var dir = r.dir;

    if (!in_bounds(pos)) {
        // Get position on surface of the octree
        let dist = ray_box_dist(Ray(pos, dir), vec3(-1.0), vec3(1.0)).x;
        if (dist == 0.0) {
            return HitInfo(false, vec4(0.0), vec3(0.0), vec3(0.0), 0u);
        }

        pos = pos + dir * dist;
    }

    var r_sign = sign(dir);
    var tcpotr = pos; // the current position of the ray
    var steps = 0u;
    var normal = trunc(pos * 1.00001);
    var voxel = Voxel(vec4(0.0), vec3(0.0), 0u);
    while (steps < 1000u) {
        voxel = get_value(tcpotr);
        if (any(voxel.data.rgb != vec3(0.0))) {
            break;
        }

        let voxel_size = 2.0 / f32(voxel.grid_size);
        let t_max = (voxel.pos - pos + r_sign * voxel_size / 2.0) / dir;

        // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
        let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
        normal = mask * -r_sign;

        let t_current = min(min(t_max.x, t_max.y), t_max.z);
        tcpotr = pos + dir * t_current - normal * 0.000002;

        if (!in_bounds(tcpotr)) {
            return HitInfo(false, vec4(0.0), vec3(0.0), vec3(0.0), steps);
        }

        steps = steps + 1u;
    }

    return HitInfo(true, voxel.data, tcpotr, normal, steps);
}

let light_dir = vec3<f32>(0.8, -1.0, 0.8);
let light_colour = vec3<f32>(1.0, 1.0, 1.0);

fn calculate_direct(material: vec4<f32>, pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    var lighting = vec3(0.0);
    if (material.a == 0.0) {
        // diffuse
        let diffuse = max(dot(normal, -normalize(light_dir)), 0.0);

        // shadow
        var shadow = 1.0;
        if (uniforms.shadows != 0u) {
            let shadow_ray = Ray(pos + normal * 0.0002, -light_dir);
            let shadow_hit = shoot_ray(shadow_ray);
            shadow = f32(!shadow_hit.hit);
        }

        lighting = diffuse * shadow * light_colour;
    } else {
        lighting = vec3(material.rgb);
    }
    return lighting;
}

fn check_voxel(pos: vec3<f32>) -> f32 {
    let voxel = textureLoad(bricks, vec3<i32>(pos));
    return f32(any(voxel.rgb != vec3(0.0)));
}
// https://www.shadertoy.com/view/ldl3DS
fn vertex_ao(side: vec2<f32>, corner: f32) -> f32 {
    return (side.x + side.y + max(corner, side.x * side.y)) / 3.1;
}
fn voxel_ao(pos: vec3<f32>, d1: vec3<f32>, d2: vec3<f32>) -> vec4<f32> {
    let side = vec4(check_voxel(pos + d1), check_voxel(pos + d2), check_voxel(pos - d1), check_voxel(pos - d2));
    let corner = vec4(check_voxel(pos + d1 + d2), check_voxel(pos - d1 + d2), check_voxel(pos - d1 - d2), check_voxel(pos + d1 - d2));

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

    let pos = uniforms.camera_inverse * vec4(clip_space.x, clip_space.y, 1.0, 1.0);
    let dir = uniforms.camera_inverse * vec4(clip_space.x, clip_space.y, 0.01, 1.0);
    let pos = pos.xyz / pos.w;
    let dir = normalize(dir.xyz / dir.w - pos);
    var ray = Ray(pos, dir);

    // let p = vec2<i32>(in.uv * 16.0);
    // let pos = vec3(p, i32(uniforms.misc_float * 16.0));
    // output_colour = textureLoad(bricks, vec3<i32>(pos)).xyz;

    // output_colour = vec3(ray_box_dist(Ray(pos, dir), vec3(-1.0), vec3(1.0)).x);

    let hit = shoot_ray(ray);
    if (hit.hit) {
        // direct lighting
        let direct_lighting = calculate_direct(hit.data, hit.pos, hit.normal);

        // aproximate indirect with ambient and voxel ao
        let texture_coords = (hit.pos * 0.5 + 0.5) * 16.0;
        let ao = voxel_ao(texture_coords, hit.normal.zxy, hit.normal.yzx);
        let uv = glmod(vec2(dot(hit.normal * texture_coords.yzx, vec3(1.0)), dot(hit.normal * texture_coords.zxy, vec3(1.0))), vec2(1.0));

        let interpolated_ao = mix(mix(ao.z, ao.w, uv.x), mix(ao.y, ao.x, uv.x), uv.y);
        let interpolated_ao = pow(interpolated_ao, 1.0 / 3.0);

        let indirect_lighting = vec3(interpolated_ao * 0.3);

        // final blend
        output_colour = (indirect_lighting + direct_lighting) * hit.data.rgb;
    } else {
        output_colour = vec3(0.4);
    }

    output_colour = max(output_colour, vec3(0.0));
    return vec4<f32>(output_colour, 1.0);
}