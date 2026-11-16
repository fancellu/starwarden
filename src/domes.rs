//! Colony domes: shelters on the surface. Colonists set down beside one go inside, where
//! Harvesters cannot reach them, and score more. A full dome fires its turret at nearby
//! Harvesters. Everyone comes back out when the next wave begins.

use bevy::prelude::*;

use crate::colonists::{CState, Colonist};
use crate::consts::*;
use crate::enemies::{Enemy, Kind};
use crate::fx::Glow;
use crate::meshes::Art;
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Component, Default)]
pub struct Dome {
    /// Colonists sheltering inside.
    pub housed: u32,
    pub fire_cd: f32,
}

impl Dome {
    pub fn full(&self) -> bool {
        self.housed >= DOME_CAPACITY
    }
}

#[derive(Component)]
pub struct TurretShot {
    pub life: f32,
}

/// Height of the turret's muzzle above the dome's base.
const MUZZLE: f32 = 43.0;

pub fn dome_x(i: usize) -> f32 {
    (i as f32 + 0.5) / DOMES as f32 * WORLD_W
}

pub fn spawn_domes(commands: &mut Commands, art: &Art) {
    for i in 0..DOMES {
        let x = dome_x(i);
        let y = terrain::height(x);
        commands.spawn((
            Dome::default(),
            GameEntity,
            Pos(Vec2::new(x, y)),
            Mesh2d(art.dome[0].clone()),
            MeshMaterial2d(art.mat.clone()),
            Transform::from_xyz(0.0, y, Z_COLONIST - 1.0),
        ));
    }
}

/// Where a turret at `from` should shoot: the offset to where the nearest Harvester in range
/// will be when the shot arrives. Each Harvester is `(position, velocity, risky)`; `risky`
/// marks one carrying a colonist too high to survive the fall, which is left alone.
pub fn turret_target(
    from: Vec2,
    harvesters: impl Iterator<Item = (Vec2, Vec2, bool)>,
) -> Option<Vec2> {
    harvesters
        .filter(|(_, _, risky)| !risky)
        .map(|(p, v, _)| (delta(from, p), v))
        .filter(|(d, _)| d.length() < DOME_RANGE)
        .min_by(|a, b| a.0.length().total_cmp(&b.0.length()))
        .map(|(d, v)| {
            // Two rounds of "how long will the shot take to get there?" is close enough.
            let mut aim = d;
            for _ in 0..2 {
                aim = d + v * (aim.length() / TURRET_SHOT_SPEED);
            }
            aim
        })
}

/// Full domes shoot at nearby Harvesters; every dome shows how many are inside.
pub fn dome_turrets(
    time: Res<Time>,
    mut commands: Commands,
    art: Res<Art>,
    mut domes: Query<(&Pos, &mut Dome, &mut Mesh2d)>,
    game: Res<Game>,
    enemies: Query<(Entity, &Pos, &Vel, &Enemy)>,
    colonists: Query<&Colonist>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    // Enemies move slower during a time warp; the turret's shots do not.
    let warp = game.enemy_time_scale();
    for (pos, mut dome, mut mesh) in &mut domes {
        let want = &art.dome[dome.housed.min(DOME_CAPACITY) as usize];
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
        dome.fire_cd = (dome.fire_cd - dt).max(0.0);
        if !dome.full() || dome.fire_cd > 0.0 {
            continue;
        }
        let muzzle = pos.0 + Vec2::new(0.0, MUZZLE);
        let harvesters = enemies
            .iter()
            .filter(|(.., en)| en.kind == Kind::Harvester && en.warp <= 0.0)
            .map(|(e, epos, vel, _)| {
                let carrying = colonists.iter().any(|c| c.state == CState::Abducted(e));
                let drop = epos.0.y - 24.0 - (terrain::height(epos.0.x) + 9.0);
                (epos.0, vel.0 * warp, carrying && drop > SAFE_FALL)
            });
        if let Some(aim) = turret_target(muzzle, harvesters) {
            dome.fire_cd = DOME_FIRE_EVERY;
            commands.spawn((
                TurretShot {
                    life: DOME_RANGE / TURRET_SHOT_SPEED * 1.4,
                },
                GameEntity,
                Pos(muzzle),
                Vel(aim.normalize_or(Vec2::Y) * TURRET_SHOT_SPEED),
                Glow {
                    color: C_AMBER,
                    size: Vec2::splat(16.0),
                    soft: true,
                },
            ));
            sfx.write(Sfx::Turret);
        }
    }
}

