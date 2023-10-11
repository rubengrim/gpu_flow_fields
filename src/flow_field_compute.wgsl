struct Uniform {
    num_spawned_lines: u32,
    max_iterations: u32,
    current_iteration: u32,
    viewport_width: f32,
    viewport_height: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniform;
@group(0) @binding(1) var<storage, read_write> vertex_buffer: array<LineVertex>;
@group(0) @binding(2) var<storage, read_write> index_buffer: array<u32>;

fn pcg_hash(input: u32) -> u32 {
    let state = input * 747796405u + 289133645u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random_f32(seed: u32) -> f32 {
    return f32(pcg_hash(seed)) / 4294967295.0;
}

struct LineVertex {
    position: vec4<f32>,
    color: vec4<f32>,
}

@compute @workgroup_size(1, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    // Create an initial line segment of 4 vertices.
    // Corresponds to two iterations.

    let line_width = 3.0;
    let step_size = 6.0;
    let flow_field_direction = vec2<f32>(1.0, 1.0);

    let seed = invocation_id.x * 2u; // Multiplying by two ensures we can get exactly two unique seeds per invocation
    let first_line_position = vec2<f32>(random_f32(seed) * uniforms.viewport_width, random_f32(seed + 1u) * uniforms.viewport_height);
    let second_line_position = vec2<f32>(first_line_position.x + flow_field_direction.x * step_size, first_line_position.y + flow_field_direction.y * step_size);

    let first_vertex_index = 4u * invocation_id.x;
    let first_triangle_index = 6u * invocation_id.x;

    let line_tangent = second_line_position - first_line_position;
    let line_normal = normalize(vec2<f32>(-line_tangent.y, line_tangent.x));
    
    // Defined in counter clockwise order
    let position_1 = first_line_position - line_normal * line_width / 2.0;
    let position_2 = first_line_position + line_normal * line_width / 2.0;
    let position_3 = second_line_position - line_normal * line_width / 2.0;
    let position_4 = second_line_position + line_normal * line_width / 2.0;

    let color = vec3<f32>(1.0, 1.0, 1.0);
    let vertex_1 = LineVertex(vec4<f32>(position_1, 0.0, 0.0), vec4<f32>(color, 0.0));
    let vertex_2 = LineVertex(vec4<f32>(position_2, 0.0, 0.0), vec4<f32>(color, 0.0));
    let vertex_3 = LineVertex(vec4<f32>(position_3, 0.0, 0.0), vec4<f32>(color, 0.0));
    let vertex_4 = LineVertex(vec4<f32>(position_4, 0.0, 0.0), vec4<f32>(color, 0.0));

    vertex_buffer[first_vertex_index] = vertex_1;
    vertex_buffer[first_vertex_index+1u] = vertex_2;
    vertex_buffer[first_vertex_index+2u] = vertex_3;
    vertex_buffer[first_vertex_index+3u] = vertex_4;
    
    index_buffer[first_triangle_index] = first_vertex_index;
    index_buffer[first_triangle_index+1u] = first_vertex_index+1u;
    index_buffer[first_triangle_index+2u] = first_vertex_index+2u;
    index_buffer[first_triangle_index+3u] = first_vertex_index+2u;
    index_buffer[first_triangle_index+4u] = first_vertex_index+3u;
    index_buffer[first_triangle_index+5u] = first_vertex_index;
}

@compute @workgroup_size(8, 1, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
}
