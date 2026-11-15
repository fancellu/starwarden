//! Developer aid (desktop only): a scripted self-playing run that saves screenshots.
//!
//! Enable with the `STARWARDEN_DEMO` environment variable set to an output directory. Add
//! `STARWARDEN_DEMO_VOID=1` to also wipe out the colonists mid-run and show the void phase.
//! `STARWARDEN_DEMO_GALLERY=1` instead lines up every enemy type next to an idle ship.
//! It does nothing unless the variable is set.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::colonists::Colonist;
use crate::enemies::{spawn_enemy, Enemy, Kind};
use crate::meshes::Art;
use crate::world::Rng;
use crate::input::PlayerInput;
use crate::player::PlayerState;
use crate::world::{delta, Pos};

#[derive(Resource)]
pub struct Demo {
    dir: String,
    void: bool,
    gallery: bool,
    next_shot: usize,
    wiped: bool,
    /// Run length in seconds (`STARWARDEN_DEMO_SECS`, default 20).
    secs: f32,
    /// Seconds between screenshots (`STARWARDEN_DEMO_EVERY`, default 2.5). Use a small value
    /// such as 0.08 to capture frames for an animation.
    every: f32,
}

pub fn plugin(app: &mut App) {
    let Ok(dir) = std::env::var("STARWARDEN_DEMO") else {
        return;
    };
    app.insert_resource(Demo {
        dir,
        void: std::env::var("STARWARDEN_DEMO_VOID").is_ok(),
        gallery: std::env::var("STARWARDEN_DEMO_GALLERY").is_ok(),
        next_shot: 0,
        wiped: false,
        secs: std::env::var("STARWARDEN_DEMO_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20.0),
        every: std::env::var("STARWARDEN_DEMO_EVERY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2.5),
    })
    .add_systems(
        Update,
        demo.after(crate::input::gather_input)
            .before(crate::menus::state_transitions),
    );
}

fn demo(
    time: Res<Time>,
    mut commands: Commands,
    mut demo: ResMut<Demo>,
    mut input: ResMut<PlayerInput>,
    mut last: Local<f32>,
    colonists: Query<Entity, With<Colonist>>,
    enemies: Query<&Pos, With<Enemy>>,
    ps: Res<PlayerState>,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    let crossed = |at: f32| *last < at && t >= at;

    *input = PlayerInput::default();
    if crossed(1.2) {
        input.start = true;
    }
    if demo.gallery {
        if crossed(3.1) {
            let kinds = [
                Kind::Harvester,
                Kind::Wraith,
                Kind::Hive,
                Kind::Stinger,
                Kind::Stalker,
            ];
            for (i, kind) in kinds.into_iter().enumerate() {
                for elite in [false, true] {
                    let off = Vec2::new(
                        -320.0 + i as f32 * 130.0,
                        if elite { 150.0 } else { 80.0 },
                    );
                    spawn_enemy(&mut commands, &art, &mut rng, kind, elite, ps.pos + off, false);
                }
            }
        }
    } else if t > 1.5 {
        // Fly at the nearest enemy, then hold station a little way off and shoot.
        let nearest = enemies
            .iter()
            .map(|p| delta(ps.pos, p.0))
            .min_by(|a, b| a.length().total_cmp(&b.length()));
        match nearest {
            Some(d) => {
                input.mv.x = if d.x.abs() > 320.0 {
                    d.x.signum()
                } else if d.x.abs() < 140.0 {
                    -d.x.signum() * 0.6
                } else {
                    d.x.signum() * 0.2
                };
                input.mv.y = (d.y / 30.0).clamp(-1.0, 1.0);
            }
            None => input.mv.x = 1.0,
        }
        input.fire = true;
        input.missile = (t * 1.2).fract() < 0.05;
        input.bomb = crossed(12.0);
        // Keeps a finished run going so the game-over and restart paths are exercised too.
        input.start = (t * 0.5).fract() < 0.02;
    }
    if demo.void && !demo.wiped && t > 7.0 {
        demo.wiped = true;
        for e in &colonists {
            commands.entity(e).try_despawn();
        }
    }
    if t >= 0.9 + demo.next_shot as f32 * demo.every {
        let path = format!("{}/shot{:03}.png", demo.dir, demo.next_shot);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        demo.next_shot += 1;
    }
    if t > demo.secs {
        exit.write(AppExit::Success);
    }
    *last = t;
}
