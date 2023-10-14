use bevy::{
    asset::load_internal_asset,
    core_pipeline::{core_2d, tonemapping::Tonemapping},
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::CameraRenderGraph,
        extract_resource::ExtractResource,
        render_graph::{RenderGraphApp, ViewNodeRunner},
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        view::ColorGrading,
        Render, RenderApp, RenderSet,
    },
};

mod compute;
mod render;
mod utilities;

use compute::*;
use render::*;

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
    // commands.spawn(Camera2dBundle {
    //     camera_render_graph: CameraRenderGraph::new(FLOW_FIELD_RENDER_GRAPH),
    //     ..default()
    // });
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

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<FlowFieldGlobals>()
            .init_resource::<FlowFieldComputeResources>()
            .init_resource::<FlowFieldComputeBindGroup>()
            .init_resource::<FlowFieldRenderResources>()
            .init_resource::<FlowFieldRenderBindGroup>();

        render_app.add_systems(
            Render,
            (queue_compute_bind_group, queue_render_bind_group).in_set(RenderSet::Queue),
        );

        render_app
            .add_render_graph_node::<ViewNodeRunner<FlowFieldComputeNode>>(
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
                core_2d::graph::node::MAIN_PASS,
                "flow_field_compute_node",
                "flow_field_render_node",
                core_2d::graph::node::BLOOM,
            ],
        );
    }
}

#[derive(Component, Default)]
pub struct FlowFieldCameraLabel;

#[derive(Bundle)]
pub struct FlowFieldCameraBundle {
    pub label: FlowFieldCameraLabel,
    pub camera: Camera,
    pub camera_render_graph: CameraRenderGraph,
    pub projection: Projection,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub tonemapping: Tonemapping,
    pub color_grading: ColorGrading,
}

impl Default for FlowFieldCameraBundle {
    fn default() -> Self {
        Self {
            label: Default::default(),
            camera: Default::default(),
            camera_render_graph: CameraRenderGraph::new(FLOW_FIELD_RENDER_GRAPH),
            projection: Default::default(),
            transform: Default::default(),
            global_transform: Default::default(),
            tonemapping: Default::default(),
            color_grading: Default::default(),
        }
    }
}

#[derive(Resource, ExtractResource, ShaderType, Clone, Copy)]
pub struct FlowFieldGlobals {
    pub num_spawned_lines: u32,
    pub max_iterations: u32,
    pub current_iteration: u32,
    pub line_width: f32,
}

impl Default for FlowFieldGlobals {
    fn default() -> Self {
        Self {
            num_spawned_lines: 1,
            max_iterations: 10,
            current_iteration: 0,
            line_width: 10.0,
        }
    }
}

impl FlowFieldGlobals {
    fn to_buffer(
        &self,
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
    ) -> UniformBuffer<FlowFieldGlobals> {
        let mut buffer = UniformBuffer::from(*self);
        buffer.set_label(Some("flow_field_globals"));
        buffer.write_buffer(render_device, render_queue);
        buffer
    }
}
