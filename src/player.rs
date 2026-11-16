//! The player's ship: flight model, death and respawn.

use bevy::prelude::*;

use crate::consts::*;
use crate::fx::Glow;
use crate::input::PlayerInput;
use crate::meshes::Art;
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Component)]
pub struct Player {
    pub facing: f32,
    pub invuln: f32,
    pub fire_cd: f32,
    pub missile_cd: f32,
}

#[derive(Component)]
pub struct Flame;

/// A snapshot of the ship that other systems (enemy AI, camera, colonists) read, so they do not
/// need to query the player entity.
#[derive(Resource)]
pub struct PlayerState {
    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: f32,
    pub alive: bool,
    pub respawn: f32,
    /// Colonists hanging under the ship, counted by `update_colonists`.
    pub carried: u32,
    /// Sprint meter, 0 to 1.
    pub sprint: f32,
    /// Set when the meter runs dry; cleared once it refills to `SPRINT_RESUME`.
    pub sprint_locked: bool,
    /// Sprinting this frame.
    pub sprinting: bool,
}

/// Advances the sprint meter by `dt`. Returns the new meter, whether it is locked, and whether
/// the ship is sprinting this frame. With `free` (super sprint) the meter does not drain.
pub fn sprint_step(
    meter: f32,
    locked: bool,
    wanted: bool,
    free: bool,
    dt: f32,
) -> (f32, bool, bool) {
    if free && wanted {
        return (meter, false, true);
    }
    let active = wanted && !locked && meter > 0.0;
    if active {
        let meter = (meter - SPRINT_DRAIN * dt).max(0.0);
        (meter, meter <= 0.0, true)
    } else {
        let meter = (meter + SPRINT_REFILL * dt).min(1.0);
        (meter, locked && meter < SPRINT_RESUME, false)
    }
}

/// Fraction of normal speed left when carrying `n` colonists.
pub fn carry_speed(n: u32) -> f32 {
    (1.0 - CARRY_SLOW * n as f32).max(CARRY_MIN_SPEED)
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            pos: Vec2::new(WORLD_W * 0.5, 0.0),
            vel: Vec2::ZERO,
            facing: 1.0,
            alive: false,
            respawn: 0.0,
            carried: 0,
            sprint: 1.0,
            sprint_locked: false,
            sprinting: false,
        }
    }
}

pub fn spawn_player(commands: &mut Commands, art: &Art, pos: Vec2, facing: f32, invuln: f32) {
    commands.spawn((
        Player {
            facing,
            invuln,
            fire_cd: 0.0,
            missile_cd: 0.0,
        },
        GameEntity,
        Pos(pos),
        Vel(Vec2::ZERO),
        Mesh2d(art.ship.clone()),
        MeshMaterial2d(art.mat.clone()),
        // Shield halo; invisible until the ship holds a shield.
        Glow {
            color: LinearRgba::NONE,
            size: Vec2::splat(SHIELD_HALO),
            soft: true,
        },
        Transform::from_xyz(0.0, pos.y, Z_PLAYER).with_scale(Vec3::new(facing, 1.0, 1.0)),
        children![(
            Flame,
            Mesh2d(art.flame.clone()),
            MeshMaterial2d(art.mat.clone()),
            Transform::from_xyz(-15.0, 0.0, -0.1),
        )],
    ));
}

