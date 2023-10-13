use std::mem::size_of;

use bevy::{
    ecs::query::QueryItem,
    prelude::*,
    render::{
        render_graph::{NodeRunError, RenderGraphContext, ViewNode},
        render_resource::*,
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
    },
};
use std::borrow::Cow;
use wgpu::{
    ColorTargetState, MultisampleState, PrimitiveState, VertexAttribute, VertexFormat,
    VertexStepMode,
};

use crate::{
    compute::FlowFieldComputeResources, utilities::*, FlowFieldSettings, FLOW_FIELD_RENDER_SHADER,
};

pub struct FlowFieldRenderNode;

impl ViewNode for FlowFieldRenderNode {
    type ViewQuery = (&'static ViewTarget, &'static ViewUniformOffset);

    fn update(&mut self, _world: &mut World) {}

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, view_uniform_offset): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let uniforms = world.resource::<FlowFieldSettings>();
        let compute_resources = world.resource::<FlowFieldComputeResources>();
        let render_resources = world.resource::<FlowFieldRenderResources>();
        let bind_group = world.resource::<FlowFieldRenderBindGroup>();

        let pipeline_cache = world.resource::<PipelineCache>();
        let (Some(pipeline), Some(bind_group)) = (
            pipeline_cache.get_render_pipeline(render_resources.pipeline_id),
            bind_group.0.clone(),
        ) else {
            return Ok(());
        };

        let vertex_buffer = render_context
            .render_device()
            .create_buffer(&BufferDescriptor {
                label: Some("vertex_buffer"),
                size: (size_of::<f32>() as u32 * 16 * uniforms.num_spawned_lines * 2).into(),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

        let num_indices = 6 * uniforms.num_spawned_lines;
        let index_buffer = render_context
            .render_device()
            .create_buffer(&BufferDescriptor {
                label: Some("index_buffer"),
                size: (size_of::<u32>() as u32 * num_indices).into(),
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

        let mut encoder = render_context
            .render_device()
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        encoder.copy_buffer_to_buffer(
            &compute_resources.vertex_buffer,
            0,
            &vertex_buffer,
            0,
            vertex_buffer.size(),
        );

        encoder.copy_buffer_to_buffer(
            &compute_resources.index_buffer,
            0,
            &index_buffer,
            0,
            index_buffer.size(),
        );

        let queue = world.resource::<RenderQueue>();
        queue.submit([encoder.finish()]);

        read_buffer_f32(&vertex_buffer, render_context.render_device(), &queue);
        read_buffer_u32(&index_buffer, render_context.render_device(), &queue);

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: view_target.main_texture_view(),
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color::WHITE),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        pass.set_render_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[view_uniform_offset.offset]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), 0, IndexFormat::Uint32);
        pass.draw_indexed(0..num_indices, 0, 0..1);

        Ok(())
    }
}

impl FromWorld for FlowFieldRenderNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

#[derive(Resource)]
pub struct FlowFieldRenderResources {
    pub pipeline_id: CachedRenderPipelineId,
    pub bind_group_layout: BindGroupLayout,
}

impl FromWorld for FlowFieldRenderResources {
    fn from_world(world: &mut World) -> Self {
        let pipeline_cache = world.resource::<PipelineCache>();

        let bind_group_layout =
            world
                .resource::<RenderDevice>()
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        // View uniforms
                        BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::VERTEX,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: true,
                                min_binding_size: Some(ViewUniform::min_size()),
                            },
                            count: None,
                        },
                        // Settings
                        BindGroupLayoutEntry {
                            binding: 1,
                            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        // let settings = world.resource::<FlowFieldSettings>();
        // let settings_buffer = settings.to_buffer(render_device, render_queue);

        // let view_uniforms = world.resource::<ViewUniforms>();

        let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: None,
            layout: vec![bind_group_layout.clone()],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: FLOW_FIELD_RENDER_SHADER.typed(),
                shader_defs: vec![],
                entry_point: Cow::from("vertex"),
                buffers: vec![VertexBufferLayout {
                    array_stride: (size_of::<f32>() * 8) as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x3,
                            offset: (size_of::<f32>() * 4) as u64,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                // cull_mode: Some(Face::Back),
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: FLOW_FIELD_RENDER_SHADER.typed(),
                shader_defs: vec![],
                entry_point: Cow::from("fragment"),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
        });

        Self {
            pipeline_id,
            bind_group_layout,
        }
    }
}

#[derive(Resource, Default)]
pub struct FlowFieldRenderBindGroup(pub Option<BindGroup>);

pub fn queue_render_bind_group(
    mut flow_field_bind_group: ResMut<FlowFieldRenderBindGroup>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    render_resources: Res<FlowFieldRenderResources>,
    view_uniforms: Res<ViewUniforms>,
    settings: Res<FlowFieldSettings>,
) {
    if let Some(view_uniforms) = view_uniforms.uniforms.binding() {
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &render_resources.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: view_uniforms.clone(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: settings
                        .to_buffer(&render_device, &render_queue)
                        .binding()
                        .unwrap(),
                },
            ],
        });

        *flow_field_bind_group = FlowFieldRenderBindGroup(Some(bind_group));
    }
}
