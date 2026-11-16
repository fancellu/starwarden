//! Developer aid (desktop only): a scripted self-playing run that saves screenshots.
//!
//! Enable with the `STARWARDEN_DEMO` environment variable set to an output directory. Add
//! `STARWARDEN_DEMO_VOID=1` to also wipe out the colonists mid-run and show the void phase.
//! `STARWARDEN_DEMO_GALLERY=1` instead lines up every enemy type next to an idle ship.
//! `STARWARDEN_DEMO_CARRY=1` instead clears the enemies, knocks three colonists loose high up,
//! catches them, carries them for a while and sets them down together (use about 22 seconds).
//! `STARWARDEN_DEMO_POWER=1` drops each powerup in turn near the ship, one every 6 seconds from
//! 3 s, for the pilot to catch (use about 36 seconds). With `STARWARDEN_DEMO_GALLERY=1` as well,
//! it instead lines up a shield and every powerup, each named, above an idle ship.
//! It does nothing unless the variable is set.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::colonists::{CState, Colonist};
use crate::consts::TOP_Y;
use crate::enemies::{spawn_enemy, Enemy, Kind};
use crate::input::PlayerInput;
use crate::meshes::Art;
use crate::pickups::{spawn_power_of, spawn_shield, Pickup, PickupKind};
use crate::player::PlayerState;
use crate::state::{Fx, FxKind, Power};
use crate::world::Rng;
use crate::world::{delta, Pos};

