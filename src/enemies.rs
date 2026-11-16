//! Enemy types, their behaviour, their shots and what happens when they die.

use bevy::prelude::*;

use crate::colonists::{CState, Colonist};
use crate::consts::*;
use crate::fx::Glow;
use crate::meshes::Art;
use crate::pickups;
use crate::player::{Player, PlayerState};
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Descends, grabs a colonist and carries them off.
    Harvester,
    /// What a Harvester becomes when it escapes with a colonist: fast and erratic.
    Wraith,
    /// Slow pod that bursts into Stingers.
    Hive,
    /// Small, fast swarm unit.
    Stinger,
    /// Hunter that arrives when a wave drags on.
    Stalker,
}

impl Kind {
    pub fn radius(self) -> f32 {
        match self {
            Kind::Harvester => 15.0,
            Kind::Wraith => 14.0,
            Kind::Hive => 19.0,
            Kind::Stinger => 8.0,
            Kind::Stalker => 13.0,
        }
    }
    pub fn score(self) -> u32 {
        match self {
            Kind::Harvester => 150,
            Kind::Wraith => 200,
            Kind::Hive => 500,
            Kind::Stinger => 100,
            Kind::Stalker => 300,
        }
    }
    fn hp(self) -> i32 {
        match self {
            Kind::Hive => 3,
            Kind::Stalker => 2,
            _ => 1,
        }
    }
    pub fn color(self, elite: bool) -> LinearRgba {
        if elite {
            return C_RED;
        }
        match self {
            Kind::Harvester => C_GREEN,
            Kind::Wraith => C_MAGENTA,
            Kind::Hive | Kind::Stinger => C_AMBER,
            Kind::Stalker => C_BLUE,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ai {
    Roam,
    Descend,
    Rise,
}

#[derive(Component)]
pub struct Enemy {
    pub kind: Kind,
    pub elite: bool,
    pub hp: i32,
    pub radius: f32,
    /// Remaining warp-in time; the enemy is harmless and invulnerable while it is positive.
    pub warp: f32,
    /// Age in seconds (offset randomly so groups do not move in lockstep).
    pub t: f32,
    pub fire_cd: f32,
    pub ai: Ai,
    /// Colonist a Harvester is after or carrying.
    pub target: Option<Entity>,
    /// Kind-specific vector: drift direction, cruise altitude or pursuit offset.
    pub aim: Vec2,
    pub timer: f32,
    /// Brief scale bump when hit but not destroyed.
    pub hit: f32,
}

#[derive(Component)]
pub struct EShot {
    pub life: f32,
}

pub const WARP_TIME: f32 = 0.7;

pub fn spawn_enemy(
    commands: &mut Commands,
    art: &Art,
    rng: &mut Rng,
    kind: Kind,
    elite: bool,
    pos: Vec2,
    warp: bool,
) -> Entity {
    let i = elite as usize;
    let mesh = match kind {
        Kind::Harvester => &art.harvester[i],
        Kind::Wraith => &art.wraith[i],
        Kind::Hive => &art.hive[i],
        Kind::Stinger => &art.stinger[i],
        Kind::Stalker => &art.stalker[i],
    };
    let aim = match kind {
        Kind::Harvester => Vec2::new(rng.sign(), rng.range(-40.0, 150.0)),
        Kind::Hive => Vec2::new(rng.sign() * rng.range(30.0, 60.0), rng.range(-60.0, 160.0)),
        _ => Vec2::ZERO,
    };
    commands
        .spawn((
            Enemy {
                kind,
                elite,
                hp: kind.hp() + elite as i32,
                radius: kind.radius(),
                warp: if warp { WARP_TIME } else { 0.0 },
                t: rng.range(0.0, 10.0),
                fire_cd: rng.range(1.5, 4.0),
                ai: Ai::Roam,
                target: None,
                aim,
                timer: rng.range(1.0, 3.0),
                hit: 0.0,
            },
            GameEntity,
            Pos(Vec2::new(wrap(pos.x), pos.y)),
            Vel(Vec2::ZERO),
            Mesh2d(mesh.clone()),
            MeshMaterial2d(art.mat.clone()),
            Transform::from_xyz(0.0, pos.y, Z_ENEMY).with_scale(Vec3::splat(if warp {
                0.0
            } else {
                1.0
            })),
        ))
        .id()
}

/// Enemies get faster as the waves go on; elites are faster still.
fn speed_mul(wave: u32, elite: bool) -> f32 {
    (1.0 + 0.05 * wave.saturating_sub(1) as f32).min(1.7) * if elite { 1.3 } else { 1.0 }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn enemy_ai(
    time: Res<Time>,
    mut commands: Commands,
    art: Res<Art>,
    game: Res<Game>,
    ps: Res<PlayerState>,
    mut rng: ResMut<Rng>,
    mut q: Query<(Entity, &mut Enemy, &mut Pos, &mut Vel, &mut Transform)>,
    mut colonists: Query<(Entity, &Pos, &mut Colonist), Without<Enemy>>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs() * game.enemy_time_scale();
    let planet = game.phase == Phase::Planet;

    for (e, mut en, mut pos, mut vel, mut tf) in &mut q {
        en.t += dt;
        en.hit = (en.hit - dt * 6.0).max(0.0);

        // Warping in: grow from nothing, do nothing else.
        if en.warp > 0.0 {
            en.warp -= dt;
            let k = 1.0 - (en.warp / WARP_TIME).clamp(0.0, 1.0);
            tf.scale = Vec3::splat(k * k * (3.0 - 2.0 * k));
            tf.rotation = Quat::from_rotation_z((1.0 - k) * 6.0);
            vel.0 = Vec2::ZERO;
            continue;
        }

        let sm = speed_mul(game.wave, en.elite);
        let to_player = delta(pos.0, ps.pos);
        let mut ceiling = TOP_Y;

        match en.kind {
            Kind::Harvester => {
                let cruise = en.aim.y;
                match en.ai {
                    Ai::Roam => {
                        // Look for an unclaimed colonist once settled in.
                        if planet && en.target.is_none() && en.t > 1.5 {
                            en.timer -= dt;
                            if en.timer <= 0.0 {
                                en.timer = rng.range(0.6, 1.6);
                                let free: Vec<Entity> = colonists
                                    .iter()
                                    .filter(|(_, _, c)| {
                                        c.state == CState::Walking && c.claimed_by.is_none()
                                    })
                                    .map(|(ce, _, _)| ce)
                                    .collect();
                                if !free.is_empty() {
                                    let pick = free[rng.index(free.len())];
                                    if let Ok((_, _, mut c)) = colonists.get_mut(pick) {
                                        c.claimed_by = Some(e);
                                        en.target = Some(pick);
                                    }
                                }
                            }
                        }
                        let target = en.target.and_then(|t| colonists.get(t).ok());
                        match target {
                            Some((_, cpos, c))
                                if c.state == CState::Walking && c.claimed_by == Some(e) =>
                            {
                                let dx = wrap_delta(cpos.0.x - pos.0.x);
                                vel.0.x = (dx * 1.6).clamp(-125.0 * sm, 125.0 * sm);
                                vel.0.y = (cruise - pos.0.y).clamp(-80.0, 80.0);
                                if dx.abs() < 14.0 {
                                    en.ai = Ai::Descend;
                                }
                            }
                            _ => {
                                en.target = None;
                                vel.0.x = en.aim.x * 75.0 * sm;
                                vel.0.y = (cruise - pos.0.y).clamp(-80.0, 80.0)
                                    + (en.t * 1.3).sin() * 45.0;
                            }
                        }
                    }
                    Ai::Descend => {
                        let target = en.target.and_then(|t| colonists.get_mut(t).ok());
                        match target {
                            Some((_, cpos, mut c))
                                if c.state == CState::Walking && c.claimed_by == Some(e) =>
                            {
                                let dx = wrap_delta(cpos.0.x - pos.0.x);
                                vel.0.x = (dx * 4.0).clamp(-90.0, 90.0);
                                vel.0.y = -95.0 * sm;
                                if pos.0.y <= cpos.0.y + 26.0 {
                                    c.state = CState::Abducted(e);
                                    en.ai = Ai::Rise;
                                    sfx.write(Sfx::Alarm);
                                }
                            }
                            _ => {
                                en.target = None;
                                en.ai = Ai::Roam;
                            }
                        }
                    }
                    Ai::Rise => {
                        ceiling = f32::MAX;
                        let held = en
                            .target
                            .and_then(|t| colonists.get(t).ok())
                            .is_some_and(|(_, _, c)| c.state == CState::Abducted(e));
                        if !held {
                            en.target = None;
                            en.ai = Ai::Roam;
                        } else {
                            vel.0 = Vec2::new((en.t * 3.0).sin() * 14.0, 62.0 * sm);
                            if pos.0.y >= TOP_Y - 6.0 {
                                // Escaped: the colonist is lost and the Harvester transforms.
                                if let Some(t) = en.target {
                                    commands.entity(t).try_despawn();
                                }
                                commands.entity(e).try_despawn();
                                en.hp = 0;
                                spawn_enemy(
                                    &mut commands,
                                    &art,
                                    &mut rng,
                                    Kind::Wraith,
                                    en.elite,
                                    pos.0,
                                    false,
                                );
                                fx.write(Fx::new(pos.0, C_MAGENTA, FxKind::Explosion));
                                fx.write(Fx::new(pos.0, C_MAGENTA, FxKind::Rescue));
                                sfx.write(Sfx::Mutate);
                                continue;
                            }
                        }
                    }
                }
                tf.rotation = Quat::from_rotation_z((en.t * 2.0).sin() * 0.08);
            }
            Kind::Wraith => {
                let chase = to_player.normalize_or_zero() * 250.0 * sm;
                let jitter = Vec2::new((en.t * 3.7).sin() * 90.0, (en.t * 6.1).sin() * 170.0);
                let want = chase + jitter;
                let k = smooth(3.0, dt);
                vel.0 = vel.0.lerp(want, k);
                tf.rotation = Quat::from_rotation_z(en.t * 2.4);
            }
            Kind::Hive => {
                vel.0.x = en.aim.x * sm;
                vel.0.y = (en.aim.y - pos.0.y).clamp(-40.0, 40.0) + (en.t * 0.9).sin() * 30.0;
                tf.rotation = Quat::from_rotation_z(en.t * 0.7);
            }
            Kind::Stinger => {
                vel.0.x += to_player.x.signum() * 950.0 * sm * dt;
                vel.0.x = vel.0.x.clamp(-470.0 * sm, 470.0 * sm);
                let vy = (to_player.y * 2.2).clamp(-230.0, 230.0) + (en.t * 9.0).sin() * 130.0;
                vel.0.y += (vy - vel.0.y) * smooth(6.0, dt);
                tf.rotation = Quat::from_rotation_z(vel.0.to_angle());
            }
            Kind::Stalker => {
                en.timer -= dt;
                if en.timer <= 0.0 {
                    en.timer = rng.range(0.8, 1.6);
                    en.aim = Vec2::new(
                        rng.sign() * rng.range(160.0, 360.0),
                        rng.range(-130.0, 130.0),
                    );
                }
                let want = ((to_player + en.aim) * 2.6).clamp_length_max(760.0 * sm);
                let k = smooth(4.0, dt);
                vel.0 = vel.0.lerp(want, k);
                tf.rotation = Quat::from_rotation_z((vel.0.x * 0.0004).clamp(-0.4, 0.4));
            }
        }

        // Keep inside the play area.
        let floor = if planet {
            terrain::height(pos.0.x) + en.radius + 6.0
        } else {
            VOID_FLOOR
        };
        if pos.0.y < floor {
            pos.0.y = floor;
            vel.0.y = vel.0.y.max(0.0);
        }
        if pos.0.y > ceiling {
            pos.0.y = ceiling;
            vel.0.y = vel.0.y.min(0.0);
        }

        // Shooting.
        let shoots = !matches!(en.kind, Kind::Hive | Kind::Stinger) && en.ai != Ai::Descend;
        if shoots && ps.alive {
            en.fire_cd -= dt * if en.elite { 1.6 } else { 1.0 };
            if en.fire_cd <= 0.0 {
                let (lo, hi, speed) = match en.kind {
                    Kind::Stalker => (0.7, 1.3, 420.0),
                    Kind::Wraith => (1.4, 2.6, 340.0),
                    _ => (2.4, 4.6, 290.0),
                };
                en.fire_cd = rng.range(lo, hi);
                if to_player.length() < 760.0 {
                    let speed = speed * sm.min(1.4);
                    // Lead the target a little; Harvesters are sloppier shots.
                    let lead = to_player + ps.vel * (to_player.length() / speed) * 0.6;
                    let spread = if en.kind == Kind::Harvester {
                        0.22
                    } else {
                        0.08
                    };
                    let dir = Vec2::from_angle(rng.range(-spread, spread))
                        .rotate(lead.normalize_or(Vec2::X));
                    commands.spawn((
                        EShot { life: 3.2 },
                        GameEntity,
                        Pos(pos.0),
                        Vel(dir * speed),
                        Glow {
                            color: if en.elite { C_AMBER } else { C_RED },
                            size: Vec2::splat(22.0),
                            soft: true,
                        },
                    ));
                    sfx.write(Sfx::EnemyShot);
                }
            }
        }

        // Idle animation: a gentle pulse, plus a bump when hit.
        let pulse = match en.kind {
            Kind::Wraith => 1.0 + 0.14 * (en.t * 9.0).sin(),
            Kind::Hive => 1.0 + 0.05 * (en.t * 3.0).sin(),
            _ => 1.0,
        };
        tf.scale = Vec3::splat(pulse + en.hit * 0.35);
    }
}

pub fn update_shots(
    time: Res<Time>,
    game: Res<Game>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut EShot, &Pos, &mut Transform)>,
) {
    let dt = time.delta_secs() * game.enemy_time_scale();
    for (e, mut s, pos, mut tf) in &mut q {
        s.life -= dt;
        let grounded = game.phase == Phase::Planet && pos.0.y < terrain::height(pos.0.x);
        if s.life <= 0.0 || grounded || pos.0.y > TOP_Y + 60.0 || pos.0.y < VOID_FLOOR - 40.0 {
            commands.entity(e).try_despawn();
        }
        tf.scale = Vec3::splat(1.0 + 0.25 * (s.life * 30.0).sin());
    }
}

/// Ship against enemies and their shots.
pub fn player_collisions(
    mut commands: Commands,
    game: Res<Game>,
    player: Query<(&Player, &Pos)>,
    enemies: Query<(Entity, &Pos, &Enemy)>,
    shots: Query<(Entity, &Pos), With<EShot>>,
    mut hit: MessageWriter<PlayerHit>,
    mut dmg: MessageWriter<Damage>,
) {
    let Ok((p, pos)) = player.single() else {
        return;
    };
    // While invincible the ship rams enemies and shrugs off shots.
    let invincible = game.power_on(Power::Invincible);
    if p.invuln > 0.0 && !invincible {
        return;
    }
    for (e, epos, en) in &enemies {
        if en.warp <= 0.0 && delta(pos.0, epos.0).length() < en.radius + PLAYER_RADIUS {
            if !invincible {
                hit.write(PlayerHit);
            }
            dmg.write(Damage {
                entity: e,
                amount: 999,
            });
        }
    }
    for (e, spos) in &shots {
        if delta(pos.0, spos.0).length() < PLAYER_RADIUS + 3.0 {
            if !invincible {
                hit.write(PlayerHit);
            }
            commands.entity(e).try_despawn();
        }
    }
}

/// Applies queued damage; destroyed enemies score, explode and (for Hives) release Stingers.
pub fn apply_damage(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut game: ResMut<Game>,
    mut cam: ResMut<GameCam>,
    mut reader: MessageReader<Damage>,
    mut q: Query<(&mut Enemy, &Pos)>,
    falling_pickups: Query<&pickups::Pickup>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    // Counted here as well as queried: pickups spawned this frame are not in the query yet.
    let mut falling = falling_pickups
        .iter()
        .filter(|p| p.kind == pickups::PickupKind::Shield)
        .count();
    let mut falling_powers = falling_pickups.iter().count() - falling;
    for d in reader.read() {
        let Ok((mut en, pos)) = q.get_mut(d.entity) else {
            continue;
        };
        if en.hp <= 0 {
            continue;
        }
        en.hp -= d.amount;
        if en.hp > 0 {
            en.hit = 1.0;
            sfx.write(Sfx::Hit);
            continue;
        }
        commands.entity(d.entity).try_despawn();
        let color = en.kind.color(en.elite);
        fx.write(Fx::new(pos.0, color, FxKind::Explosion));
        if game.register_kill() {
            sfx.write(Sfx::Blip);
        }
        let mult = game.multiplier();
        let pts = en.kind.score() * if en.elite { 2 } else { 1 } * mult;
        if pts >= 300 || mult > 1 {
            fx.write(Fx::new(pos.0, color, FxKind::Score(pts)));
        }
        if game.add_score(pts) {
            sfx.write(Sfx::ExtraLife);
        }
        if falling < SHIELD_FALLING_MAX && rng.f32() < pickups::drop_chance(en.kind, en.elite) {
            pickups::spawn_shield(&mut commands, &art, &mut rng, pos.0);
            falling += 1;
        } else if falling_powers < POWER_FALLING_MAX
            && rng.f32() < pickups::power_drop_chance(en.kind, en.elite)
        {
            pickups::spawn_power(&mut commands, &art, &mut rng, pos.0);
            falling_powers += 1;
        }
        cam.shake = cam.shake.max(0.22);
        sfx.write(if en.kind == Kind::Hive {
            Sfx::ExplodeBig
        } else {
            Sfx::ExplodeSmall
        });
        if en.kind == Kind::Hive {
            let n = if en.elite { 7 } else { 5 };
            for _ in 0..n {
                let off = rng.dir() * rng.range(6.0, 22.0);
                let s = spawn_enemy(
                    &mut commands,
                    &art,
                    &mut rng,
                    Kind::Stinger,
                    en.elite,
                    pos.0 + off,
                    false,
                );
                commands
                    .entity(s)
                    .insert(Vel(off.normalize_or(Vec2::X) * 260.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn speed_scales_with_wave_and_is_capped() {
        assert_eq!(speed_mul(1, false), 1.0);
        assert!(speed_mul(5, false) > speed_mul(2, false));
        assert!(speed_mul(500, false) <= 1.7 + 1e-6);
        assert!(speed_mul(3, true) > speed_mul(3, false));
    }

    fn art() -> Art {
        let pair = || [Handle::default(), Handle::default()];
        Art {
            mat: Handle::default(),
            glow: Handle::default(),
            glow_mat: Handle::default(),
            ship: Handle::default(),
            flame: Handle::default(),
            missile: Handle::default(),
            colonist: Handle::default(),
            harvester: pair(),
            wraith: pair(),
            hive: pair(),
            stinger: pair(),
            stalker: pair(),
            shield: Handle::default(),
            power: Default::default(),
        }
    }

    fn world() -> World {
        let mut w = World::new();
        w.init_resource::<Time>();
        w.init_resource::<Game>();
        w.init_resource::<Rng>();
        w.init_resource::<GameCam>();
        w.init_resource::<Messages<Fx>>();
        w.init_resource::<Messages<Sfx>>();
        w.init_resource::<Messages<Damage>>();
        w.insert_resource(art());
        // Player far away and "dead" so nothing shoots during the test.
        w.insert_resource(PlayerState::default());
        w
    }

    fn spawn(w: &mut World, kind: Kind, pos: Vec2) -> Entity {
        let e = w
            .run_system_once(
                move |mut commands: Commands, art: Res<Art>, mut rng: ResMut<Rng>| {
                    spawn_enemy(&mut commands, &art, &mut rng, kind, false, pos, false)
                },
            )
            .unwrap();
        w.flush();
        e
    }

    fn colonist(w: &mut World, x: f32) -> Entity {
        w.spawn((
            Colonist {
                state: CState::Walking,
                claimed_by: None,
                dir: 1.0,
                t: 0.0,
                fall_from: 0.0,
                vy: 0.0,
            },
            Pos(Vec2::new(x, terrain::height(x) + 9.0)),
        ))
        .id()
    }

    fn kinds(w: &mut World) -> Vec<Kind> {
        w.query::<&Enemy>().iter(w).map(|e| e.kind).collect()
    }

    #[test]
    fn harvester_grabs_its_target_and_becomes_a_wraith_at_the_top() {
        let x = 2000.0;
        let mut w = world();
        let c = colonist(&mut w, x);
        let cy = w.get::<Pos>(c).unwrap().0.y;
        let h = spawn(&mut w, Kind::Harvester, Vec2::new(x, cy + 20.0));
        w.get_mut::<Colonist>(c).unwrap().claimed_by = Some(h);
        {
            let mut en = w.get_mut::<Enemy>(h).unwrap();
            en.ai = Ai::Descend;
            en.target = Some(c);
        }

        // Close enough: the colonist is grabbed and the Harvester starts to climb.
        w.run_system_once(enemy_ai).unwrap();
        assert_eq!(w.get::<Colonist>(c).unwrap().state, CState::Abducted(h));
        assert_eq!(w.get::<Enemy>(h).unwrap().ai, Ai::Rise);

        // Reaching the top loses the colonist and turns the Harvester into a Wraith.
        w.get_mut::<Pos>(h).unwrap().0.y = TOP_Y;
        w.run_system_once(enemy_ai).unwrap();
        assert!(w.get_entity(c).is_err(), "colonist should be gone");
        assert!(w.get_entity(h).is_err(), "harvester should be gone");
        assert_eq!(kinds(&mut w), vec![Kind::Wraith]);
    }

    #[test]
    fn harvester_gives_up_when_its_target_is_taken() {
        let x = 2000.0;
        let mut w = world();
        let c = colonist(&mut w, x);
        let h = spawn(&mut w, Kind::Harvester, Vec2::new(x, 0.0));
        {
            let mut en = w.get_mut::<Enemy>(h).unwrap();
            en.ai = Ai::Descend;
            en.target = Some(c);
        }
        // The colonist is claimed by nobody (e.g. already rescued and re-claimed elsewhere).
        w.run_system_once(enemy_ai).unwrap();
        let en = w.get::<Enemy>(h).unwrap();
        assert_eq!(en.ai, Ai::Roam);
        assert_eq!(en.target, None);
    }

    #[test]
    fn destroyed_hive_releases_stingers_and_scores() {
        let mut w = world();
        let hive = spawn(&mut w, Kind::Hive, Vec2::new(500.0, 0.0));
        w.resource_mut::<Messages<Damage>>().write(Damage {
            entity: hive,
            amount: 999,
        });
        w.run_system_once(apply_damage).unwrap();
        assert_eq!(kinds(&mut w), vec![Kind::Stinger; 5]);
        assert_eq!(w.resource::<Game>().score, Kind::Hive.score());
    }

    #[test]
    fn tough_enemies_survive_a_single_hit() {
        let mut w = world();
        let hive = spawn(&mut w, Kind::Hive, Vec2::new(500.0, 0.0));
        w.resource_mut::<Messages<Damage>>().write(Damage {
            entity: hive,
            amount: 1,
        });
        w.run_system_once(apply_damage).unwrap();
        assert_eq!(kinds(&mut w), vec![Kind::Hive]);
        assert_eq!(w.resource::<Game>().score, 0);
    }

    /// Ship and an enemy in the same place; returns (hits on the ship, damage messages).
    fn ram(invincible: bool) -> (usize, usize) {
        let mut w = world();
        w.init_resource::<Messages<PlayerHit>>();
        if invincible {
            w.resource_mut::<Game>().grant_power(Power::Invincible);
        }
        let at = Vec2::new(500.0, 0.0);
        w.spawn((
            Player {
                facing: 1.0,
                invuln: 0.0,
                fire_cd: 0.0,
                missile_cd: 0.0,
            },
            Pos(at),
        ));
        spawn(&mut w, Kind::Harvester, at);
        w.run_system_once(player_collisions).unwrap();
        let hits = w.resource_mut::<Messages<PlayerHit>>().drain().count();
        let dmg = w.resource_mut::<Messages<Damage>>().drain().count();
        (hits, dmg)
    }

    #[test]
    fn an_invincible_ship_rams_enemies_without_being_hit() {
        assert_eq!(ram(false), (1, 1));
        assert_eq!(ram(true), (0, 1));
    }
}
