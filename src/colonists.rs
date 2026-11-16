//! The colonists the player is protecting: walking, being abducted, falling and being rescued.

use bevy::prelude::*;

use crate::consts::*;
use crate::enemies::Enemy;
use crate::meshes::Art;
use crate::player::PlayerState;
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CState {
    Walking,
    /// Held by this Harvester.
    Abducted(Entity),
    Falling,
    /// Caught by the player and waiting to be set down.
    Carried,
}

#[derive(Component)]
pub struct Colonist {
    pub state: CState,
    /// Harvester that has picked this colonist as its target.
    pub claimed_by: Option<Entity>,
    pub dir: f32,
    pub t: f32,
    /// Altitude the current fall started from.
    pub fall_from: f32,
    pub vy: f32,
}

pub fn spawn_colonists(commands: &mut Commands, art: &Art, rng: &mut Rng) {
    for i in 0..COLONISTS {
        let x = (i as f32 + rng.range(0.2, 0.8)) / COLONISTS as f32 * WORLD_W;
        commands.spawn((
            Colonist {
                state: CState::Walking,
                claimed_by: None,
                dir: rng.sign(),
                t: rng.range(0.0, 10.0),
                fall_from: 0.0,
                vy: 0.0,
            },
            GameEntity,
            Pos(Vec2::new(x, terrain::height(x) + 9.0)),
            Mesh2d(art.colonist.clone()),
            MeshMaterial2d(art.mat.clone()),
            Transform::from_xyz(0.0, 0.0, Z_COLONIST),
        ));
    }
}