pub fn update_turret_shots(
    time: Res<Time>,
    mut commands: Commands,
    mut shots: Query<(Entity, &mut TurretShot, &Pos)>,
    enemies: Query<(Entity, &Pos, &Enemy)>,
    mut dmg: MessageWriter<Damage>,
    mut fx: MessageWriter<Fx>,
) {
    let dt = time.delta_secs();
    for (e, mut shot, pos) in &mut shots {
        shot.life -= dt;
        let hit = enemies.iter().find(|(_, epos, en)| {
            en.warp <= 0.0 && delta(pos.0, epos.0).length() < en.radius + 6.0
        });
        if let Some((ee, epos, _)) = hit {
            dmg.write(Damage {
                entity: ee,
                amount: 1,
            });
            fx.write(Fx::new(epos.0, C_AMBER, FxKind::Spark));
        }
        if hit.is_some() || shot.life <= 0.0 {
            commands.entity(e).try_despawn();
        }
    }
}

/// When the wave number changes, everyone sheltering walks back out and the domes are empty.
pub fn empty_domes(
    game: Res<Game>,
    mut last_wave: Local<u32>,
    mut domes: Query<(&Pos, &mut Dome), Without<Colonist>>,
    mut colonists: Query<(&mut Colonist, &mut Pos)>,
) {
    if *last_wave == game.wave {
        return;
    }
    *last_wave = game.wave;
    let mut out = 0;
    for (mut c, mut pos) in &mut colonists {
        let CState::Sheltered(d) = c.state else {
            continue;
        };
        c.state = CState::Walking;
        if let Ok((dpos, _)) = domes.get(d) {
            // Alternate sides of the dome.
            let side = if out % 2 == 0 { -1.0 } else { 1.0 };
            let x = wrap(dpos.0.x + side * (40.0 + 6.0 * out as f32));
            pos.0 = Vec2::new(x, terrain::height(x) + 9.0);
            c.dir = side;
            out += 1;
        }
    }
    for (_, mut dome) in &mut domes {
        dome.housed = 0;
    }
}

