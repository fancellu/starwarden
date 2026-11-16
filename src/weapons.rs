//! Laser bolts, homing missiles and the smart bomb.

use bevy::prelude::*;

use crate::consts::*;
use crate::enemies::{EShot, Enemy};
use crate::fx::Glow;
use crate::input::PlayerInput;
use crate::meshes::Art;
use crate::player::Player;
use crate::state::*;
use crate::world::*;

#[derive(Component)]
pub struct Bolt {
    pub life: f32,
    /// Wide-beam bolts pierce: they keep going and remember whom they hit.
    pub wide: bool,
    pub hits: Vec<Entity>,
}

#[derive(Component)]
pub struct Missile {
    pub life: f32,
    pub target: Option<Entity>,
    pub smoke: f32,
}

pub fn fire_weapons(
    time: Res<Time>,
    mut commands: Commands,
    input: Res<PlayerInput>,
    art: Res<Art>,
    view: Res<View>,
    mut game: ResMut<Game>,
    mut cam: ResMut<GameCam>,
    mut flash: ResMut<Flash>,
    mut player: Query<(&mut Player, &Pos, &Vel)>,
    bolts: Query<(), With<Bolt>>,
    enemies: Query<(Entity, &Pos, &Enemy)>,
    shots: Query<(Entity, &Pos), With<EShot>>,
    mut dmg: MessageWriter<Damage>,
    mut sfx: MessageWriter<Sfx>,
) {
    let Ok((mut p, pos, vel)) = player.single_mut() else {
        return;
    };

    // --- Laser ---
    if input.fire && p.fire_cd <= 0.0 && bolts.iter().count() < LASER_MAX {
        p.fire_cd = LASER_COOLDOWN;
        // The beam colour cycles so a held trigger paints a shifting ribbon.
        let t = (time.elapsed_secs() * 3.0).sin() * 0.5 + 0.5;
        let color = C_CYAN.mix(&C_MAGENTA, t);
        let wide = game.power_on(Power::WideBeam);
        let height = if wide { 2.6 * WIDE_BEAM } else { 2.6 };
        commands.spawn((
            Bolt {
                life: LASER_LIFE,
                wide,
                hits: Vec::new(),
            },
            GameEntity,
            Pos(pos.0 + Vec2::new(p.facing * (26.0 + LASER_HALF_LEN), -1.0)),
            Vel(Vec2::new(p.facing * LASER_SPEED + vel.0.x, 0.0)),
            Glow {
                color,
                size: Vec2::new(LASER_HALF_LEN * 2.0, height),
                soft: false,
            },
        ));
        sfx.write(Sfx::Laser);
    }

    // --- Missile ---
    if input.missile && p.missile_cd <= 0.0 && game.missiles > 0 {
        game.missiles -= 1;
        p.missile_cd = 0.3;
        commands.spawn((
            Missile {
                life: MISSILE_LIFE,
                target: None,
                smoke: 0.0,
            },
            GameEntity,
            Pos(pos.0 + Vec2::new(p.facing * 10.0, -8.0)),
            Vel(Vec2::new(p.facing * 380.0 + vel.0.x, -60.0)),
            Mesh2d(art.missile.clone()),
            MeshMaterial2d(art.mat.clone()),
            Transform::from_xyz(0.0, 0.0, Z_SHOT),
        ));
        sfx.write(Sfx::Missile);
    }

    // --- Smart bomb: everything on screen is destroyed ---
    if input.bomb && game.bombs > 0 {
        game.bombs -= 1;
        let half = view.w * 0.5 + 40.0;
        for (e, epos, enemy) in &enemies {
            if enemy.warp <= 0.0 && wrap_delta(epos.0.x - cam.x).abs() < half {
                dmg.write(Damage {
                    entity: e,
                    amount: 999,
                });
            }
        }
        for (e, spos) in &shots {
            if wrap_delta(spos.0.x - cam.x).abs() < half {
                commands.entity(e).try_despawn();
            }
        }
        flash.0 = 1.0;
        cam.shake = cam.shake.max(1.2);
        sfx.write(Sfx::Bomb);
    }
}

