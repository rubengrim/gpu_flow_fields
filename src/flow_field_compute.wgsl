#import bevy_render::view  View

struct Globals {
    num_spawned_lines: u32,
    max_iterations: u32,
    current_iteration: u32,
    step_size: f32,
    line_width: f32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> globals: Globals;
@group(0) @binding(2) var<storage, read_write> vertex_buffer: array<LineVertex>;
@group(0) @binding(3) var<storage, read_write> index_buffer: array<u32>;

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

@compute @workgroup_size(16, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    // Create an initial line segment of 4 vertices.
    // Corresponds to two iterations.

    // let line_width = 10.0;
    // let step_size = 2.0;
    // Temporary arbitrary direction
    let field_direction = vec2<f32>(1.0, 1.0); 

    // Multiplying by two ensures there's room for exactly two unique seeds per invocation
    let seed_1 = invocation_id.x * 2u; 
    let seed_2 = seed_1 + 1u;

    // view.viewport is vec4<f32>(x_orig, y_orid, width, height)
    let viewport_bottom_left = vec2<f32>(view.viewport.x - view.viewport.z / 2.0, view.viewport.y - view.viewport.w / 2.0);
    let joint_1 = vec2<f32>(viewport_bottom_left.x + random_f32(seed_1) * view.viewport.z,  viewport_bottom_left.y + random_f32(seed_2) * view.viewport.w);
    // let joint_1 = vec2<f32>(viewport_bottom_left.x + 0.5 * view.viewport.z,  viewport_bottom_left.y + 0.5 * view.viewport.w);
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

    // let d = globals.current_iteration;
    // index_buffer[first_triangle_index] = d;
    // index_buffer[first_triangle_index+1u] = d;
    // index_buffer[first_triangle_index+2u] = d;
    // index_buffer[first_triangle_index+3u] = d;
    // index_buffer[first_triangle_index+4u] = d;
    // index_buffer[first_triangle_index+5u] = d;

    // Debug
    // index_buffer[0] = u32(globals.current_iteration);
}

@compute @workgroup_size(16, 1, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    // let line_width = 10.0;
    // let step_size = 2.0;

    let base_vertex_index = 2u * globals.current_iteration + 2u * globals.max_iterations * invocation_id.x;
    let base_triangle_index = 6u * (globals.current_iteration - 1u) + 6u * (globals.max_iterations - 1u) * invocation_id.x;

    // Vertex position for previous line joint
    let prev_joint_v1_pos = vertex_buffer[base_vertex_index-u32(2)].position.xy;
    let prev_joint_v2_pos = vertex_buffer[base_vertex_index-u32(1)].position.xy;

    let prev_joint = prev_joint_v1_pos + 0.5 * (prev_joint_v2_pos - prev_joint_v1_pos);

    let field_angle = perlinNoise2(prev_joint * 0.003) * 2.0 * 3.1415;
    let field_direction = vec2<f32>(cos(field_angle), sin(field_angle));

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