pub fn player_control(
    time: Res<Time>,
    input: Res<PlayerInput>,
    mut game: ResMut<Game>,
    mut ps: ResMut<PlayerState>,
    mut q: Query<(
        &mut Player,
        &mut Pos,
        &mut Vel,
        &mut Transform,
        &mut Visibility,
        &mut Glow,
    )>,
    mut flame: Query<&mut Transform, (With<Flame>, Without<Player>)>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    let Ok((mut p, mut pos, mut vel, mut tf, mut vis, mut halo)) = q.single_mut() else {
        return;
    };

    if game.tick_powers(dt) {
        sfx.write(Sfx::PowerDown);
    }
    let invincible = game.power_on(Power::Invincible);

    // Invincibility shows as a bright gold halo; otherwise the halo brightens a little with
    // each shield held and pulses gently.
    halo.color = if invincible {
        let pulse = 0.75 + 0.25 * (time.elapsed_secs() * 9.0).sin();
        with_alpha(C_AMBER, 0.35 * pulse)
    } else if game.shields > 0 {
        let pulse = 0.85 + 0.15 * (time.elapsed_secs() * 3.0).sin();
        let strength = 0.10 + 0.02 * game.shields.min(SHIELD_MAX) as f32;
        with_alpha(C_CYAN, strength * pulse)
    } else {
        LinearRgba::NONE
    };

    if input.mv.x.abs() > 0.15 {
        p.facing = input.mv.x.signum();
    }
    let super_sprint = game.power_on(Power::SuperSprint);
    let (meter, locked, sprinting) =
        sprint_step(ps.sprint, ps.sprint_locked, input.sprint, super_sprint, dt);
    if sprinting && !ps.sprinting {
        sfx.write(Sfx::Sprint);
    }
    ps.sprint = meter;
    ps.sprint_locked = locked;
    ps.sprinting = sprinting;

    // Each colonist carried makes the ship a little slower; sprinting and the speed powerup
    // make it faster.
    let boost = match (sprinting, super_sprint) {
        (false, _) => 1.0,
        (true, false) => SPRINT_BOOST,
        (true, true) => SUPER_SPRINT_BOOST,
    };
    let speed = if game.power_on(Power::Speed) {
        POWER_SPEED
    } else {
        1.0
    };
    let load = carry_speed(ps.carried) * boost * speed;
    vel.0.x += input.mv.x * PLAYER_ACCEL * load * dt;
    vel.0.x *= (-PLAYER_DRAG * dt).exp();
    vel.0.x = vel.0.x.clamp(-PLAYER_MAX_VX * load, PLAYER_MAX_VX * load);
    vel.0.y += (input.mv.y * PLAYER_VY - vel.0.y) * smooth(14.0, dt);

    // Stay between the ground (or the bottom of the void) and the ceiling.
    let floor = if game.phase == Phase::Planet {
        terrain::height(pos.0.x) + 16.0
    } else {
        VOID_FLOOR
    };
    if pos.0.y < floor {
        pos.0.y = floor;
        vel.0.y = vel.0.y.max(0.0);
    }
    if pos.0.y > TOP_Y {
        pos.0.y = TOP_Y;
        vel.0.y = vel.0.y.min(0.0);
    }

    p.invuln = (p.invuln - dt).max(0.0);
    p.fire_cd = (p.fire_cd - dt).max(0.0);
    p.missile_cd = (p.missile_cd - dt).max(0.0);

    // The ship flips smoothly when it reverses, and banks slightly when climbing or diving.
    let sx = tf.scale.x + (p.facing - tf.scale.x) * smooth(18.0, dt);
    tf.scale.x = if sx.abs() < 0.05 { 0.05 * p.facing } else { sx };
    tf.rotation = Quat::from_rotation_z(vel.0.y * 0.0005 * p.facing);
    let blink =
        p.invuln > 0.0 && !invincible && ((time.elapsed_secs() * 14.0) as u32).is_multiple_of(2);
    *vis = if blink {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    let thrust = input.mv.x.abs();
    let flicker = 0.8 + 0.4 * (time.elapsed_secs() * 60.0).sin();
    // The flame stretches while sprinting.
    let stretch = if sprinting { 1.7 } else { 1.0 };
    for mut f in &mut flame {
        f.scale.x = (0.25 + thrust * 1.1) * flicker * stretch;
        f.scale.y = (0.7 + thrust * 0.5) * stretch.sqrt();
    }
    if thrust > 0.3 {
        fx.write(Fx::new(
            pos.0 + Vec2::new(-p.facing * 20.0, 0.0),
            C_FLAME,
            FxKind::Trail(Vec2::new(-p.facing * 160.0 + vel.0.x * 0.4, 0.0)),
        ));
    }

    ps.pos = pos.0;
    ps.vel = vel.0;
    ps.facing = p.facing;
    ps.alive = true;
}

pub fn player_death(
    mut commands: Commands,
    mut hits: MessageReader<PlayerHit>,
    mut game: ResMut<Game>,
    mut ps: ResMut<PlayerState>,
    mut cam: ResMut<GameCam>,
    mut q: Query<(Entity, &mut Player, &Pos)>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    if hits.read().count() == 0 {
        return;
    }
    let Ok((e, mut p, pos)) = q.single_mut() else {
        return;
    };
    if p.invuln > 0.0 || game.power_on(Power::Invincible) {
        return;
    }
    game.break_combo();
    if game.shields > 0 {
        // A shield takes the hit instead of the ship.
        game.shields -= 1;
        p.invuln = SHIELD_INVULN;
        fx.write(Fx::new(pos.0, C_CYAN, FxKind::Rescue));
        fx.write(Fx::new(pos.0, C_CYAN, FxKind::Explosion));
        sfx.write(Sfx::ShieldHit);
        cam.shake = cam.shake.max(0.5);
        return;
    }
    commands.entity(e).try_despawn();
    fx.write(Fx::new(pos.0, C_CYAN, FxKind::BigExplosion));
    sfx.write(Sfx::PlayerDie);
    cam.shake = 1.4;
    game.lives = game.lives.saturating_sub(1);
    game.clear_powers();
    ps.alive = false;
    ps.carried = 0;
    ps.vel = Vec2::ZERO;
    ps.respawn = 2.2;
}

pub fn player_respawn(
    time: Res<Time>,
    mut commands: Commands,
    art: Res<Art>,
    mut game: ResMut<Game>,
    mut ps: ResMut<PlayerState>,
    mut next: ResMut<NextState<GameState>>,
    q: Query<(), With<Player>>,
) {
    if ps.alive || !q.is_empty() {
        return;
    }
    ps.respawn -= time.delta_secs();
    if ps.respawn > 0.0 {
        return;
    }
    if game.lives == 0 {
        game.over_time = 0.0;
        next.set(GameState::GameOver);
        return;
    }
    let pos = Vec2::new(ps.pos.x, 40.0);
    ps.pos = pos;
    ps.alive = true;
    ps.sprint = 1.0;
    ps.sprint_locked = false;
    spawn_player(&mut commands, &art, pos, ps.facing, 2.5);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn world(shields: u32) -> (World, Entity) {
        let mut w = World::new();
        w.insert_resource(Game {
            shields,
            ..default()
        });
        w.insert_resource(PlayerState {
            alive: true,
            ..default()
        });
        w.init_resource::<GameCam>();
        w.init_resource::<Messages<PlayerHit>>();
        w.init_resource::<Messages<Fx>>();
        w.init_resource::<Messages<Sfx>>();
        let ship = w
            .spawn((
                Player {
                    facing: 1.0,
                    invuln: 0.0,
                    fire_cd: 0.0,
                    missile_cd: 0.0,
                },
                Pos(Vec2::new(100.0, 100.0)),
            ))
            .id();
        w.resource_mut::<Messages<PlayerHit>>().write(PlayerHit);
        (w, ship)
    }

    #[test]
    fn a_shield_absorbs_the_hit() {
        let (mut w, ship) = world(1);
        w.run_system_once(player_death).unwrap();
        assert!(w.get_entity(ship).is_ok());
        assert_eq!(w.get::<Player>(ship).unwrap().invuln, SHIELD_INVULN);
        assert_eq!(w.resource::<Game>().shields, 0);
        assert_eq!(w.resource::<Game>().lives, START_LIVES);
        assert!(w.resource::<PlayerState>().alive);
    }

    #[test]
    fn without_a_shield_the_ship_is_lost() {
        let (mut w, ship) = world(0);
        w.run_system_once(player_death).unwrap();
        assert!(w.get_entity(ship).is_err());
        assert_eq!(w.resource::<Game>().lives, START_LIVES - 1);
        assert!(!w.resource::<PlayerState>().alive);
    }

    #[test]
    fn invincibility_ignores_hits_and_a_lost_ship_loses_its_powers() {
        let (mut w, ship) = world(0);
        w.resource_mut::<Game>().grant_power(Power::Invincible);
        w.run_system_once(player_death).unwrap();
        assert!(w.get_entity(ship).is_ok());

        let (mut w, ship) = world(0);
        w.resource_mut::<Game>().grant_power(Power::Speed);
        w.run_system_once(player_death).unwrap();
        assert!(w.get_entity(ship).is_err());
        assert!(!w.resource::<Game>().power_on(Power::Speed));
    }
}

#[cfg(test)]
mod sprint_tests {
    use super::*;

    #[test]
    fn sprint_drains_while_held_and_refills_when_released() {
        let (m, locked, active) = sprint_step(1.0, false, true, false, 1.0);
        assert!(active && !locked);
        assert!((m - (1.0 - SPRINT_DRAIN)).abs() < 1e-6);
        let (m2, _, active) = sprint_step(m, false, false, false, 1.0);
        assert!(!active);
        assert!((m2 - (m + SPRINT_REFILL)).abs() < 1e-6);
        assert_eq!(sprint_step(1.0, false, false, false, 10.0).0, 1.0);
    }

    #[test]
    fn empty_meter_locks_until_partly_refilled() {
        let (m, locked, _) = sprint_step(0.1, false, true, false, 1.0);
        assert_eq!(m, 0.0);
        assert!(locked);
        // Still locked while held and below the resume level.
        let (m, locked, active) = sprint_step(m, locked, true, false, 0.5);
        assert!(!active && locked);
        let (_, locked, _) = sprint_step(m, locked, true, false, SPRINT_RESUME / SPRINT_REFILL);
        assert!(!locked);
    }

    #[test]
    fn super_sprint_does_not_drain_and_ignores_the_lock() {
        let (m, locked, active) = sprint_step(0.5, false, true, true, 10.0);
        assert!(active && !locked);
        assert_eq!(m, 0.5);
        let (_, locked, active) = sprint_step(0.0, true, true, true, 1.0);
        assert!(active && !locked);
    }
}
