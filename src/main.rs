//! Starwarden: a side-scrolling planet-defence shooter.
//!
//! Everything (art, sound, terrain) is generated in code; there are no asset files.

// Hide the console window on Windows release builds.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]
// Bevy systems routinely take many parameters and use complex query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod audio;
mod background;
mod colonists;
mod consts;
#[cfg(not(target_arch = "wasm32"))]
mod dev;
mod enemies;
mod fx;
mod hud;
mod input;
mod menus;
mod meshes;
mod player;
mod state;
mod terrain;
mod waves;
mod weapons;
mod world;

use bevy::camera::{Hdr, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::WindowResolution;

use consts::*;
use state::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Starwarden".into(),
            resolution: WindowResolution::new(1280, 720),
            // On the web the game fills the page and keeps the browser from scrolling or
            // zooming when the canvas is touched.
            canvas: Some("#game".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::BLACK))
    .init_state::<GameState>()
    .init_resource::<Game>()
    .init_resource::<Banner>()
    .init_resource::<Flash>()
    .init_resource::<world::GameCam>()
    .init_resource::<world::View>()
    .init_resource::<world::Rng>()
    .init_resource::<player::PlayerState>()
    .init_resource::<input::PlayerInput>()
    .init_resource::<input::TouchState>()
    .init_resource::<waves::WaveQueue>()
    .init_resource::<background::VoidBlend>()
    .add_message::<Damage>()
    .add_message::<PlayerHit>()
    .add_message::<Sfx>()
    .add_message::<Fx>()
    .configure_sets(
        Update,
        (
            Step::Input,
            Step::Logic,
            Step::Move,
            Step::Collide,
            Step::Resolve,
            Step::Present,
        )
            .chain(),
    );

    app.add_systems(PreStartup, meshes::setup_art).add_systems(
        Startup,
        (
            setup_camera,
            terrain::setup_terrain,
            background::setup_background,
            audio::setup_audio,
            hud::setup_hud,
            hud::setup_gizmos,
            fx::setup_glow_batch,
        ),
    );

    // Menus and state changes.
    app.add_systems(OnEnter(GameState::Title), menus::setup_title)
        .add_systems(OnEnter(GameState::Paused), menus::setup_paused)
        .add_systems(OnEnter(GameState::GameOver), menus::setup_game_over)
        .add_systems(OnExit(GameState::Title), waves::new_game)
        .add_systems(OnExit(GameState::GameOver), waves::new_game);

    // The world keeps simulating behind the game-over screen; only the player's own systems
    // and wave progression stop.

    app.add_systems(
        Update,
        (
            world::update_view,
            input::gather_input,
            menus::state_transitions,
        )
            .chain()
            .in_set(Step::Input),
    )
    .add_systems(
        Update,
        (
            (
                player::player_control,
                weapons::fire_weapons,
                waves::run_waves,
            )
                .chain()
                .run_if(playing),
            (enemies::enemy_ai, colonists::update_colonists)
                .chain()
                .run_if(simulating),
        )
            .chain()
            .in_set(Step::Logic),
    )
    .add_systems(
        Update,
        world::integrate.run_if(simulating).in_set(Step::Move),
    )
    .add_systems(
        Update,
        (
            (
                weapons::update_bolts,
                weapons::update_missiles,
                enemies::update_shots,
            )
                .chain()
                .run_if(simulating),
            enemies::player_collisions.run_if(playing),
        )
            .chain()
            .in_set(Step::Collide),
    )
    .add_systems(
        Update,
        (
            enemies::apply_damage.run_if(simulating),
            (
                player::player_death,
                player::player_respawn,
                world::camera_follow,
            )
                .chain()
                .run_if(playing),
        )
            .chain()
            .in_set(Step::Resolve),
    )
    .add_systems(
        Update,
        (
            (fx::spawn_fx, fx::update_particles, fx::update_popups)
                .chain()
                .run_if(simulating),
            world::sync_transforms,
            fx::draw_glows,
            world::camera_shake,
            apply_graphics_mode,
            terrain::update_terrain,
            background::update_background,
            hud::update_ui_scale,
            hud::update_hud,
            hud::update_touch_ui,
            hud::draw_overlays,
            menus::animate_menus,
            audio::play_sfx,
            audio::thrust_audio,
        )
            .chain()
            .in_set(Step::Present),
    );

    #[cfg(not(target_arch = "wasm32"))]
    dev::plugin(&mut app);

    app.run();
}

fn playing(state: Res<State<GameState>>) -> bool {
    *state.get() == GameState::Playing
}

fn simulating(state: Res<State<GameState>>) -> bool {
    matches!(state.get(), GameState::Playing | GameState::GameOver)
}

/// "Lite" graphics: no HDR, bloom or tonemapping, for devices that cannot afford them.
/// On the web the page sets `window.starwardenLite` (see `index.html`), and may do so while
/// the game is running. On the desktop set the `STARWARDEN_LITE` environment variable.
fn lite_graphics() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().is_some_and(|w| w.get("starwardenLite").is_some())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("STARWARDEN_LITE").is_ok()
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: VIEW_MIN_W,
                min_height: VIEW_MIN_H,
            },
            ..OrthographicProjection::default_2d()
        }),
        // MSAA and HDR do not mix on WebGL2; the bloom hides most of the aliasing anyway.
        #[cfg(target_arch = "wasm32")]
        Msaa::Off,
    ));
}

/// Adds or removes the camera's glow effects when the graphics mode changes.
fn apply_graphics_mode(
    mut commands: Commands,
    camera: Query<Entity, With<Camera2d>>,
    mut current: Local<Option<bool>>,
) {
    let lite = lite_graphics();
    if *current == Some(lite) {
        return;
    }
    let Ok(camera) = camera.single() else {
        return;
    };
    *current = Some(lite);
    let mut camera = commands.entity(camera);
    if lite {
        camera
            .remove::<(Hdr, Bloom, DebandDither)>()
            .insert(Tonemapping::None);
    } else {
        camera.insert((
            Hdr,
            Tonemapping::TonyMcMapface,
            Bloom {
                intensity: 0.22,
                ..Bloom::default()
            },
            DebandDither::Enabled,
        ));
    }
}
