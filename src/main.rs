use bevy::{
    asset::load_internal_asset,
    core_pipeline::{
        core_2d,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        tonemapping::Tonemapping,
    },
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::CameraRenderGraph,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{RenderGraphApp, ViewNodeRunner},
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        view::ColorGrading,
        Render, RenderApp, RenderSet,
    },
    time::Stopwatch,
    window::{WindowResized, WindowResolution},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

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

const WORK_GROUP_SIZE: u32 = 16;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            LogDiagnosticsPlugin::default(),
            FrameTimeDiagnosticsPlugin::default(),
            FlowFieldPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // commands.spawn(Camera2dBundle {
    //     camera_render_graph: CameraRenderGraph::new(FLOW_FIELD_RENDER_GRAPH),
    //     ..default()
    // });
    commands.spawn((Camera2dBundle::default(),));
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

        app.add_plugins(EguiPlugin).add_systems(Update, update_ui);

        app.insert_resource(FlowFieldStopwatch(Stopwatch::new()))
            .init_resource::<ShouldUpdateFlowField>()
            .add_plugins(ExtractResourcePlugin::<ShouldUpdateFlowField>::default())
            .add_systems(Update, update_flow_field_stopwatch);

        let window = app.world.query::<&Window>().single(&app.world);
        app.insert_resource(FlowFieldGlobals {
            viewport_width: window.resolution.width(),
            viewport_height: window.resolution.height(),
            ..default()
        })
        .add_plugins(ExtractResourcePlugin::<FlowFieldGlobals>::default());

        // app.sub_app_mut(RenderApp).insert_resource(WindowSize {
        //     width: width as u32,
        //     height: height as u32,
        //     resized: true,
        // });

        // app.insert_resource(WindowSize {
        //     width: window.resolution.width() as u32,
        //     height: window.resolution.height() as u32,
        //     resized: true,
        // })
        // .add_plugins(ExtractResourcePlugin::<WindowSize>::default())
        // .add_systems(Update, on_window_resize);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<FlowFieldComputeState>()
            .init_resource::<CurrentIterationCount>()
            .init_resource::<FlowFieldLineMeshBuffers>()
            .init_resource::<FlowFieldComputeResources>()
            .init_resource::<FlowFieldComputeBindGroup>()
            .init_resource::<MSRenderTarget>()
            .init_resource::<FlowFieldRenderResources>()
            .init_resource::<FlowFieldRenderBindGroup>();

        render_app
            .add_systems(
                Render,
                (create_ms_render_target, create_line_mesh_buffers).in_set(RenderSet::Prepare),
            )
            .add_systems(
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

pub fn update_ui(mut contexts: EguiContexts) {
    egui::Window::new("Hello").show(contexts.ctx_mut(), |ui| {
        ui.label("world");
    });
}

#[derive(Resource, Clone, ExtractResource)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
    // Resized this frame
    pub resized: bool,
}

pub fn on_window_resize(
    mut window_size: ResMut<WindowSize>,
    mut resize_event_reader: EventReader<WindowResized>,
) {
    window_size.resized = false;
    for e in resize_event_reader.iter() {
        window_size.width = e.width as u32;
        window_size.height = e.height as u32;
        window_size.resized = true;
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
    // The flow field state will reset if set to 1
    pub should_reset: u32,
    pub paused: u32,
    // Does not update when resizing window
    pub viewport_width: f32,
    // Does not update when resizing window
    pub viewport_height: f32,
    pub num_lines: u32,
    // Needs to be > 2 because the init stage does 2 iterations
    pub max_iterations: u32,
    // Step size of particles per iteration in pixels
    pub step_size: f32,
    // Upper bound on particle speed in pixels per second. May still go slower depending on framerate.
    // step_size takes priority over particle speed
    pub max_particle_speed: f32,
    pub line_width: f32,
    pub line_rgba: Vec4,
    // Snaps line angle if this is > 0.
    // If == 10 the line angles will snap to multiples of (pi/2)/10.
    // If == 0 no snapping will be done.
    pub num_angles_allowed: u32,
    pub noise_scale: f32,
    pub field_offset_x: f32,
    pub field_offset_y: f32,
}

impl Default for FlowFieldGlobals {
    fn default() -> Self {
        Self {
            should_reset: 0,
            paused: 0,
            viewport_width: 640.0,
            viewport_height: 480.0,
            num_lines: 20000,
            max_iterations: 300,
            step_size: 1.0,
            max_particle_speed: 200.0,
            line_width: 1.0,
            line_rgba: Vec4::new(0.1, 1.0, 0.2, 0.1),
            num_angles_allowed: 0,
            noise_scale: 0.005,
            field_offset_x: 0.0,
            field_offset_y: 0.0,
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

#[derive(Resource, Clone, ExtractResource, Default)]
pub struct ShouldUpdateFlowField(pub bool);

#[derive(Resource, Clone, ExtractResource, Default)]
pub struct FlowFieldStopwatch(pub Stopwatch);

pub fn update_flow_field_stopwatch(
    time: Res<Time>,
    mut stopwatch: ResMut<FlowFieldStopwatch>,
    mut should_update: ResMut<ShouldUpdateFlowField>,
    globals: Res<FlowFieldGlobals>,
) {
    let time_step = globals.step_size / globals.max_particle_speed;
    if stopwatch.0.elapsed_secs() >= time_step {
        stopwatch.0.reset();
        should_update.0 = true;
    } else {
        stopwatch.0.tick(time.delta());
        should_update.0 = false;
    }
}
