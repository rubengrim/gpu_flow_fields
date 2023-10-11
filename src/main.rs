use bevy::{
    asset::load_internal_asset,
    core_pipeline::*,
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::CameraRenderGraph,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{
            Node, NodeRunError, RenderGraph, RenderGraphApp, RenderGraphContext, ViewNode,
            ViewNodeRunner,
        },
        render_resource::*,
        renderer::{RenderContext, RenderDevice, RenderQueue},
        Render, RenderApp, RenderSet,
    },
    utils::Uuid,
    window::WindowPlugin,
};
use std::mem::size_of;
use std::num::NonZeroU64;
use std::ops::Deref;

mod compute;
mod render;
mod utilities;

use compute::*;
use render::*;
use utilities::*;

const FLOW_FIELD_RENDER_GRAPH: &str = "flow_field_graph";

const FLOW_FIELD_COMPUTE_SHADER: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 3935433275);
const FLOW_FIELD_RENDER_SHADER: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2388554825);

const WORK_GROUP_SIZE: u32 = 1;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FlowFieldPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

struct FlowFieldPlugin;

impl Plugin for FlowFieldPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            FLOW_FIELD_COMPUTE_SHADER,
            "flow_field_compute.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            FLOW_FIELD_RENDER_SHADER,
            "flow_field_render.wgsl",
            Shader::from_wgsl
        );
    }

    fn finish(&self, app: &mut App) {
        // app.init_resource::<FlowFieldUniforms>();

        app.sub_app_mut(RenderApp)
            .init_resource::<FlowFieldUniforms>()
            .init_resource::<FlowFieldComputeResources>()
            .init_resource::<FlowFieldRenderResources>();

        // app.add_plugins(ExtractResourcePlugin::<FlowFieldUniforms>::default());

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            // .add_render_sub_graph(FLOW_FIELD_RENDER_GRAPH)
            .add_render_graph_node::<FlowFieldComputeNode>(
                core_2d::graph::NAME,
                "flow_field_compute_node",
            )
            .add_render_graph_node::<ViewNodeRunner<FlowFieldRenderNode>>(
                core_2d::graph::NAME,
                "flow_field_render_node",
            );
        render_app.add_render_graph_edges(
            core_2d::graph::NAME,
            &[
                "flow_field_compute_node",
                "flow_field_render_node",
                core_2d::graph::node::MSAA_WRITEBACK,
            ],
        );
    }
}

#[derive(Resource, ExtractResource, ShaderType, Clone, Copy)]
pub struct FlowFieldUniforms {
    pub num_spawned_lines: u32,
    pub max_iterations: u32,
    pub current_iteration: u32,
    pub viewport_width: f32,
    pub viewport_height: f32,
}

impl Default for FlowFieldUniforms {
    fn default() -> Self {
        Self {
            num_spawned_lines: 1,
            max_iterations: 1,
            current_iteration: 0,
            viewport_width: 1280.0,
            viewport_height: 720.0,
        }
    }
}

impl FlowFieldUniforms {
    fn to_buffer(
        &self,
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
    ) -> UniformBuffer<FlowFieldUniforms> {
        let mut buffer = UniformBuffer::from(*self);
        buffer.set_label(Some("flow_field_uniforms"));
        buffer.write_buffer(render_device, render_queue);
        buffer
    }
}
