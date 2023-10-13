#import bevy_render::view  View

struct Settings {
    num_spawned_lines: u32,
    max_iterations: u32,
    current_iteration: u32,
    viewport_width: f32,
    viewport_height: f32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> settings: Settings;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec3<f32>,    
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = view.view_proj * vec4<f32>(in.position, 1.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // return vec4<f32>(in.color, 1.0);
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}