//! The planet surface: a periodic height function and the mesh drawn from it.

use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::consts::*;
use crate::meshes::Shape;
use crate::state::{Game, Phase};
use crate::world::{wrap, GameCam};

const SEGMENTS: usize = 640;

/// Height of the ground at world x. Periodic over `WORLD_W`, so the surface joins up with
/// itself where the world wraps.
pub fn height(x: f32) -> f32 {
    let u = wrap(x) / WORLD_W * TAU;
    let ridge = |k: f32, ph: f32| 1.0 - 2.0 * ((u * k + ph).sin()).abs();
    GROUND_BASE
        + 26.0 * (u * 3.0 + 0.4).sin()
        + 18.0 * (u * 7.0 + 1.9).sin()
        + 16.0 * ridge(11.0, 0.7)
        + 9.0 * ridge(29.0, 2.3)
        + 4.0 * (u * 61.0 + 4.1).sin()
}

#[derive(Component)]
pub struct TerrainCopy(pub f32);

fn terrain_mesh() -> Mesh {
    let mut s = Shape::default();
    let step = WORLD_W / SEGMENTS as f32;
    let deep = rgb(0.001, 0.001, 0.006);
    for i in 0..SEGMENTS {
        let (x0, x1) = (i as f32 * step, (i + 1) as f32 * step);
        let (h0, h1) = (height(x0), height(x1));
        // The ridge colour drifts between cyan and magenta along the world.
        let ridge = |x: f32| {
            let t = 0.5 + 0.5 * (x / WORLD_W * TAU * 2.0).sin();
            C_CYAN.mix(&C_MAGENTA, t)
        };
        let (r0, r1) = (ridge(x0), ridge(x1));
        let body = |c: LinearRgba| LinearRgba {
            red: c.red * 0.018,
            green: c.green * 0.018,
            blue: c.blue * 0.018,
            alpha: 1.0,
        };
        // Ground body, fading into darkness.
        s.quad(
            [(x0, h0), (x1, h1), (x1, h1 - 150.0), (x0, h0 - 150.0)],
            [body(r0), body(r1), deep, deep],
        );
        s.quad(
            [(x0, h0 - 150.0), (x1, h1 - 150.0), (x1, -2000.0), (x0, -2000.0)],
            [deep; 4],
        );
        // Bright ridge line.
        s.quad(
            [(x0, h0 + 1.6), (x1, h1 + 1.6), (x1, h1 - 1.6), (x0, h0 - 1.6)],
            [r0, r1, r1, r0],
        );
    }
    s.build()
}

pub fn setup_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mesh = meshes.add(terrain_mesh());
    let mat = materials.add(ColorMaterial::default());
    for i in [-1.0, 0.0, 1.0] {
        commands.spawn((
            TerrainCopy(i),
            Mesh2d(mesh.clone()),
            MeshMaterial2d(mat.clone()),
            Transform::from_xyz(0.0, 0.0, Z_TERRAIN),
        ));
    }
}

/// Three copies of the terrain are laid side by side so the surface is continuous across the
/// wrap seam. In the void phase the planet is gone.
pub fn update_terrain(
    cam: Res<GameCam>,
    game: Res<Game>,
    mut q: Query<(&TerrainCopy, &mut Transform, &mut Visibility)>,
) {
    for (c, mut t, mut vis) in &mut q {
        t.translation.x = -cam.x + c.0 * WORLD_W;
        let want = if game.phase == Phase::Planet {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_periodic_over_the_world() {
        for i in 0..200 {
            let x = i as f32 * 48.0;
            assert!((height(x) - height(x + WORLD_W)).abs() < 0.05);
            assert!((height(x) - height(x - WORLD_W)).abs() < 0.05);
        }
    }

    #[test]
    fn height_stays_below_the_play_area() {
        for i in 0..(WORLD_W as usize) {
            let h = height(i as f32);
            assert!(h < -180.0 && h > -360.0, "height {h} at {i}");
        }
    }
}
