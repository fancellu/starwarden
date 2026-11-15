//! Sky gradient, nebula clouds and a three-layer parallax starfield.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;

use crate::consts::*;
use crate::meshes::{Art, GlowQuads, Shape};
use crate::state::{Game, Phase};
use crate::world::*;

/// Scrolls at a fraction of the camera speed and repeats every `span` units.
#[derive(Component)]
pub struct Parallax {
    pub factor: f32,
    pub x: f32,
    pub span: f32,
}

/// One copy of a baked star layer. The layer repeats every [`SPAN`] units.
#[derive(Component)]
pub struct StarLayer {
    /// Fraction of the camera speed this layer scrolls at.
    factor: f32,
    /// Which of the two side-by-side copies this is (-1 or 0).
    copy: f32,
}

#[derive(Component)]
pub struct Nebula {
    pub planet: LinearRgba,
    pub void: LinearRgba,
}

#[derive(Component)]
pub struct Sky;

/// 0 while the planet stands, easing to 1 in the void phase; drives the colour shift.
#[derive(Resource, Default)]
pub struct VoidBlend(pub f32);

const SPAN: f32 = 3200.0;

pub fn setup_background(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Sky: deep blue overhead, a violet haze near the horizon.
    let top = rgb(0.0008, 0.0015, 0.008);
    let mid = rgb(0.003, 0.002, 0.016);
    let low = rgb(0.018, 0.004, 0.034);
    let mut s = Shape::default();
    let w = 4000.0;
    s.quad(
        [(-w, 1500.0), (w, 1500.0), (w, 0.0), (-w, 0.0)],
        [top, top, mid, mid],
    );
    s.quad(
        [(-w, 0.0), (w, 0.0), (w, -340.0), (-w, -340.0)],
        [mid, mid, low, low],
    );
    s.quad(
        [(-w, -340.0), (w, -340.0), (w, -1500.0), (-w, -1500.0)],
        [low, low, top, top],
    );
    commands.spawn((
        Sky,
        Mesh2d(meshes.add(s.build())),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, Z_SKY),
    ));

    // Nebula clouds.
    let tints = [
        (rgba(0.25, 0.05, 0.6, 0.085), rgba(0.7, 0.05, 0.1, 0.10)),
        (rgba(0.02, 0.3, 0.6, 0.075), rgba(0.6, 0.2, 0.02, 0.09)),
        (rgba(0.5, 0.05, 0.35, 0.07), rgba(0.5, 0.02, 0.3, 0.09)),
    ];
    for i in 0..9 {
        let (planet, void) = tints[i % tints.len()];
        let size = Vec2::new(rng.range(700.0, 1300.0), rng.range(320.0, 620.0));
        commands.spawn((
            Nebula { planet, void },
            Parallax {
                factor: 0.06,
                x: i as f32 / 9.0 * SPAN + rng.range(-100.0, 100.0),
                span: SPAN,
            },
            Sprite {
                image: art.glow.clone(),
                color: col(planet),
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(0.0, rng.range(-150.0, 320.0), Z_NEBULA),
        ));
    }

    // Stars: far layers are dimmer, smaller and slower.
    for (layer, (factor, count, size, bright)) in [
        (0.08, 130, 5.0, 0.7),
        (0.18, 80, 8.0, 1.4),
        (0.34, 36, 12.0, 2.6),
    ]
    .into_iter()
    .enumerate()
    {
        // Each layer is baked into one mesh. Two copies side by side cover the screen as the
        // layer scrolls.
        let mut quads = GlowQuads::default();
        for _ in 0..count {
            let tint = match rng.index(5) {
                0 => rgb(0.6, 0.8, 1.0),
                1 => rgb(1.0, 0.7, 0.9),
                _ => rgb(1.0, 1.0, 1.0),
            };
            let b = bright * rng.range(0.5, 1.0);
            let base = rgb(tint.red * b, tint.green * b, tint.blue * b);
            quads.push(
                Vec2::new(rng.range(0.0, SPAN), rng.range(-380.0, 700.0)),
                Vec2::splat(size * rng.range(0.6, 1.0)),
                base,
                true,
            );
        }
        let mesh = meshes.add(quads.build());
        for copy in [-1.0, 0.0] {
            commands.spawn((
                StarLayer { factor, copy },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(art.glow_mat.clone()),
                Transform::from_xyz(0.0, 0.0, Z_STARS + layer as f32),
                NoFrustumCulling,
            ));
        }
    }
}

pub fn update_background(
    time: Res<Time>,
    cam: Res<GameCam>,
    game: Res<Game>,
    mut blend: ResMut<VoidBlend>,
    mut scroll: Local<(f32, f32)>,
    mut layers: Query<(&Parallax, &mut Transform), Without<StarLayer>>,
    mut stars: Query<(&StarLayer, &mut Transform)>,
    mut nebulae: Query<(&Nebula, &mut Sprite)>,
    sky: Query<&MeshMaterial2d<ColorMaterial>, With<Sky>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    // Track the camera as an unwrapped distance so the parallax never jumps at the seam.
    let (last, total) = &mut *scroll;
    *total += wrap_delta(cam.x - *last);
    *last = cam.x;
    // A slow constant drift keeps the title screen alive.
    let travel = *total + t * 14.0;

    for (p, mut tf) in &mut layers {
        tf.translation.x =
            (p.x - travel * p.factor + p.span * 0.5).rem_euclid(p.span) - p.span * 0.5;
    }
    for (s, mut tf) in &mut stars {
        let left = (-travel * s.factor).rem_euclid(SPAN) - SPAN * 0.5;
        tf.translation.x = left + s.copy * SPAN;
    }

    let target = if game.phase == Phase::Void { 1.0 } else { 0.0 };
    let before = blend.0;
    blend.0 += (target - blend.0) * smooth(1.2, dt);
    if (blend.0 - before).abs() > 1e-4 {
        for (n, mut sprite) in &mut nebulae {
            sprite.color = col(n.planet.mix(&n.void, blend.0));
        }
        // The sky takes on a blood-red cast in the void.
        if let Ok(h) = sky.single() {
            if let Some(mut m) = materials.get_mut(&h.0) {
                let k = blend.0;
                m.color = col(rgb(1.0 + 1.6 * k, 1.0 - 0.5 * k, 1.0 - 0.6 * k));
            }
        }
    }
}
