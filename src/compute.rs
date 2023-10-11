use crate::*;
use bevy::render::{
    render_graph::{Node, NodeRunError, RenderGraphContext},
    renderer::{RenderContext, RenderDevice, RenderQueue},
};
use std::{borrow::Cow, mem::size_of};

pub struct FlowFieldComputeNode;

impl Node for FlowFieldComputeNode {
    fn update(&mut self, _world: &mut World) {}

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let resources = world.resource::<FlowFieldComputeResources>();
        let uniforms = world.resource::<FlowFieldUniforms>();

        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(resources.pipeline_id)
        else {
            return Ok(());
        };

        let command_encoder = render_context.command_encoder();
        let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("flow_field_compute_pass"),
        });

        pass.set_bind_group(0, &resources.bind_group, &[]);
        pass.set_pipeline(&compute_pipeline);
        pass.dispatch_workgroups(uniforms.num_spawned_lines / WORK_GROUP_SIZE, 1, 1);

        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();
        // read_buffer_u32(&bind_group.index_buffer, device, queue);
        // read_buffer_f32(&resources.vertex_buffer, device, queue);

        Ok(())
    }
}

impl FromWorld for FlowFieldComputeNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

#[derive(Resource)]
pub struct FlowFieldComputeResources {
    pub pipeline_id: CachedComputePipelineId,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
    pub uniforms_buffer: UniformBuffer<FlowFieldUniforms>,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

impl FromWorld for FlowFieldComputeResources {
    fn from_world(world: &mut World) -> Self {
        let uniforms = world.resource::<FlowFieldUniforms>();
        let render_device = world.resource::<RenderDevice>();
        let render_queue = world.resource::<RenderQueue>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let uniforms_buffer = uniforms.to_buffer(render_device, render_queue);

        let vertex_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("compute_vertex_buffer"),
            size: (size_of::<f32>() as u32
                * 16
                * uniforms.num_spawned_lines
                * (uniforms.max_iterations + 1))
                .into(),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let index_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("compute_index_buffer"),
            size: (size_of::<u32>() as u32
                * 6
                * uniforms.num_spawned_lines
                * uniforms.max_iterations)
                .into(),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    // Settings
                    BindGroupLayoutEntry {
                        binding: 0,
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
                        binding: 1,
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
                        binding: 2,
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

        let entries = &[
            BindGroupEntry {
                binding: 0,
                resource: uniforms_buffer.binding().unwrap(),
            },
            BindGroupEntry {
                binding: 1,
                resource: vertex_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: index_buffer.as_entire_binding(),
            },
        ];

        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("flow_field_bind_group"),
            layout: &bind_group_layout,
            entries,
        });

        let pipeline_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: vec![bind_group_layout.clone()],
            push_constant_ranges: vec![],
            shader: FLOW_FIELD_COMPUTE_SHADER.typed(),
            shader_defs: vec![],
            entry_point: Cow::from("init"),
        });

        Self {
            pipeline_id,
            bind_group_layout,
            bind_group,
            uniforms_buffer,
            vertex_buffer,
            index_buffer,
        }
    }
}