/// The domes are destroyed with the planet (`run_waves` builds new ones when it is restored).
pub fn sync_domes_with_planet(
    mut commands: Commands,
    game: Res<Game>,
    domes: Query<(Entity, &Pos), With<Dome>>,
    mut fx: MessageWriter<Fx>,
) {
    if game.phase != Phase::Void {
        return;
    }
    for (e, pos) in &domes {
        commands.entity(e).try_despawn();
        fx.write(Fx::new(
            pos.0 + Vec2::new(0.0, 15.0),
            C_CYAN,
            FxKind::Explosion,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    const X: f32 = 1600.0;

    fn world() -> World {
        let mut w = World::new();
        w.init_resource::<Time>();
        w.init_resource::<Game>();
        w.init_resource::<Messages<Fx>>();
        w.init_resource::<Messages<Sfx>>();
        w.init_resource::<Messages<Damage>>();
        w
    }

    fn dome(w: &mut World, housed: u32) -> Entity {
        w.spawn((
            Dome {
                housed,
                fire_cd: 0.0,
            },
            Pos(Vec2::new(X, terrain::height(X))),
        ))
        .id()
    }

    fn sheltered(w: &mut World, d: Entity) -> Entity {
        w.spawn((
            Colonist {
                state: CState::Sheltered(d),
                claimed_by: None,
                dir: 1.0,
                t: 0.0,
                fall_from: 0.0,
                vy: 0.0,
            },
            Pos(Vec2::new(X, 0.0)),
        ))
        .id()
    }

    #[test]
    fn domes_are_spread_around_the_world_with_one_at_the_start() {
        assert_eq!(dome_x(DOMES / 2), WORLD_W * 0.5);
        assert!(dome_x(1) - dome_x(0) > 4.0 * DOME_RANGE);
    }

    #[test]
    fn turret_picks_the_nearest_safe_harvester_in_range() {
        let from = Vec2::new(X, -200.0);
        let near = Vec2::new(X + 100.0, 0.0);
        let far = Vec2::new(X - 300.0, 0.0);
        let away = Vec2::new(X + DOME_RANGE + 50.0, -200.0);
        let still = Vec2::ZERO;
        assert_eq!(
            turret_target(from, [(away, still, false)].into_iter()),
            None
        );
        assert_eq!(
            turret_target(
                from,
                [(far, still, false), (near, still, false)].into_iter()
            ),
            Some(near - from)
        );
        // One carrying a colonist too high up is left alone.
        assert_eq!(
            turret_target(from, [(far, still, false), (near, still, true)].into_iter()),
            Some(far - from)
        );
        assert_eq!(turret_target(from, [(near, still, true)].into_iter()), None);
    }

    #[test]
    fn turret_leads_a_moving_harvester() {
        let from = Vec2::new(X, -200.0);
        let start = Vec2::new(X, 100.0);
        let vel = Vec2::new(120.0, 0.0);
        let aim = turret_target(from, [(start, vel, false)].into_iter()).unwrap();
        // The shot and the Harvester should be in the same place when the shot gets there.
        let t = aim.length() / TURRET_SHOT_SPEED;
        let miss = (from + aim) - (start + vel * t);
        assert!(aim.x > 0.0 && miss.length() < 3.0, "miss by {miss}");
    }

    #[test]
    fn a_new_wave_sends_everyone_back_outside() {
        let mut w = world();
        let d = dome(&mut w, 2);
        let a = sheltered(&mut w, d);
        let b = sheltered(&mut w, d);
        let sys = w.register_system(empty_domes);
        // The game starts on wave 1; the system's first run notes it and lets everyone out.
        w.run_system(sys).unwrap();
        w.get_mut::<Dome>(d).unwrap().housed = 2;
        for e in [a, b] {
            w.get_mut::<Colonist>(e).unwrap().state = CState::Sheltered(d);
        }
        w.run_system(sys).unwrap();
        assert_eq!(
            w.get::<Dome>(d).unwrap().housed,
            2,
            "same wave: still inside"
        );

        w.resource_mut::<Game>().wave += 1;
        w.run_system(sys).unwrap();
        assert_eq!(w.get::<Dome>(d).unwrap().housed, 0);
        for e in [a, b] {
            assert_eq!(w.get::<Colonist>(e).unwrap().state, CState::Walking);
        }
        let (ax, bx) = (w.get::<Pos>(a).unwrap().0.x, w.get::<Pos>(b).unwrap().0.x);
        assert!(ax < X && bx > X, "one each side of the dome");
    }

    #[test]
    fn a_turret_shot_damages_the_enemy_it_touches() {
        let mut w = world();
        let h = w
            .spawn((Enemy::test(Kind::Harvester), Pos(Vec2::new(X, 0.0))))
            .id();
        let hit = w
            .spawn((TurretShot { life: 1.0 }, Pos(Vec2::new(X + 5.0, 3.0))))
            .id();
        let miss = w
            .spawn((TurretShot { life: 1.0 }, Pos(Vec2::new(X + 200.0, 0.0))))
            .id();
        w.run_system_once(update_turret_shots).unwrap();
        assert!(w.get_entity(hit).is_err());
        assert!(w.get_entity(miss).is_ok());
        let got: Vec<Damage> = w.resource_mut::<Messages<Damage>>().drain().collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].entity, h);
    }

    #[test]
    fn domes_go_with_the_planet() {
        let mut w = world();
        let d = dome(&mut w, 0);
        w.run_system_once(sync_domes_with_planet).unwrap();
        assert!(w.get_entity(d).is_ok());
        w.resource_mut::<Game>().phase = Phase::Void;
        w.run_system_once(sync_domes_with_planet).unwrap();
        assert!(w.get_entity(d).is_err());
    }
}