pub fn update_bolts(
    time: Res<Time>,
    mut commands: Commands,
    mut bolts: Query<(Entity, &mut Bolt, &Pos, &mut Glow)>,
    enemies: Query<(Entity, &Pos, &Enemy)>,
    mut dmg: MessageWriter<Damage>,
    mut fx: MessageWriter<Fx>,
) {
    let dt = time.delta_secs();
    for (e, mut b, pos, mut glow) in &mut bolts {
        b.life -= dt;
        if b.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        // Fade out towards the end of the bolt's range.
        let a = (b.life / 0.12).min(1.0);
        glow.color.alpha = a;
        let band = if b.wide { 3.0 * WIDE_BEAM } else { 3.0 };
        for (ee, epos, enemy) in &enemies {
            if enemy.warp > 0.0 || b.hits.contains(&ee) {
                continue;
            }
            let d = delta(pos.0, epos.0);
            if d.x.abs() < LASER_HALF_LEN + enemy.radius && d.y.abs() < enemy.radius + band {
                dmg.write(Damage {
                    entity: ee,
                    amount: 1,
                });
                fx.write(Fx::new(epos.0, C_WHITE, FxKind::Spark));
                if b.wide {
                    b.hits.push(ee);
                } else {
                    commands.entity(e).try_despawn();
                    break;
                }
            }
        }
    }
}

pub fn update_missiles(
    time: Res<Time>,
    mut commands: Commands,
    mut missiles: Query<(Entity, &mut Missile, &Pos, &mut Vel, &mut Transform)>,
    enemies: Query<(Entity, &Pos, &Enemy)>,
    mut dmg: MessageWriter<Damage>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut cam: ResMut<GameCam>,
) {
    let dt = time.delta_secs();
    for (e, mut m, pos, mut vel, mut tf) in &mut missiles {
        m.life -= dt;

        // Acquire (or re-acquire) the nearest live enemy.
        let valid = m
            .target
            .and_then(|t| enemies.get(t).ok())
            .is_some_and(|(_, _, en)| en.hp > 0);
        if !valid {
            m.target = enemies
                .iter()
                .filter(|(_, _, en)| en.warp <= 0.0)
                .map(|(ee, epos, _)| (ee, delta(pos.0, epos.0).length()))
                .filter(|(_, d)| *d < 1100.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(ee, _)| ee);
        }

        // Steer towards the target with a limited turn rate, accelerating all the while.
        let speed = (vel.0.length() + 1400.0 * dt).min(MISSILE_SPEED);
        let mut dir = vel.0.normalize_or(Vec2::X);
        if let Some((_, tpos, _)) = m.target.and_then(|t| enemies.get(t).ok()) {
            let want = delta(pos.0, tpos.0).normalize_or(dir);
            let ang = dir
                .angle_to(want)
                .clamp(-MISSILE_TURN * dt, MISSILE_TURN * dt);
            dir = Vec2::from_angle(ang).rotate(dir);
        }
        vel.0 = dir * speed;
        tf.rotation = Quat::from_rotation_z(dir.to_angle());

        m.smoke -= dt;
        if m.smoke <= 0.0 {
            m.smoke = 0.02;
            fx.write(Fx::new(pos.0 - dir * 9.0, C_AMBER, FxKind::Smoke));
        }

        let hit = enemies
            .iter()
            .any(|(_, epos, en)| en.warp <= 0.0 && delta(pos.0, epos.0).length() < en.radius + 9.0);
        if hit || m.life <= 0.0 {
            for (ee, epos, en) in &enemies {
                if en.warp <= 0.0 && delta(pos.0, epos.0).length() < MISSILE_BLAST + en.radius {
                    dmg.write(Damage {
                        entity: ee,
                        amount: 3,
                    });
                }
            }
            fx.write(Fx::new(pos.0, C_AMBER, FxKind::Explosion));
            fx.write(Fx::new(pos.0, C_WHITE, FxKind::Rescue));
            sfx.write(Sfx::ExplodeSmall);
            cam.shake = cam.shake.max(0.35);
            commands.entity(e).try_despawn();
        }
    }
}
