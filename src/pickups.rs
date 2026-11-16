//! Shields dropped by destroyed enemies: they drift down and must be caught before they reach
//! the ground. Each one held absorbs a hit that would otherwise destroy the ship.

use bevy::prelude::*;

use crate::consts::*;
use crate::enemies::Kind;
use crate::fx::Glow;
use crate::meshes::Art;
use crate::player::PlayerState;
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Component)]
pub struct ShieldPickup {
    /// Seconds since it was dropped.
    pub t: f32,
    /// Phase of its sideways sway, so neighbours do not move in step.
    pub sway: f32,
}

/// Chance that a destroyed enemy of this kind drops a shield.
pub fn drop_chance(kind: Kind, elite: bool) -> f32 {
    let base = if kind == Kind::Hive {
        SHIELD_DROP_HIVE
    } else {
        SHIELD_DROP
    };
    if elite {
        base * 2.0
    } else {
        base
    }
}

pub fn spawn_shield(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2) {
    commands.spawn((
        ShieldPickup {
            t: 0.0,
            sway: rng.range(0.0, 6.0),
        },
        GameEntity,
        Pos(pos),
        Vel(Vec2::new(0.0, -SHIELD_FALL)),
        Glow {
            color: with_alpha(C_CYAN, 0.35),
            size: Vec2::splat(46.0),
            soft: true,
        },
        Mesh2d(art.shield.clone()),
        MeshMaterial2d(art.mat.clone()),
        Transform::from_xyz(0.0, pos.y, Z_SHOT),
    ));
}

/// Moves falling shields, lets the ship catch them and removes those that reach the ground.
pub fn update_pickups(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut commands: Commands,
    ps: Res<PlayerState>,
    mut game: ResMut<Game>,
    mut q: Query<(Entity, &mut ShieldPickup, &Pos, &mut Vel, &mut Transform)>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    let playing = *state.get() == GameState::Playing;
    for (e, mut s, pos, mut vel, mut tf) in &mut q {
        s.t += dt;
        vel.0.x = (s.t * 1.7 + s.sway).sin() * 22.0;
        tf.rotation = Quat::from_rotation_z(s.t * 1.3);

        if playing && ps.alive && delta(ps.pos, pos.0).length() < SHIELD_REACH {
            commands.entity(e).try_despawn();
            fx.write(Fx::new(pos.0, C_CYAN, FxKind::Rescue));
            if game.add_shield() {
                sfx.write(Sfx::ShieldUp);
            } else {
                if game.add_score(SHIELD_FULL_POINTS) {
                    sfx.write(Sfx::ExtraLife);
                }
                fx.write(Fx::new(pos.0, C_CYAN, FxKind::Score(SHIELD_FULL_POINTS)));
                sfx.write(Sfx::Blip);
            }
            continue;
        }

        let grounded = if game.phase == Phase::Planet {
            pos.0.y <= terrain::height(pos.0.x) + 6.0
        } else {
            pos.0.y <= VOID_FLOOR || s.t > SHIELD_LIFE_VOID
        };
        if grounded {
            commands.entity(e).try_despawn();
            fx.write(Fx::new(pos.0, C_CYAN_DIM, FxKind::Spark));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    const X: f32 = 1000.0;

    fn world(player: Vec2) -> World {
        let mut w = World::new();
        w.insert_resource(State::new(GameState::Playing));
        w.init_resource::<Time>();
        w.init_resource::<Game>();
        w.init_resource::<Messages<Fx>>();
        w.init_resource::<Messages<Sfx>>();
        w.insert_resource(PlayerState {
            pos: player,
            alive: true,
            ..default()
        });
        w
    }

    fn shield(w: &mut World, pos: Vec2) -> Entity {
        w.spawn((
            ShieldPickup { t: 0.0, sway: 0.0 },
            Pos(pos),
            Vel(Vec2::ZERO),
            Transform::default(),
        ))
        .id()
    }

    fn step(w: &mut World) {
        w.run_system_once(update_pickups).unwrap();
    }

    #[test]
    fn shield_is_caught_by_the_ship() {
        let mut w = world(Vec2::new(X, 100.0));
        let s = shield(&mut w, Vec2::new(X + 5.0, 105.0));
        step(&mut w);
        assert!(w.get_entity(s).is_err());
        assert_eq!(w.resource::<Game>().shields, 1);
    }

    #[test]
    fn a_full_ship_scores_instead() {
        let mut w = world(Vec2::new(X, 100.0));
        w.resource_mut::<Game>().shields = SHIELD_MAX;
        shield(&mut w, Vec2::new(X, 100.0));
        step(&mut w);
        assert_eq!(w.resource::<Game>().shields, SHIELD_MAX);
        assert_eq!(w.resource::<Game>().score, SHIELD_FULL_POINTS);
    }

    #[test]
    fn shield_that_reaches_the_ground_is_lost() {
        let mut w = world(Vec2::new(X + 2000.0, 100.0));
        let ground = terrain::height(X);
        let low = shield(&mut w, Vec2::new(X, ground + 1.0));
        let high = shield(&mut w, Vec2::new(X, ground + 100.0));
        step(&mut w);
        assert!(w.get_entity(low).is_err());
        assert!(w.get_entity(high).is_ok());
        assert_eq!(w.resource::<Game>().shields, 0);
    }

    #[test]
    fn hives_and_elites_drop_more_often() {
        assert!(drop_chance(Kind::Hive, false) > drop_chance(Kind::Harvester, false));
        assert_eq!(
            drop_chance(Kind::Harvester, true),
            2.0 * drop_chance(Kind::Harvester, false)
        );
    }
}
