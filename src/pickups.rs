//! Pickups dropped by destroyed enemies: they drift down and must be caught before they reach
//! the ground. A shield absorbs a hit that would otherwise destroy the ship; a powerup gives the
//! ship a timed boost.

use bevy::prelude::*;

use crate::consts::*;
use crate::enemies::Kind;
use crate::fx::Glow;
use crate::meshes::Art;
use crate::player::PlayerState;
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickupKind {
    Shield,
    Power(Power),
}

impl PickupKind {
    pub fn color(self) -> LinearRgba {
        match self {
            PickupKind::Shield => C_CYAN,
            PickupKind::Power(p) => p.color(),
        }
    }
}

#[derive(Component)]
pub struct Pickup {
    pub kind: PickupKind,
    /// Seconds since it was dropped.
    pub t: f32,
    /// Phase of its sideways sway, so neighbours do not move in step.
    pub sway: f32,
}

fn chance(kind: Kind, elite: bool, base: f32, hive: f32) -> f32 {
    let c = if kind == Kind::Hive { hive } else { base };
    if elite {
        c * 2.0
    } else {
        c
    }
}

/// Chance that a destroyed enemy of this kind drops a shield.
pub fn drop_chance(kind: Kind, elite: bool) -> f32 {
    chance(kind, elite, SHIELD_DROP, SHIELD_DROP_HIVE)
}

/// Chance that a destroyed enemy of this kind drops a powerup.
pub fn power_drop_chance(kind: Kind, elite: bool) -> f32 {
    chance(kind, elite, POWER_DROP, POWER_DROP_HIVE)
}

fn spawn_pickup(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    art: &Art,
    rng: &mut Rng,
    pos: Vec2,
    kind: PickupKind,
) {
    commands.spawn((
        Pickup {
            kind,
            t: 0.0,
            sway: rng.range(0.0, 6.0),
        },
        GameEntity,
        Pos(pos),
        Vel(Vec2::new(0.0, -SHIELD_FALL)),
        Glow {
            color: with_alpha(kind.color(), 0.35),
            size: Vec2::splat(46.0),
            soft: true,
        },
        Mesh2d(mesh),
        MeshMaterial2d(art.mat.clone()),
        Transform::from_xyz(0.0, pos.y, Z_SHOT),
    ));
}

pub fn spawn_shield(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2) {
    spawn_pickup(
        commands,
        art.shield.clone(),
        art,
        rng,
        pos,
        PickupKind::Shield,
    );
}

/// Drops a powerup of a random kind.
pub fn spawn_power(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2) {
    let p = Power::ALL[rng.index(Power::COUNT)];
    spawn_power_of(commands, art, rng, pos, p);
}

pub fn spawn_power_of(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2, p: Power) {
    let mesh = art.power[p as usize].clone();
    spawn_pickup(commands, mesh, art, rng, pos, PickupKind::Power(p));
}

/// Moves falling pickups, lets the ship catch them and removes those that reach the ground.
pub fn update_pickups(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut commands: Commands,
    ps: Res<PlayerState>,
    mut game: ResMut<Game>,
    mut q: Query<(Entity, &mut Pickup, &Pos, &mut Vel, &mut Transform)>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    let playing = *state.get() == GameState::Playing;
    let magnet = playing && ps.alive && game.power_on(Power::Magnet);
    for (e, mut s, pos, mut vel, mut tf) in &mut q {
        s.t += dt;
        let to_ship = delta(pos.0, ps.pos);
        if magnet && to_ship.length() < MAGNET_REACH {
            vel.0 = to_ship.normalize_or_zero() * MAGNET_PULL;
        } else {
            vel.0 = Vec2::new((s.t * 1.7 + s.sway).sin() * 22.0, -SHIELD_FALL);
        }
        // Shields spin; powerups only rock, so their symbols stay readable.
        let angle = match s.kind {
            PickupKind::Shield => s.t * 1.3,
            PickupKind::Power(_) => (s.t * 2.2 + s.sway).sin() * 0.3,
        };
        tf.rotation = Quat::from_rotation_z(angle);
        let color = s.kind.color();

        if playing && ps.alive && delta(ps.pos, pos.0).length() < SHIELD_REACH {
            commands.entity(e).try_despawn();
            fx.write(Fx::new(pos.0, color, FxKind::Rescue));
            match s.kind {
                PickupKind::Shield => {
                    if game.add_shield() {
                        sfx.write(Sfx::ShieldUp);
                    } else {
                        if game.add_score(SHIELD_FULL_POINTS) {
                            sfx.write(Sfx::ExtraLife);
                        }
                        fx.write(Fx::new(pos.0, color, FxKind::Score(SHIELD_FULL_POINTS)));
                        sfx.write(Sfx::Blip);
                    }
                }
                PickupKind::Power(p) => {
                    game.grant_power(p);
                    fx.write(Fx::new(pos.0, color, FxKind::Label(p.name())));
                    sfx.write(Sfx::PowerUp);
                }
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
            fx.write(Fx::new(pos.0, with_alpha(color * 0.4, 1.0), FxKind::Spark));
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

    fn pickup(w: &mut World, pos: Vec2, kind: PickupKind) -> Entity {
        w.spawn((
            Pickup {
                kind,
                t: 0.0,
                sway: 0.0,
            },
            Pos(pos),
            Vel(Vec2::ZERO),
            Transform::default(),
        ))
        .id()
    }

    fn shield(w: &mut World, pos: Vec2) -> Entity {
        pickup(w, pos, PickupKind::Shield)
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
    fn catching_a_powerup_starts_it() {
        let mut w = world(Vec2::new(X, 100.0));
        let p = pickup(
            &mut w,
            Vec2::new(X, 100.0),
            PickupKind::Power(Power::WideBeam),
        );
        step(&mut w);
        assert!(w.get_entity(p).is_err());
        let g = w.resource::<Game>();
        assert!(g.power_on(Power::WideBeam));
        assert!(!g.power_on(Power::Speed));
        assert_eq!(g.shields, 0);
    }

    #[test]
    fn a_magnet_pulls_pickups_in() {
        let mut w = world(Vec2::new(X, 300.0));
        let near = shield(&mut w, Vec2::new(X + 200.0, 300.0));
        let far = shield(&mut w, Vec2::new(X + MAGNET_REACH + 50.0, 300.0));
        step(&mut w);
        assert_eq!(
            w.get::<Vel>(near).unwrap().0.y,
            -SHIELD_FALL,
            "no magnet, no pull"
        );
        w.resource_mut::<Game>().grant_power(Power::Magnet);
        step(&mut w);
        let v = w.get::<Vel>(near).unwrap().0;
        assert!((v - Vec2::new(-MAGNET_PULL, 0.0)).length() < 0.01, "{v}");
        assert_eq!(w.get::<Vel>(far).unwrap().0.y, -SHIELD_FALL, "out of reach");
    }

    #[test]
    fn hives_and_elites_drop_more_often() {
        assert!(drop_chance(Kind::Hive, false) > drop_chance(Kind::Harvester, false));
        assert_eq!(
            drop_chance(Kind::Harvester, true),
            2.0 * drop_chance(Kind::Harvester, false)
        );
        assert!(power_drop_chance(Kind::Hive, false) > power_drop_chance(Kind::Wraith, false));
    }
}
