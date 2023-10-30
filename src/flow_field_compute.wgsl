#import bevy_render::view  View

struct Globals {
    num_spawned_lines: u32,
    max_iterations: u32,
    current_iteration: u32,
    step_size: f32,
    line_width: f32,
    viewport_width: f32,
    // Does not update when resizing window
    viewport_height: f32,
    // Space between grid points when discretizing the flow field
    grid_point_distance: f32,
    grid_margin: f32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> globals: Globals;
@group(0) @binding(2) var<storage, read_write> field_grid_buffer: array<f32>;
@group(0) @binding(3) var<storage, read_write> vertex_buffer: array<LineVertex>;
@group(0) @binding(4) var<storage, read_write> index_buffer: array<u32>;

@compute @workgroup_size(16, 16, 1)
fn discretize_flow_field(@builtin(global_invocation_id) id: vec3<u32>) {
    let grid_resolution_width =
        u32((globals.viewport_width + 2.0 * globals.grid_margin) / globals.grid_point_distance);
    let grid_resolution_height =
        u32((globals.viewport_height + 2.0 * globals.grid_margin) / globals.grid_point_distance);

    if id.x >= grid_resolution_width || id.y >= grid_resolution_height {
        return;
    }
    let origin_offset = vec2<f32>(-(globals.viewport_width * 0.5 + globals.grid_margin), -(globals.viewport_height * 0.5 + globals.grid_margin));
    let world_position = vec2<f32>(f32(id.x) * globals.grid_point_distance, f32(id.y) * globals.grid_point_distance) + origin_offset; 


    let noise_scale = 0.003;
    let field_angle = 6.2832 * perlinNoise2(world_position * noise_scale);

    // var field_angle = 1.0;
    // if world_position.x > 0.0 && world_position.y > 0.0 {
    //     field_angle = 3.1415 * 0.25;
    // }
    // if world_position.x < 0.0 && world_position.y > 0.0 {
    //     field_angle = 3.1415 * 0.75;
    // }
    // if world_position.x < 0.0 && world_position.y < 0.0 {
    //     field_angle = 3.1415 * 1.25;
    // }
    // if world_position.x > 0.0 && world_position.y < 0.0 {
    //     field_angle = 3.1415 * 1.75;
    // }

    let grid_index = id.x + grid_resolution_width * id.y;
    field_grid_buffer[grid_index] = field_angle;
}

// MIT License. © Stefan Gustavson, Munrocket
//
fn permute4(x: vec4f) -> vec4f { return ((x * 34. + 1.) * x) % vec4f(289.); }
fn fade2(t: vec2f) -> vec2f { return t * t * t * (t * (t * 6. - 15.) + 10.); }

fn perlinNoise2(P: vec2f) -> f32 {
    var Pi: vec4f = floor(P.xyxy) + vec4f(0., 0., 1., 1.);
    let Pf = fract(P.xyxy) - vec4f(0., 0., 1., 1.);
    Pi = Pi % vec4f(289.); // To avoid truncation effects in permutation
    let ix = Pi.xzxz;
    let iy = Pi.yyww;
    let fx = Pf.xzxz;
    let fy = Pf.yyww;
    let i = permute4(permute4(ix) + iy);
    var gx: vec4f = 2. * fract(i * 0.0243902439) - 1.; // 1/41 = 0.024...
    let gy = abs(gx) - 0.5;
    let tx = floor(gx + 0.5);
    gx = gx - tx;
    var g00: vec2f = vec2f(gx.x, gy.x);
    var g10: vec2f = vec2f(gx.y, gy.y);
    var g01: vec2f = vec2f(gx.z, gy.z);
    var g11: vec2f = vec2f(gx.w, gy.w);
    let norm = 1.79284291400159 - 0.85373472095314 *
        vec4f(dot(g00, g00), dot(g01, g01), dot(g10, g10), dot(g11, g11));
    g00 = g00 * norm.x;
    g01 = g01 * norm.y;
    g10 = g10 * norm.z;
    g11 = g11 * norm.w;
    let n00 = dot(g00, vec2f(fx.x, fy.x));
    let n10 = dot(g10, vec2f(fx.y, fy.y));
    let n01 = dot(g01, vec2f(fx.z, fy.z));
    let n11 = dot(g11, vec2f(fx.w, fy.w));
    let fade_xy = fade2(Pf.xy);
    let n_x = mix(vec2f(n00, n01), vec2f(n10, n11), vec2f(fade_xy.x));
    let n_xy = mix(n_x.x, n_x.y, fade_xy.y);
    return 2.3 * n_xy;
}

fn hash(value: u32) -> u32 {
    var state = value;
    state = state ^ 2747636419u;
    state = state * 2654435769u;
    state = state ^ state >> 16u;
    state = state * 2654435769u;
    state = state ^ state >> 16u;
    state = state * 2654435769u;
    return state;
}

fn random_f32(value: u32) -> f32 {
    return f32(hash(value)) / 4294967295.0;
}

fn pcg_hash(input: u32) -> u32 {
    let state = input * 747796405u + 289133645u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

struct LineVertex {
    position: vec4<f32>,
    color: vec4<f32>,
}

struct LineVertexPair {
    first: LineVertex,
    second: LineVertex,
}

fn create_debug_line_segment(first_vertex_index: u32, first_triangle_index: u32) {
    let half_length_x = 50.0;
    let half_length_y = 100.0;
    let p_1 = vec2<f32>(-half_length_x, -half_length_y);
    let p_2 = vec2<f32>(half_length_x, -half_length_y);
    let p_3 = vec2<f32>(-half_length_x, half_length_y);
    let p_4 = vec2<f32>(half_length_x, half_length_y);

    let c_1 = vec3<f32>(1.0, 0.0, 0.0);
    let c_2 = vec3<f32>(0.0, 1.0, 0.0);
    let c_3 = vec3<f32>(0.0, 0.0, 1.0);
    let c_4 = vec3<f32>(1.0, 1.0, 1.0);

    vertex_buffer[first_vertex_index] = LineVertex(vec4<f32>(p_1, 0.0, 0.0), vec4<f32>(c_1, 0.0)); 
    vertex_buffer[first_vertex_index+1u] = LineVertex(vec4<f32>(p_2, 0.0, 0.0), vec4<f32>(c_2, 0.0)); 
    vertex_buffer[first_vertex_index+2u] = LineVertex(vec4<f32>(p_3, 0.0, 0.0), vec4<f32>(c_3, 0.0)); 
    vertex_buffer[first_vertex_index+3u] = LineVertex(vec4<f32>(p_4, 0.0, 0.0), vec4<f32>(c_4, 0.0)); 

    index_buffer[first_triangle_index] = first_vertex_index;
    index_buffer[first_triangle_index+1u] = first_vertex_index+1u;
    index_buffer[first_triangle_index+2u] = first_vertex_index+3u;
    index_buffer[first_triangle_index+3u] = first_vertex_index;
    index_buffer[first_triangle_index+4u] = first_vertex_index+3u;
    index_buffer[first_triangle_index+5u] = first_vertex_index+2u;
}

fn create_vertices_for_line_joint(joint: vec2<f32>, field_direction: vec2<f32>, line_width: f32) -> LineVertexPair {
    let line_normal = normalize(vec2<f32>(field_direction.y, -field_direction.x));
    let p_1 = joint - line_normal * line_width / 2.0;
    let p_2 = joint + line_normal * line_width / 2.0;
    let c_1 = vec3<f32>(0.5, 0.5, 0.5);
    let c_2 = vec3<f32>(0.5, 0.5, 0.5);

    return LineVertexPair(
        LineVertex(vec4<f32>(p_1, 0.0, 0.0), vec4<f32>(c_1, 1.0)), 
        LineVertex(vec4<f32>(p_2, 0.0, 0.0), vec4<f32>(c_2, 1.0))
    );
}

fn get_cached_field_direction(pos: vec2<f32>) -> vec2<f32> {
    var pos_cpy = pos;
    let max_abs_width = globals.viewport_width * 0.5 + globals.grid_margin;
    let max_abs_height = globals.viewport_height * 0.5 + globals.grid_margin;

    // Clamp to area where grid is defined
    if pos_cpy.x < -max_abs_width {
        pos_cpy.x = -max_abs_width;
    }
    else if pos_cpy.x > max_abs_width {
        pos_cpy.x = max_abs_width;
    }
    if pos_cpy.y < -max_abs_height {
        pos_cpy.y = -max_abs_height;
    }
    else if pos_cpy.y > max_abs_height {
        pos_cpy.y = max_abs_height;
    }

    let grid_resolution_width =
        u32((globals.viewport_width + 2.0 * globals.grid_margin) / globals.grid_point_distance);

    let grid_space_x = u32(round((pos_cpy.x + max_abs_width) / globals.grid_point_distance));
    let grid_space_y = u32(round((pos_cpy.y + max_abs_height) / globals.grid_point_distance));
    let grid_index = grid_space_x + grid_resolution_width * grid_space_y;
    let field_angle = field_grid_buffer[grid_index];
    let field_direction = normalize(vec2<f32>(cos(field_angle), sin(field_angle)));
    
    return field_direction;
}

fn get_true_field_direction(pos: vec2<f32>) -> vec2<f32> {
    // let field_angle = 6.2832 * perlinNoise2(pos * 0.003);
    // let field_direction = normalize(vec2<f32>(cos(field_angle), sin(field_angle)));
    
    // return field_direction;
    return vec2<f32>(1.0, 0.0);
}

// Create an initial line segment of 4 vertices.
// Corresponds to two iterations.
@compute @workgroup_size(16, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    if invocation_id.x >= globals.num_spawned_lines {
        return;
    }

    // Multiplying by two ensures there's room for exactly two unique seeds per invocation
    let seed_1 = invocation_id.x * 2u; 
    let seed_2 = seed_1 + 1u;

    // view.viewport is vec4<f32>(x_orig, y_orid, width, height)
    let viewport_bottom_left = vec2<f32>(view.viewport.x - view.viewport.z / 2.0, view.viewport.y - view.viewport.w / 2.0);
    let joint_1 = vec2<f32>(viewport_bottom_left.x + random_f32(seed_1) * view.viewport.z,  viewport_bottom_left.y + random_f32(seed_2) * view.viewport.w);

    // let field_direction = get_cached_field_direction(joint_1);
    let field_direction = get_true_field_direction(joint_1);
    let joint_2 = vec2<f32>(joint_1.x + field_direction.x * globals.step_size, joint_1.y + field_direction.y * globals.step_size);

    // let joint_1 = vec2<f32>(100.0, 100.0);
    // let joint_2 = vec2<f32>(150.0, 100.0);

    let joint_1_vertices = create_vertices_for_line_joint(joint_1, field_direction, globals.line_width);
    let joint_2_vertices = create_vertices_for_line_joint(joint_2, field_direction, globals.line_width);

    let first_vertex_index = 2u * globals.max_iterations * invocation_id.x;
    let first_triangle_index = 6u * (globals.max_iterations - 1u) * invocation_id.x;

    vertex_buffer[first_vertex_index] = joint_1_vertices.first;
    vertex_buffer[first_vertex_index+1u] = joint_1_vertices.second;
    vertex_buffer[first_vertex_index+2u] = joint_2_vertices.first;
    vertex_buffer[first_vertex_index+3u] = joint_2_vertices.second;
    
    index_buffer[first_triangle_index] = first_vertex_index;
    index_buffer[first_triangle_index+1u] = first_vertex_index+1u;
    index_buffer[first_triangle_index+2u] = first_vertex_index+3u;
    index_buffer[first_triangle_index+3u] = first_vertex_index;
    index_buffer[first_triangle_index+4u] = first_vertex_index+3u;
    index_buffer[first_triangle_index+5u] = first_vertex_index+2u;
}

@compute @workgroup_size(16, 1, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    if invocation_id.x >= globals.num_spawned_lines {
        return;
    }

    let base_vertex_index = 2u * globals.current_iteration + 2u * globals.max_iterations * invocation_id.x;
    let base_triangle_index = 6u * (globals.current_iteration - 1u) + 6u * (globals.max_iterations - 1u) * invocation_id.x;

    // Vertex position for previous line joint
    let prev_joint_v1_pos = vertex_buffer[base_vertex_index-u32(2)].position.xy;
    let prev_joint_v2_pos = vertex_buffer[base_vertex_index-u32(1)].position.xy;

    let prev_joint = prev_joint_v1_pos + 0.5 * (prev_joint_v2_pos - prev_joint_v1_pos);

    // let field_direction = get_cached_field_direction(prev_joint);
    let field_direction = get_true_field_direction(prev_joint);

    let new_joint = vec2<f32>(prev_joint.x + field_direction.x * globals.step_size, prev_joint.y + field_direction.y * globals.step_size);
    let new_joint_vertices = create_vertices_for_line_joint(new_joint, field_direction, globals.line_width);


    vertex_buffer[base_vertex_index] = new_joint_vertices.first;
    vertex_buffer[base_vertex_index+1u] = new_joint_vertices.second;
    
    index_buffer[base_triangle_index] = base_vertex_index-u32(2u);
    index_buffer[base_triangle_index+1u] = base_vertex_index-u32(1u);
    index_buffer[base_triangle_index+2u] = base_vertex_index+u32(1u);
    index_buffer[base_triangle_index+3u] = base_vertex_index-u32(2u);
    index_buffer[base_triangle_index+4u] = base_vertex_index+u32(1u);
    index_buffer[base_triangle_index+5u] = base_vertex_index;

    // let d = globals.current_iteration;
    // index_buffer[base_triangle_index] = d;
    // index_buffer[base_triangle_index+1u] = d;
    // index_buffer[base_triangle_index+2u] = d;
    // index_buffer[base_triangle_index+3u] = d;
    // index_buffer[base_triangle_index+4u] = d;
    // index_buffer[base_triangle_index+5u] = d;

    // Debug
    // index_buffer[0] = u32(globals.current_iteration);
}

