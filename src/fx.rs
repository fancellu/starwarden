//! Particles, score pop-ups and other purely visual effects.

use bevy::prelude::*;

use crate::consts::*;
use crate::meshes::Art;
use crate::state::*;
use crate::world::*;

#[derive(Component)]
pub struct Particle {
    pub life: f32,
    pub max: f32,
    pub size: f32,
    /// Size multiplier reached at the end of life (below 1 shrinks, above 1 grows).
    pub grow: f32,
    pub color: LinearRgba,
    pub drag: f32,
    pub gravity: f32,
}

#[derive(Component)]
pub struct Popup {
    pub life: f32,
}

struct Spec {
    life: f32,
    size: f32,
    grow: f32,
    drag: f32,
    gravity: f32,
}

fn emit(
    commands: &mut Commands,
    art: &Art,
    pos: Vec2,
    vel: Vec2,
    color: LinearRgba,
    spec: Spec,
) {
    commands.spawn((
        Particle {
            life: spec.life,
            max: spec.life,
            size: spec.size,
            grow: spec.grow,
            color,
            drag: spec.drag,
            gravity: spec.gravity,
        },
        GameEntity,
        Pos(pos),
        Vel(vel),
        Sprite {
            image: art.glow.clone(),
            color: col(color),
            custom_size: Some(Vec2::splat(spec.size)),
            ..default()
        },
        Transform::from_xyz(0.0, pos.y, Z_PARTICLE),
    ));
}

pub fn spawn_fx(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut reader: MessageReader<Fx>,
    particles: Query<(), With<Particle>>,
) {
    // Under heavy load, drop the cheap decorative effects first.
    let mut budget = MAX_PARTICLES.saturating_sub(particles.iter().count());
    for f in reader.read() {
        let white = C_WHITE;
        match f.kind {
            FxKind::Explosion | FxKind::BigExplosion => {
                let big = matches!(f.kind, FxKind::BigExplosion);
                let (n, speed, life) = if big { (90, 520.0, 1.5) } else { (26, 330.0, 0.75) };
                for i in 0..n {
                    let v = rng.dir() * rng.range(0.15, 1.0) * speed;
                    let c = if i % 3 == 0 { white } else { f.color };
                    emit(
                        &mut commands,
                        &art,
                        f.pos,
                        v,
                        c,
                        Spec {
                            life: life * rng.range(0.5, 1.0),
                            size: rng.range(8.0, 20.0),
                            grow: 0.2,
                            drag: 2.4,
                            gravity: 0.0,
                        },
                    );
                }
                // A quick bright core flash.
                emit(
                    &mut commands,
                    &art,
                    f.pos,
                    Vec2::ZERO,
                    with_alpha(f.color.mix(&white, 0.5), if big { 0.9 } else { 0.55 }),
                    Spec {
                        life: if big { 0.45 } else { 0.2 },
                        size: if big { 130.0 } else { 48.0 },
                        grow: 1.9,
                        drag: 0.0,
                        gravity: 0.0,
                    },
                );
                budget = budget.saturating_sub(n);
            }
            FxKind::Spark => {
                for _ in 0..5 {
                    emit(
                        &mut commands,
                        &art,
                        f.pos,
                        rng.dir() * rng.range(80.0, 260.0),
                        f.color,
                        Spec {
                            life: rng.range(0.12, 0.3),
                            size: 9.0,
                            grow: 0.3,
                            drag: 3.0,
                            gravity: 0.0,
                        },
                    );
                }
            }
            FxKind::Warp => {
                // Particles converge on the spawn point.
                for i in 0..22 {
                    let d = Vec2::from_angle(i as f32 / 22.0 * std::f32::consts::TAU);
                    let r = rng.range(50.0, 80.0);
                    emit(
                        &mut commands,
                        &art,
                        f.pos + d * r,
                        -d * r / 0.6,
                        f.color,
                        Spec {
                            life: 0.6,
                            size: 11.0,
                            grow: 0.6,
                            drag: 0.0,
                            gravity: 0.0,
                        },
                    );
                }
            }
            FxKind::Rescue => {
                for i in 0..20 {
                    let d = Vec2::from_angle(i as f32 / 20.0 * std::f32::consts::TAU);
                    emit(
                        &mut commands,
                        &art,
                        f.pos,
                        d * 230.0,
                        f.color,
                        Spec {
                            life: 0.5,
                            size: 10.0,
                            grow: 0.4,
                            drag: 3.0,
                            gravity: 0.0,
                        },
                    );
                }
            }
            FxKind::Trail(v) => {
                if budget > 200 {
                    emit(
                        &mut commands,
                        &art,
                        f.pos + Vec2::new(0.0, rng.range(-2.0, 2.0)),
                        v + rng.dir() * 25.0,
                        f.color,
                        Spec {
                            life: rng.range(0.18, 0.34),
                            size: 13.0,
                            grow: 0.15,
                            drag: 1.5,
                            gravity: 0.0,
                        },
                    );
                    budget -= 1;
                }
            }
            FxKind::Smoke => {
                if budget > 100 {
                    emit(
                        &mut commands,
                        &art,
                        f.pos,
                        rng.dir() * 18.0,
                        with_alpha(f.color, 0.6),
                        Spec {
                            life: 0.4,
                            size: 9.0,
                            grow: 1.8,
                            drag: 1.0,
                            gravity: 0.0,
                        },
                    );
                    budget -= 1;
                }
            }
            FxKind::PlanetChunk => {
                for i in 0..7 {
                    let v = Vec2::new(rng.range(-260.0, 260.0), rng.range(120.0, 620.0));
                    let c = if i % 2 == 0 { f.color } else { C_CYAN };
                    emit(
                        &mut commands,
                        &art,
                        f.pos,
                        v,
                        c,
                        Spec {
                            life: rng.range(1.2, 2.8),
                            size: rng.range(10.0, 30.0),
                            grow: 0.3,
                            drag: 0.5,
                            gravity: -240.0,
                        },
                    );
                }
            }
            FxKind::Score(pts) => {
                commands.spawn((
                    Popup { life: 1.1 },
                    GameEntity,
                    Pos(f.pos + Vec2::new(0.0, 18.0)),
                    Vel(Vec2::new(0.0, 46.0)),
                    Text2d::new(pts.to_string()),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(col(f.color)),
                    Transform::from_xyz(0.0, 0.0, Z_POPUP),
                ));
            }
        }
    }
}

pub fn update_particles(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Particle, &mut Vel, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut vel, mut sprite) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        let k = p.life / p.max;
        vel.0 *= (-p.drag * dt).exp();
        vel.0.y += p.gravity * dt;
        sprite.color = col(with_alpha(p.color, p.color.alpha * k));
        let s = p.size * (p.grow + (1.0 - p.grow) * k);
        sprite.custom_size = Some(Vec2::splat(s));
    }
}

pub fn update_popups(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Popup, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut color) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        color.0.set_alpha((p.life / 0.4).min(1.0));
    }
}
