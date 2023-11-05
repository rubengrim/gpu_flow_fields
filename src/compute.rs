use crate::utilities::*;
use crate::*;
use bevy::{
    ecs::query::QueryItem,
    render::{
        render_graph::{NodeRunError, RenderGraphContext, ViewNode},
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::{ViewUniform, ViewUniformOffset, ViewUniforms},
    },
};
use std::{borrow::Cow, mem::size_of};

#[derive(Resource, Default, ShaderType, Clone, Copy)]
pub struct CurrentIterationCount {
    pub value: u32,
}

#[derive(Resource, Default)]
pub enum FlowFieldComputeState {
    #[default]
    Loading,
    Initializing,
    Updating,
    Finished,
}

pub struct FlowFieldComputeNode;

impl ViewNode for FlowFieldComputeNode {
    type ViewQuery = &'static ViewUniformOffset;

    fn update(&mut self, world: &mut World) {
        world.resource_scope(|world, mut state: Mut<FlowFieldComputeState>| {
            world.resource_scope(|world, mut iteration_count: Mut<CurrentIterationCount>| {
                let globals = world.resource::<FlowFieldGlobals>();

                if globals.should_reset == 1 {
                    *state = FlowFieldComputeState::Loading;
                    iteration_count.value = 0;
                }

                if !world.resource::<ShouldUpdateFlowField>().0 {
                    return;
                }

                let compute_resources = world.resource::<FlowFieldComputeResources>();
                let pipeline_cache = world.resource::<PipelineCache>();

                match *state {
                    FlowFieldComputeState::Loading => {
                        if let (CachedPipelineState::Ok(_), CachedPipelineState::Ok(_)) = (
                            pipeline_cache
                                .get_compute_pipeline_state(compute_resources.init_pipeline_id),
                            pipeline_cache
                                .get_compute_pipeline_state(compute_resources.update_pipeline_id),
                        ) {
                            // Init performs the equivalent of 2 iterations.
                            iteration_count.value = 2;
                            *state = FlowFieldComputeState::Initializing;
                        }
                    }
                    FlowFieldComputeState::Initializing => {
                        iteration_count.value += 1;
                        *state = FlowFieldComputeState::Updating;
                    }
                    FlowFieldComputeState::Updating => {
                        if iteration_count.value >= globals.max_iterations {
                            *state = FlowFieldComputeState::Finished;
                        } else {
                            iteration_count.value += 1;
                        }
                    }
                    FlowFieldComputeState::Finished => {}
                };
            });
        });
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        view_uniform_offset: QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if !world.resource::<ShouldUpdateFlowField>().0 {
            return Ok(());
        }

        let compute_resources = world.resource::<FlowFieldComputeResources>();
        let globals = world.resource::<FlowFieldGlobals>();
        let state = world.resource::<FlowFieldComputeState>();
        let bind_group = match world.resource::<FlowFieldComputeBindGroup>().0.clone() {
            Some(val) => val,
            None => return Ok(()),
        };

        let pipeline_cache = world.resource::<PipelineCache>();
        match *state {
            FlowFieldComputeState::Loading => {
                return Ok(());
            }
            FlowFieldComputeState::Initializing => {
                if let Some(init_pipeline) =
                    pipeline_cache.get_compute_pipeline(compute_resources.init_pipeline_id)
                {
                    let command_encoder = render_context.command_encoder();
                    let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("flow_field_compute_pass"),
                    });

                    pass.set_bind_group(0, &bind_group, &[view_uniform_offset.offset]);

                    // Init lines
                    pass.set_pipeline(&init_pipeline);
                    let num_workgroups =
                        (globals.num_lines as f32 / WORK_GROUP_SIZE as f32).ceil() as u32;
                    pass.dispatch_workgroups(num_workgroups, 1, 1);
                } else {
                    return Ok(());
                }
            }
            FlowFieldComputeState::Updating => {
                if let Some(update_pipeline) =
                    pipeline_cache.get_compute_pipeline(compute_resources.update_pipeline_id)
                {
                    let command_encoder = render_context.command_encoder();
                    let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("flow_field_compute_pass"),
                    });

                    pass.set_bind_group(0, &bind_group, &[view_uniform_offset.offset]);
                    pass.set_pipeline(&update_pipeline);
                    let num_workgroups =
                        (globals.num_lines as f32 / WORK_GROUP_SIZE as f32).ceil() as u32;
                    pass.dispatch_workgroups(num_workgroups, 1, 1);
                }
            }
            FlowFieldComputeState::Finished => {
                return Ok(());
            }
        };

        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();
        // read_buffer_u32(&index_buffer, device, queue);
        // read_buffer_f32(&compute_resources.field_grid_buffer, device, queue);

        Ok(())
    }
}