#[allow(clippy::type_complexity)]
pub fn update_colonists(
    time: Res<Time>,
    mut commands: Commands,
    mut ps: ResMut<PlayerState>,
    mut game: ResMut<Game>,
    mut rng: ResMut<Rng>,
    mut q: Query<(Entity, &mut Colonist, &mut Pos, &mut Transform), Without<Enemy>>,
    enemies: Query<&Pos, With<Enemy>>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    // Everyone carried is set down together when the ship flies low, and each is worth more
    // the more are set down at once.
    let aboard = q
        .iter()
        .filter(|(_, c, _, _)| c.state == CState::Carried)
        .count() as u32;
    let ship_ground = terrain::height(ps.pos.x) + 9.0;
    let set_down = ps.alive && aboard > 0 && ps.pos.y - 19.0 <= ship_ground + 14.0;
    let set_down_each = SET_DOWN_POINTS * aboard.min(CARRY_BONUS_MAX);
    let mut landed = 0;
    let mut carried = 0;
    let magnet = ps.alive && game.power_on(Power::Magnet);
    let ship = ps.pos;
    let in_reach = |p: Vec2| magnet && delta(p, ship).length() < MAGNET_REACH;
    for (e, mut c, mut pos, mut tf) in &mut q {
        c.t += dt;
        if c.claimed_by.is_some_and(|h| enemies.get(h).is_err()) {
            c.claimed_by = None;
        }
        let ground = terrain::height(pos.0.x) + 9.0;
        let mut tilt = 0.0;

        match c.state {
            CState::Walking => {
                if rng.f32() < dt * 0.25 {
                    c.dir = -c.dir;
                }
                pos.0.x = wrap(pos.0.x + c.dir * 11.0 * dt);
                pos.0.y = terrain::height(pos.0.x) + 9.0 + (c.t * 9.0).sin().abs() * 1.5;
                tf.scale.x = c.dir;
            }
            CState::Abducted(h) => match enemies.get(h) {
                Ok(hpos) if !in_reach(pos.0) => {
                    pos.0 = hpos.0 + Vec2::new(0.0, -24.0);
                    tilt = (c.t * 14.0).sin() * 0.25;
                }
                Ok(_) => {
                    // Snatched from the Harvester by the magnet; it then pulls them in.
                    c.state = CState::Falling;
                    c.claimed_by = None;
                    c.fall_from = pos.0.y;
                    c.vy = 0.0;
                    fx.write(Fx::new(pos.0, C_RED, FxKind::Rescue));
                    sfx.write(Sfx::Rescue);
                }
                Err(_) => {
                    // The captor was destroyed: start falling.
                    c.state = CState::Falling;
                    c.claimed_by = None;
                    c.fall_from = pos.0.y;
                    c.vy = 0.0;
                }
            },
            CState::Falling => {
                if in_reach(pos.0) {
                    // Pulled straight at the ship; if the magnet runs out, the fall starts
                    // again from here.
                    let to_ship = delta(pos.0, ps.pos);
                    let step =
                        to_ship.normalize_or_zero() * (MAGNET_PULL * dt).min(to_ship.length());
                    pos.0 = Vec2::new(wrap(pos.0.x + step.x), pos.0.y + step.y);
                    c.vy = 0.0;
                    c.fall_from = pos.0.y;
                } else {
                    c.vy = (c.vy - 420.0 * dt).max(-FALL_SPEED);
                    pos.0.y += c.vy * dt;
                }
                tilt = c.t * 5.0;
                if ps.alive && delta(pos.0, ps.pos).length() < 30.0 {
                    c.state = CState::Carried;
                    if game.add_score(500) {
                        sfx.write(Sfx::ExtraLife);
                    }
                    fx.write(Fx::new(pos.0, C_GREEN, FxKind::Rescue));
                    fx.write(Fx::new(pos.0, C_GREEN, FxKind::Score(500)));
                    sfx.write(Sfx::Rescue);
                } else if pos.0.y <= ground {
                    pos.0.y = ground;
                    if c.fall_from - ground <= SAFE_FALL {
                        c.state = CState::Walking;
                        if game.add_score(250) {
                            sfx.write(Sfx::ExtraLife);
                        }
                        fx.write(Fx::new(pos.0, C_GREEN, FxKind::Score(250)));
                        sfx.write(Sfx::Landed);
                    } else {
                        commands.entity(e).try_despawn();
                        fx.write(Fx::new(pos.0, C_PEACH, FxKind::Explosion));
                        sfx.write(Sfx::ColonistLost);
                    }
                }
            }
            CState::Carried => {
                if !ps.alive {
                    // Dropped when the ship is destroyed.
                    c.state = CState::Falling;
                    c.fall_from = pos.0.y;
                    c.vy = 0.0;
                } else if set_down {
                    pos.0 = Vec2::new(wrap(ps.pos.x + landed as f32 * 8.0), 0.0);
                    pos.0.y = terrain::height(pos.0.x) + 9.0;
                    c.state = CState::Walking;
                    landed += 1;
                    fx.write(Fx::new(pos.0, C_GREEN, FxKind::Rescue));
                } else {
                    pos.0 = Vec2::new(wrap(ps.pos.x + carried as f32 * 8.0), ps.pos.y - 19.0);
                    carried += 1;
                }
            }
        }
        if c.state != CState::Walking {
            tf.scale.x = 1.0;
        }
        tf.rotation = Quat::from_rotation_z(tilt);
    }
    if landed > 0 {
        let pts = set_down_each * landed;
        if game.add_score(pts) {
            sfx.write(Sfx::ExtraLife);
        }
        fx.write(Fx::new(
            Vec2::new(ps.pos.x, ship_ground),
            C_GREEN,
            FxKind::Score(pts),
        ));
        sfx.write(Sfx::Landed);
    }
    ps.carried = carried;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    const X: f32 = 1000.0;

    fn world(player: Vec2) -> World {
        let mut w = World::new();
        w.init_resource::<Time>();
        w.init_resource::<Game>();
        w.init_resource::<Rng>();
        w.init_resource::<Messages<Fx>>();
        w.init_resource::<Messages<Sfx>>();
        w.insert_resource(PlayerState {
            pos: player,
            alive: true,
            ..default()
        });
        w
    }

    fn colonist(w: &mut World, state: CState, y: f32, fall_from: f32) -> Entity {
        w.spawn((
            Colonist {
                state,
                claimed_by: None,
                dir: 1.0,
                t: 0.0,
                fall_from,
                vy: 0.0,
            },
            Pos(Vec2::new(X, y)),
            Transform::default(),
        ))
        .id()
    }

    fn state(w: &World, e: Entity) -> Option<CState> {
        w.get::<Colonist>(e).map(|c| c.state)
    }

    fn step(w: &mut World) {
        w.run_system_once(update_colonists).unwrap();
    }

    #[test]
    fn falling_colonist_is_caught_then_set_down() {
        let ground = terrain::height(X);
        let mut w = world(Vec2::new(X, 100.0));
        let e = colonist(&mut w, CState::Falling, 90.0, 200.0);
        step(&mut w);
        assert_eq!(state(&w, e), Some(CState::Carried));
        assert_eq!(w.resource::<Game>().score, 500);

        // Still carried while the ship is high up.
        step(&mut w);
        assert_eq!(state(&w, e), Some(CState::Carried));

        // Flying low sets the colonist down.
        w.resource_mut::<PlayerState>().pos = Vec2::new(X, ground + 20.0);
        step(&mut w);
        assert_eq!(state(&w, e), Some(CState::Walking));
        assert_eq!(w.resource::<Game>().score, 1000);
    }

    #[test]
    fn short_fall_is_survived_long_fall_is_not() {
        let ground = terrain::height(X) + 9.0;
        let mut w = world(Vec2::new(X + 2000.0, 200.0));
        let low = colonist(
            &mut w,
            CState::Falling,
            ground - 1.0,
            ground + SAFE_FALL - 10.0,
        );
        let high = colonist(
            &mut w,
            CState::Falling,
            ground - 1.0,
            ground + SAFE_FALL + 60.0,
        );
        step(&mut w);
        assert_eq!(state(&w, low), Some(CState::Walking));
        assert_eq!(state(&w, high), None, "a long fall is fatal");
        assert_eq!(w.resource::<Game>().score, 250);
    }

    #[test]
    fn colonist_falls_when_its_captor_is_gone() {
        let mut w = world(Vec2::new(X + 2000.0, 200.0));
        let captor = w.spawn_empty().id();
        w.despawn(captor);
        let e = colonist(&mut w, CState::Abducted(captor), 120.0, 0.0);
        step(&mut w);
        assert_eq!(state(&w, e), Some(CState::Falling));
        assert_eq!(w.get::<Colonist>(e).unwrap().fall_from, 120.0);
    }

    #[test]
    fn a_magnet_snatches_a_colonist_and_pulls_them_in_but_leaves_walkers() {
        let mut w = world(Vec2::new(X, 400.0));
        w.resource_mut::<Game>().grant_power(Power::Magnet);
        w.insert_resource(Time::<()>::default());
        let harvester = Enemy {
            kind: crate::enemies::Kind::Harvester,
            elite: false,
            hp: 1,
            radius: 20.0,
            warp: 0.0,
            t: 0.0,
            fire_cd: 9.0,
            ai: crate::enemies::Ai::Rise,
            target: None,
            aim: Vec2::ZERO,
            timer: 0.0,
            hit: 0.0,
        };
        let captor = w.spawn((harvester, Pos(Vec2::new(X, 224.0)))).id();
        let held = colonist(&mut w, CState::Abducted(captor), 200.0, 0.0);
        let walker = colonist(&mut w, CState::Walking, terrain::height(X) + 9.0, 0.0);
        step(&mut w);
        assert_eq!(state(&w, held), Some(CState::Falling), "snatched");
        assert_eq!(state(&w, walker), Some(CState::Walking));

        // Pulled up to the ship and caught, despite gravity.
        let mut time = Time::<()>::default();
        for _ in 0..60 {
            time.advance_by(std::time::Duration::from_millis(16));
            w.insert_resource(time);
            step(&mut w);
        }
        assert_eq!(state(&w, held), Some(CState::Carried));
        assert_eq!(state(&w, walker), Some(CState::Walking));
    }

    #[test]
    fn colonists_set_down_together_are_worth_more() {
        let ground = terrain::height(X);
        let mut w = world(Vec2::new(X, ground + 200.0));
        let a = colonist(&mut w, CState::Carried, 80.0, 0.0);
        let b = colonist(&mut w, CState::Carried, 80.0, 0.0);
        step(&mut w);
        assert_eq!(w.resource::<PlayerState>().carried, 2);
        assert_eq!(w.resource::<Game>().score, 0);

        w.resource_mut::<PlayerState>().pos = Vec2::new(X, ground + 20.0);
        step(&mut w);
        assert_eq!(state(&w, a), Some(CState::Walking));
        assert_eq!(state(&w, b), Some(CState::Walking));
        assert_eq!(w.resource::<Game>().score, 2 * 2 * SET_DOWN_POINTS);
        assert_eq!(w.resource::<PlayerState>().carried, 0);
    }

    #[test]
    fn set_down_bonus_is_capped() {
        let ground = terrain::height(X);
        let mut w = world(Vec2::new(X, ground + 20.0));
        let n = CARRY_BONUS_MAX + 2;
        for _ in 0..n {
            colonist(&mut w, CState::Carried, 80.0, 0.0);
        }
        step(&mut w);
        assert_eq!(
            w.resource::<Game>().score,
            n * CARRY_BONUS_MAX * SET_DOWN_POINTS
        );
    }

    #[test]
    fn carrying_slows_the_ship_down_to_a_limit() {
        use crate::player::carry_speed;
        assert_eq!(carry_speed(0), 1.0);
        assert!((carry_speed(2) - (1.0 - 2.0 * CARRY_SLOW)).abs() < 1e-6);
        assert_eq!(carry_speed(100), CARRY_MIN_SPEED);
    }

    #[test]
    fn carried_colonist_is_dropped_when_the_ship_dies() {
        let mut w = world(Vec2::new(X, 100.0));
        let e = colonist(&mut w, CState::Carried, 80.0, 0.0);
        w.resource_mut::<PlayerState>().alive = false;
        step(&mut w);
        assert_eq!(state(&w, e), Some(CState::Falling));
    }
}
