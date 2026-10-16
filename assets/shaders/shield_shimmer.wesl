import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::{globals, view},
};

struct ShieldMaterial {
    color: vec4<f32>,
    hit: f32,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> material: ShieldMaterial;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let facing = clamp(dot(normal, view_dir), 0.0, 1.0);
    let rim = pow(1.0 - facing, 2.0);
    let sweep = 0.5 + 0.5 * sin(in.world_position.y * 6.0 + globals.time * 2.6);
    let pulse = 0.72 + 0.28 * sin(globals.time * 1.7);
    let shimmer = rim * (0.55 + 0.45 * sweep) * pulse * 0.42;
    let hit_alpha = material.hit * (0.22 + rim * 0.55);
    let color = mix(material.color.rgb, vec3<f32>(0.86, 0.97, 1.0), material.hit);
    return vec4<f32>(color, min(shimmer + hit_alpha, 0.9));
}