#[derive(Resource)]
pub struct Demo {
    dir: String,
    void: bool,
    gallery: bool,
    carry: bool,
    power: bool,
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
        carry: std::env::var("STARWARDEN_DEMO_CARRY").is_ok(),
        power: std::env::var("STARWARDEN_DEMO_POWER").is_ok(),
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

#[allow(clippy::too_many_arguments)]
fn demo(
    time: Res<Time>,
    mut commands: Commands,
    mut demo: ResMut<Demo>,
    mut input: ResMut<PlayerInput>,
    mut last: Local<f32>,
    mut colonists: Query<(Entity, &mut Colonist, &mut Pos), Without<Enemy>>,
    enemies: Query<(Entity, &Pos), With<Enemy>>,
    pickups: Query<&Pos, (With<Pickup>, Without<Colonist>)>,
    ps: Res<PlayerState>,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut fx: MessageWriter<Fx>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    let crossed = |at: f32| *last < at && t >= at;

    *input = PlayerInput::default();
    if crossed(1.2) {
        input.start = true;
    }
    if demo.gallery && demo.power {
        // A row of every pickup, each named, above an idle ship.
        // After the wave banner has faded.
        if crossed(7.0) {
            let kinds = std::iter::once(PickupKind::Shield)
                .chain(Power::ALL.into_iter().map(PickupKind::Power));
            for (i, kind) in kinds.enumerate() {
                let at = ps.pos + Vec2::new(ps.facing * 200.0 + (i as f32 - 2.5) * 175.0, 110.0);
                let name = match kind {
                    PickupKind::Shield => {
                        spawn_shield(&mut commands, &art, &mut rng, at);
                        "SHIELD"
                    }
                    PickupKind::Power(p) => {
                        spawn_power_of(&mut commands, &art, &mut rng, at, p);
                        p.name()
                    }
                };
                fx.write(Fx::new(
                    at - Vec2::new(0.0, 75.0),
                    kind.color(),
                    FxKind::Label(name),
                ));
            }
        }
    } else if demo.gallery {
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
                    let off =
                        Vec2::new(-320.0 + i as f32 * 130.0, if elite { 150.0 } else { 80.0 });
                    spawn_enemy(
                        &mut commands,
                        &art,
                        &mut rng,
                        kind,
                        elite,
                        ps.pos + off,
                        false,
                    );
                }
            }
        }
    } else if demo.carry {
        if t > 1.5 {
            carry_demo(
                t,
                &crossed,
                &mut commands,
                &mut input,
                &mut colonists,
                &enemies,
                &ps,
            );
        }
    } else if t > 1.5 {
        if demo.power && ps.alive {
            for (i, &p) in Power::ALL.iter().enumerate() {
                if crossed(3.0 + 6.0 * i as f32) {
                    let at = ps.pos + Vec2::new(0.0, 70.0);
                    spawn_power_of(&mut commands, &art, &mut rng, at, p);
                }
            }
        }
        // Go for a nearby falling pickup; otherwise fly at the nearest enemy, then hold
        // station a little way off and shoot.
        let shield = pickups
            .iter()
            .map(|p| delta(ps.pos, p.0))
            .filter(|d| d.length() < 300.0)
            .min_by(|a, b| a.length().total_cmp(&b.length()));
        let nearest = enemies
            .iter()
            .map(|(_, p)| delta(ps.pos, p.0))
            .min_by(|a, b| a.length().total_cmp(&b.length()));
        match (shield, nearest) {
            (Some(d), _) => {
                input.mv = Vec2::new((d.x / 60.0).clamp(-1.0, 1.0), (d.y / 30.0).clamp(-1.0, 1.0));
            }
            (None, Some(d)) => {
                input.mv.x = if d.x.abs() > 320.0 {
                    d.x.signum()
                } else if d.x.abs() < 140.0 {
                    -d.x.signum() * 0.6
                } else {
                    d.x.signum() * 0.2
                };
                input.mv.y = (d.y / 30.0).clamp(-1.0, 1.0);
                // Sprint to close a long gap.
                input.sprint = d.x.abs() > 700.0;
            }
            (None, None) => {
                input.mv.x = 1.0;
                input.sprint = true;
            }
        }
        input.fire = true;
        input.missile = (t * 1.2).fract() < 0.05;
        input.bomb = crossed(12.0);
        // Keeps a finished run going so the game-over and restart paths are exercised too.
        input.start = (t * 0.5).fract() < 0.02;
    }
    if demo.void && !demo.wiped && t > 7.0 {
        demo.wiped = true;
        for (e, _, _) in &colonists {
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

/// Seconds at which a colonist is knocked loose above the ship.
const CARRY_DROPS: [f32; 3] = [3.0, 6.0, 9.0];
/// Height the ship cruises at while carrying.
const CARRY_CRUISE_Y: f32 = 150.0;

/// The carry demo: no enemies; catch three falling colonists, cruise with them, set them down.
fn carry_demo(
    t: f32,
    crossed: &dyn Fn(f32) -> bool,
    commands: &mut Commands,
    input: &mut PlayerInput,
    colonists: &mut Query<(Entity, &mut Colonist, &mut Pos), Without<Enemy>>,
    enemies: &Query<(Entity, &Pos), With<Enemy>>,
    ps: &PlayerState,
) {
    for (e, _) in enemies {
        commands.entity(e).try_despawn();
    }
    if !ps.alive {
        return;
    }
    // As if a Harvester carrying the colonist had just been shot near the top of the screen.
    if CARRY_DROPS.iter().any(|&at| crossed(at)) {
        if let Some((_, mut c, mut pos)) = colonists
            .iter_mut()
            .find(|(_, c, _)| c.state == CState::Walking)
        {
            c.state = CState::Falling;
            c.claimed_by = None;
            c.vy = 0.0;
            pos.0 = Vec2::new(ps.pos.x + 40.0, TOP_Y - 10.0);
            c.fall_from = pos.0.y;
        }
    }
    let falling = colonists
        .iter()
        .filter(|(_, c, _)| c.state == CState::Falling)
        .map(|(_, _, p)| delta(ps.pos, p.0))
        .min_by(|a, b| a.length().total_cmp(&b.length()));
    let last_drop = CARRY_DROPS[CARRY_DROPS.len() - 1];
    if let Some(d) = falling {
        // Go and catch it.
        input.mv = Vec2::new((d.x / 60.0).clamp(-1.0, 1.0), (d.y / 30.0).clamp(-1.0, 1.0));
    } else if t < last_drop + 2.0 {
        // Wait at cruising height for the next one.
        input.mv.y = ((CARRY_CRUISE_Y - ps.pos.y) / 30.0).clamp(-1.0, 1.0);
    } else if t < last_drop + 6.0 {
        // Cruise at full throttle to show the ship is slower when loaded.
        input.mv = Vec2::new(1.0, ((CARRY_CRUISE_Y - ps.pos.y) / 30.0).clamp(-1.0, 1.0));
    } else {
        // Dive to set everyone down together.
        input.mv = Vec2::new(0.3, -1.0);
    }
}
