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
    utils::tracing::Instrument,
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
            // LogDiagnosticsPlugin::default(),
            // FrameTimeDiagnosticsPlugin::default(),
            FlowFieldPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
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

        app.add_systems(Update, on_window_resize);

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

pub fn update_ui(mut contexts: EguiContexts, mut globals: ResMut<FlowFieldGlobals>) {
    egui::Window::new("Settings").show(contexts.ctx_mut(), |ui| {
        let mut should_reset = false;

        ui.horizontal(|ui| {
            ui.label("Number of lines");
            if ui
                .add(
                    egui::DragValue::new(&mut globals.num_lines)
                        .speed(1.0)
                        .clamp_range(1..=100000),
                )
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Number of iterations");
            if ui
                .add(
                    egui::DragValue::new(&mut globals.max_iterations)
                        .speed(1.0)
                        .clamp_range(2..=2000),
                )
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Iteration step size");
            if ui
                .add(egui::DragValue::new(&mut globals.step_size).speed(0.1).clamp_range(0..=1000)).on_hover_text("The distance (in pixels) every line is extended per iteration")
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Line width");
            if ui
                .add(egui::DragValue::new(&mut globals.line_width).speed(0.1))
                .changed()
            {
                should_reset = true;
            }
        });

        let mut rgba_start = [
            globals.line_color_start.x,
            globals.line_color_start.y,
            globals.line_color_start.z,
            globals.line_color_start.w,
        ];
        let mut rgba_end = [
            globals.line_color_end.x,
            globals.line_color_end.y,
            globals.line_color_end.z,
            globals.line_color_end.w,
        ];
        ui.horizontal(|ui| {
            ui.label("Color start ");
            if ui
                .color_edit_button_rgba_premultiplied(&mut rgba_start)
                .changed()
            {
                should_reset = true;
            }
            ui.label("Color end ");
            if ui
                .color_edit_button_rgba_premultiplied(&mut rgba_end)
                .changed()
            {
                should_reset = true;
            }
        });
        globals.line_color_start = Vec4::from_array(rgba_start);
        globals.line_color_end = Vec4::from_array(rgba_end);

        let mut rgba_background = [
            globals.background_color.x,
            globals.background_color.y,
            globals.background_color.z,
            globals.background_color.w,
        ];
        ui.horizontal(|ui| {
            ui.label("Background color");
            ui.color_edit_button_rgba_premultiplied(&mut rgba_background)
                .changed();
        });
        globals.background_color = Vec4::from_array(rgba_background);

        ui.horizontal(|ui| {
            ui.label("Number of angles");
            if ui
                .add(egui::DragValue::new(&mut globals.num_angles_allowed).speed(1.0))
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Sine modulation frequency").on_hover_text(
                "The frequency of the sine wave by which the flow field direction is modulated",
            );
            if ui
                .add(egui::DragValue::new(&mut globals.angle_modulation_frequency).speed(0.001).clamp_range(0..=10))
                .on_hover_text(
                    "The frequency of the sine wave by which the flow field direction is being modulated",
                )
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Sine modulation strength").on_hover_text("The strength of the sine wave modulation done to the flow field direction.");
            if ui
                .add(
                    egui::DragValue::new(&mut globals.angle_modulation_strength)
                        .speed(0.001)
                        .clamp_range(0.0..=1.0),
                ).on_hover_text("The strength of the sine wave modulation done to the flow field direction.")
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Noise scale");
            if ui
                .add(
                    egui::DragValue::new(&mut globals.noise_scale)
                        .speed(0.00005)
                        .clamp_range(0.0..=1.0),
                )
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Field offset");
            if ui
                .add(
                    egui::DragValue::new(&mut globals.field_offset_y)
                        .speed(1.0)
                        .prefix("x:"),
                )
                .changed()
            {
                should_reset = true;
            }

            if ui
                .add(
                    egui::DragValue::new(&mut globals.field_offset_x)
                        .speed(1.0)
                        .prefix("y:"),
                )
                .changed()
            {
                should_reset = true;
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Reset").clicked() {
                should_reset = true;
            }
            let mut paused_bool = globals.paused == 1;
            ui.checkbox(&mut paused_bool, "Paused");
            globals.paused = if paused_bool { 1 } else { 0 };
            // globals.paused = 1;
        });

        if should_reset {
            globals.should_reset = 1;
        } else {
            globals.should_reset = 0;
        }
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
    mut globals: ResMut<FlowFieldGlobals>,
    mut resize_event_reader: EventReader<WindowResized>,
) {
    for e in resize_event_reader.iter() {
        globals.viewport_width = e.width;
        globals.viewport_height = e.height;
        globals.should_reset = 1;
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
    pub viewport_width: f32,
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
    pub line_color_start: Vec4,
    pub line_color_end: Vec4,
    pub background_color: Vec4,
    // Snaps line angle if this is > 0.
    // If == 10 the line angles will snap to multiples of (pi/2)/10.
    // If == 0 no snapping will be done.
    pub num_angles_allowed: u32,
    pub angle_modulation_frequency: f32,
    pub angle_modulation_strength: f32,
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
            num_lines: 40000,
            max_iterations: 300,
            step_size: 1.0,
            max_particle_speed: 30.0,
            line_width: 1.0,
            line_color_start: Vec4::new(0.0, 0.0, 0.0, 0.1),
            line_color_end: Vec4::new(0.0, 0.0, 0.0, 0.1),
            background_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            num_angles_allowed: 0,
            angle_modulation_frequency: 0.1,
            angle_modulation_strength: 0.0,
            noise_scale: 0.005,
            field_offset_x: 0.0,
            field_offset_y: 0.0,
        }
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
    if globals.paused == 0 && stopwatch.0.elapsed_secs() >= time_step {
        stopwatch.0.reset();
        should_update.0 = true;
    } else {
        stopwatch.0.tick(time.delta());
        should_update.0 = false;
    }
}
