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
    compute::{FlowFieldComputeResources, FlowFieldLineMeshBuffers},
    utilities::*,
    FlowFieldGlobals, WindowSize, FLOW_FIELD_RENDER_SHADER,
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
        let globals = world.resource::<FlowFieldGlobals>();
        let mesh_buffers = world.resource::<FlowFieldLineMeshBuffers>();
        let render_resources = world.resource::<FlowFieldRenderResources>();
        let bind_group = world.resource::<FlowFieldRenderBindGroup>();

        let pipeline_cache = world.resource::<PipelineCache>();
        let (Some(pipeline), Some(bind_group)) = (
            pipeline_cache.get_render_pipeline(render_resources.pipeline_id),
            bind_group.0.clone(),
        ) else {
            return Ok(());
        };

        let ms_render_target = world.resource::<MSRenderTarget>();
        if let (Some(target_view), Some(vertex_buffer), Some(index_buffer)) = (
            &ms_render_target.view,
            mesh_buffers.vertex_buffer.clone(),
            mesh_buffers.index_buffer.clone(),
        ) {
            // FOR DEBUG
            // let queue = world.resource::<RenderQueue>();
            // read_buffer_f32(&vertex_buffer, render_context.render_device(), &queue);
            // read_buffer_u32(&index_buffer, render_context.render_device(), &queue);

            let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("flow_field_render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: Some(view_target.main_texture_view()),
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            // let num_indices = 6 * globals.num_spawned_lines * (globals.current_iteration - 1);
            let num_indices = 6 * globals.num_lines * (globals.max_iterations - 1);

            pass.set_render_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[view_uniform_offset.offset]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), 0, IndexFormat::Uint32);
            pass.draw_indexed(0..num_indices, 0, 0..1);
        }

        Ok(())
    }
}

impl FromWorld for FlowFieldRenderNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

#[derive(Resource)]
pub struct MSRenderTarget {
    pub texture: Option<Texture>,
    pub view: Option<TextureView>,
}

impl Default for MSRenderTarget {
    fn default() -> Self {
        Self {
            texture: None,
            view: None,
        }
    }
}

pub fn create_ms_render_target(
    device: Res<RenderDevice>,
    globals: Res<FlowFieldGlobals>,
    mut ms_render_target: ResMut<MSRenderTarget>,
) {
    if ms_render_target.texture.is_none()
        || ms_render_target.view.is_none()
        || globals.should_reset == 1
    {
        let ms_texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: globals.viewport_width as u32,
                height: globals.viewport_height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 8,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::COPY_SRC
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[TextureFormat::Rgba8UnormSrgb],
        });

        let ms_view = ms_texture.create_view(&TextureViewDescriptor {
            label: None,
            format: Some(TextureFormat::Rgba8UnormSrgb),
            dimension: Some(TextureViewDimension::D2),
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        ms_render_target.texture = Some(ms_texture);
        ms_render_target.view = Some(ms_view);
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
                        // Globals
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
                            format: VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
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
                count: 8,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: FLOW_FIELD_RENDER_SHADER.typed(),
                shader_defs: vec![],
                entry_point: Cow::from("fragment"),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend: Some(BlendState::ALPHA_BLENDING),
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
    globals: Res<FlowFieldGlobals>,
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
                    resource: struct_to_buffer(*globals, &render_device, &render_queue)
                        .binding()
                        .unwrap(),
                },
            ],
        });

        *flow_field_bind_group = FlowFieldRenderBindGroup(Some(bind_group));
    }
}