impl FromWorld for FlowFieldComputeNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

#[derive(Resource)]
pub struct FlowFieldLineMeshBuffers {
    pub vertex_buffer: Option<Buffer>,
    pub index_buffer: Option<Buffer>,
}

impl Default for FlowFieldLineMeshBuffers {
    fn default() -> Self {
        Self {
            vertex_buffer: None,
            index_buffer: None,
        }
    }
}

pub fn create_line_mesh_buffers(
    mut mesh_data: ResMut<FlowFieldLineMeshBuffers>,
    globals: Res<FlowFieldGlobals>,
    device: Res<RenderDevice>,
) {
    if mesh_data.vertex_buffer.is_none()
        || mesh_data.index_buffer.is_none()
        || globals.should_reset == 1
    {
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("compute_vertex_buffer"),
            size: (size_of::<f32>() as u32 * 16 * globals.num_lines * globals.max_iterations)
                .into(),
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("compute_index_buffer"),
            size: (size_of::<u32>() as u32 * 6 * globals.num_lines * (globals.max_iterations - 1))
                .into(),
            usage: BufferUsages::INDEX | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        mesh_data.vertex_buffer = Some(vertex_buffer);
        mesh_data.index_buffer = Some(index_buffer);
    }
}

#[derive(Resource)]
pub struct FlowFieldComputeResources {
    pub init_pipeline_id: CachedComputePipelineId,
    pub update_pipeline_id: CachedComputePipelineId,
    pub bind_group_layout: BindGroupLayout,
}

impl FromWorld for FlowFieldComputeResources {
    fn from_world(world: &mut World) -> Self {
        // let window_size = world.resource::<WindowSize>();
        let render_device = world.resource::<RenderDevice>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    // View
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(ViewUniform::min_size()),
                        },
                        count: None,
                    },
                    // Globals
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Iteration count
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Vertex buffer
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Index buffer
                    BindGroupLayoutEntry {
                        binding: 4,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let init_pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some(Cow::from("flow_field_init_pipeline")),
            layout: vec![bind_group_layout.clone()],
            push_constant_ranges: vec![],
            shader: FLOW_FIELD_COMPUTE_SHADER.typed(),
            shader_defs: vec![],
            entry_point: Cow::from("init"),
        });

        let update_pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some(Cow::from("flow_field_update_pipeline")),
            layout: vec![bind_group_layout.clone()],
            push_constant_ranges: vec![],
            shader: FLOW_FIELD_COMPUTE_SHADER.typed(),
            shader_defs: vec![],
            entry_point: Cow::from("update"),
        });

        Self {
            init_pipeline_id,
            update_pipeline_id,
            bind_group_layout,
        }
    }
}

#[derive(Resource, Default)]
pub struct FlowFieldComputeBindGroup(pub Option<BindGroup>);

pub fn queue_compute_bind_group(
    mut compute_bind_group: ResMut<FlowFieldComputeBindGroup>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    compute_resources: Res<FlowFieldComputeResources>,
    mesh_buffers: Res<FlowFieldLineMeshBuffers>,
    view_uniforms: Res<ViewUniforms>,
    globals: Res<FlowFieldGlobals>,
    iteration_count: Res<CurrentIterationCount>,
) {
    // let globals_buffer = globals.to_buffer(&*render_device, &*render_queue);
    let globals_buffer = struct_to_buffer(*globals, &*render_device, &*render_queue);
    let iteration_buffer = struct_to_buffer(*iteration_count, &*render_device, &*render_queue);
    // info!("{}", iteration_count.value);

    if let (Some(view_uniforms), Some(vertex_buffer), Some(index_buffer)) = (
        view_uniforms.uniforms.binding(),
        mesh_buffers.vertex_buffer.clone(),
        mesh_buffers.index_buffer.clone(),
    ) {
        let entries = &[
            BindGroupEntry {
                binding: 0,
                resource: view_uniforms.clone(),
            },
            BindGroupEntry {
                binding: 1,
                resource: globals_buffer.binding().unwrap(),
            },
            BindGroupEntry {
                binding: 2,
                resource: iteration_buffer.binding().unwrap(),
            },
            BindGroupEntry {
                binding: 3,
                resource: vertex_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: index_buffer.as_entire_binding(),
            },
        ];

        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("flow_field_compute_bind_group"),
            layout: &compute_resources.bind_group_layout,
            entries,
        });

        *compute_bind_group = FlowFieldComputeBindGroup(Some(bind_group));
    }
}
